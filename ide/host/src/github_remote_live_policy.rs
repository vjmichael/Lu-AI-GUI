//! FIN-03 / 6E-05B: fixed-origin, authenticated, read-only LIVE branch
//! and policy observations. Never turns public data into remote write rights.
//!
//! The token is provided only by a trusted, OS-native host credential vault,
//! via a synchronous closure. No token is returned, stored, serialized,
//! logged, exposed to Tauri/model, fetched from Git config, or persisted.
//! A connected OS keychain adapter is a separate production acceptance gate.
//! This module does not implement any credential acquisition or push API.
use crate::{IdeWorkspaceRegistry,github_remote_publication::RemotePublicationPreflight};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest,Sha256};
use std::{io::Read,time::{Duration,SystemTime,UNIX_EPOCH}};

const MAX_RESPONSE:u64=256*1024;
const API:&str="https://api.github.com";
const MAX_POLICY_RULES:usize=128;

/// Only the trusted native process implements this interface using OS
/// credential custody. A read-only token is borrowed for a bounded operation,
/// and must never originate from WebView, project contents or an LLM action.
/// There is deliberately no default provider or environment-variable path.
pub trait NativeGitHubReadCredentialVault {
    fn with_token<R,F>(&self,repo:&str,f:F)->Result<R,&'static str>
    where F:FnOnce(&str)->Result<R,&'static str>;
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct LiveBranchPolicyEvidence {
    pub schema_version:u32,
    pub project_id:String,
    pub repository:String,
    pub branch:String,
    pub exact_local_head:String,
    pub exact_local_preview_sha256:String,
    pub remote_head_sha256:Option<String>,
    pub remote_ref_same_on_recheck:bool,
    pub local_is_descendant_of_live_head:bool,
    pub branch_protection_observed:bool,
    pub branch_rules_observed:bool,
    pub policy_same_on_recheck:bool,
    pub policy_response_sha256:Option<String>,
    pub checked_unix_seconds:u64,
    pub error_code:Option<&'static str>,
    pub authenticated_host_read:bool,
    pub source_rechecked:bool,
    pub live_ref_compare_and_swap_authorized:bool,
    pub current_remote_write_lease:bool,
    pub native_approval_verified:bool,
    pub remote_mutation_performed:bool,
    pub remote_write_route_available:bool,
    pub installed_accepted:bool,
}
fn valid_sha(s:&str)->bool{
    s.len()==40&&s.bytes().all(|b|b.is_ascii_hexdigit())
}
fn safe_component(s:&str)->bool{
    !s.is_empty()&&s.len()<=100&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'))
        &&s!="."&&s!=".."
}
fn encode_branch(branch:&str)->Result<String,&'static str>{
    if branch.is_empty()||branch.len()>160
        ||branch.starts_with('/')||branch.ends_with('/')
        ||branch.contains("..")||branch.contains("//")
        ||!branch.bytes().all(|b|b.is_ascii_alphanumeric()
            ||matches!(b,b'_'|b'-'|b'.'|b'/'))
    {return Err("FIN03_LIVE_BRANCH_UNSAFE");}
    let mut encoded=String::new();
    for b in branch.bytes(){
        if b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'|b'.'){
            encoded.push(char::from(b));
        }else{encoded.push_str(&format!("%{b:02X}"));}
    }
    Ok(encoded)
}
fn fingerprint(data:&[u8])->String{format!("{:x}",Sha256::digest(data))}
fn read_fixed(
    agent:&ureq::Agent,token:&str,owner:&str,repo:&str,
    fixed_path:&str,
)->Result<Vec<u8>,&'static str>{
    // Reject a remotely supplied URL, hostname, query or redirect.
    // Only the exact pinned GitHub REST origin and a fixed endpoint are
    // possible. No arbitrary user/model URL or Git remote URL is fetched.
    if token.is_empty()||token.len()>4096||token.bytes().any(|b|
        !b.is_ascii_graphic()||b==b'\r'||b==b'\n'){
        return Err("FIN03_HOST_CREDENTIAL_UNAVAILABLE");
    }
    if !safe_component(owner)||!safe_component(repo)
        ||!fixed_path.starts_with('/')
        ||fixed_path.contains('?')||fixed_path.contains('#')
        ||fixed_path.contains("..")||fixed_path.contains("//")
    {return Err("FIN03_LIVE_API_PATH_DENIED");}
    let url=format!("{API}/repos/{owner}/{repo}{fixed_path}");
    let response=agent.get(&url)
        .set("Authorization",&format!("Bearer {token}"))
        .set("Accept","application/vnd.github+json")
        .set("X-GitHub-Api-Version","2022-11-28")
        .set("User-Agent","Lu-IDE-FIN03-ReadOnly/0.1")
        .call().map_err(|_|"FIN03_LIVE_API_UNAVAILABLE")?;
    if response.status()!=200
        ||response.header("Link").is_some_and(|l|l.contains("rel=\"next\"")){
        return Err("FIN03_LIVE_POLICY_PARTIAL");
    }
    let mut body=Vec::new();
    response.into_reader().take(MAX_RESPONSE+1)
        .read_to_end(&mut body).map_err(|_|"FIN03_LIVE_RESPONSE_UNREADABLE")?;
    if body.is_empty()||body.len() as u64>MAX_RESPONSE{
        return Err("FIN03_LIVE_RESPONSE_BOUNDS");
    }
    Ok(body)
}
fn exact_ref(json:&Value,branch:&str)->Result<String,&'static str>{
    if json.get("ref").and_then(Value::as_str)!=Some(format!("refs/heads/{branch}").as_str())
        ||json.pointer("/object/type").and_then(Value::as_str)!=Some("commit"){
        return Err("FIN03_LIVE_REF_SHAPE_MISMATCH");
    }
    let sha=json.pointer("/object/sha").and_then(Value::as_str)
        .ok_or("FIN03_LIVE_REF_MISSING")?;
    if !valid_sha(sha){return Err("FIN03_LIVE_REF_INVALID");}
    Ok(sha.to_ascii_lowercase())
}
fn parse_fixed_json(body:&[u8])->Result<Value,&'static str>{
    serde_json::from_slice(body).map_err(|_|"FIN03_LIVE_RESPONSE_JSON_INVALID")
}
fn policy_shape(protection:&Value,rules:&Value)->Result<(),&'static str>{
    // The branch protection endpoint alone cannot enumerate rulesets.
    // Require the *evaluated rules for this branch* endpoint as well.
    // A 401/403/404 or an empty/error object remains UNKNOWN, never
    // proof of unprotected status or merge permission.
    if !protection.is_object()
        ||!rules.is_array()
        ||rules.as_array().is_none_or(|a|a.len()>MAX_POLICY_RULES)
    {return Err("FIN03_LIVE_POLICY_UNVERIFIED");}
    for value in rules.as_array().unwrap(){
        if !value.is_object()
            ||value.get("type").and_then(Value::as_str).is_none()
        {return Err("FIN03_LIVE_RULE_SHAPE_MISMATCH");}
    }
    Ok(())
}
fn blank(preview:&RemotePublicationPreflight)->LiveBranchPolicyEvidence{
    LiveBranchPolicyEvidence{
        schema_version:1,project_id:preview.project_id.clone(),
        repository:preview.repository.clone(),branch:preview.local_branch.clone(),
        exact_local_head:preview.local_head.clone(),
        exact_local_preview_sha256:preview.preview_evidence_sha256.clone(),
        remote_head_sha256:None,remote_ref_same_on_recheck:false,
        local_is_descendant_of_live_head:false,
        branch_protection_observed:false,branch_rules_observed:false,
        policy_same_on_recheck:false,policy_response_sha256:None,
        checked_unix_seconds:0,error_code:Some("FIN03_LIVE_POLICY_UNVERIFIED"),
        authenticated_host_read:false,source_rechecked:false,
        live_ref_compare_and_swap_authorized:false,
        current_remote_write_lease:false,native_approval_verified:false,
        remote_mutation_performed:false,remote_write_route_available:false,
        installed_accepted:false,
    }
}
impl IdeWorkspaceRegistry {
    /// An explicit, trusted native-host-only call. It does NOT obtain
    /// credentials itself or accept arbitrary URLs. A vault must be wired
    /// from the native OS credential system before physical deployment.
    /// A successful *read* is never a lease or proof of approval to push.
    pub fn inspect_live_github_branch_policy<V:NativeGitHubReadCredentialVault>(
        &self,project_id:&str,vault:&V,
    )->Result<LiveBranchPolicyEvidence,&'static str>{
        let before=self.review_github_remote_publication(project_id)?;
        let mut out=blank(&before);
        if before.blockers.iter().any(|b|matches!(*b,
            "origin_push_url_override_denied"|"repository_in_git_operation"
            |"worktree_or_index_dirty"|"dirty_status_too_large"
            |"foreign_git_lock_present")){
            return Err("FIN03_LIVE_LOCAL_SOURCE_BLOCKED");
        }
        let (owner,name)=before.repository.split_once('/')
            .ok_or("FIN03_LIVE_CANONICAL_ORIGIN_MISSING")?;
        if !safe_component(owner)||!safe_component(name)||!valid_sha(&before.local_head){
            return Err("FIN03_LIVE_ORIGIN_INVALID");
        }
        let branch=encode_branch(&before.local_branch)?;
        let ref_path=format!("/git/ref/heads/{branch}");
        let protection_path=format!("/branches/{branch}/protection");
        let evaluated_rules_path=format!("/rules/branches/{branch}");
        let agent=ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(4))
            .timeout_read(Duration::from_secs(6))
            .timeout_write(Duration::from_secs(4))
            .build();
        // Token lifetime and source are controlled by trusted native vault.
        // The closure cannot return it as part of the typed response.
        let observed=vault.with_token(&before.repository,|token|{
            let a=read_fixed(&agent,token,owner,name,&ref_path)?;
            let p=read_fixed(&agent,token,owner,name,&protection_path)?;
            let rules=read_fixed(&agent,token,owner,name,&evaluated_rules_path)?;
            let b=read_fixed(&agent,token,owner,name,&ref_path)?;
            let p2=read_fixed(&agent,token,owner,name,&protection_path)?;
            let rules2=read_fixed(&agent,token,owner,name,&evaluated_rules_path)?;
            Ok((a,p,rules,b,p2,rules2))
        })?;
        let (a,p,r,b,p2,r2)=observed;
        let live_a=exact_ref(&parse_fixed_json(&a)?,&before.local_branch)?;
        let live_b=exact_ref(&parse_fixed_json(&b)?,&before.local_branch)?;
        if live_a!=live_b{return Err("FIN03_LIVE_REF_MOVED_DURING_READ");}
        policy_shape(&parse_fixed_json(&p)?,&parse_fixed_json(&r)?)?;
        if p!=p2||r!=r2{
            return Err("FIN03_LIVE_POLICY_MOVED_DURING_READ");
        }
        let after=self.review_github_remote_publication(project_id)?;
        if before.preview_evidence_sha256!=after.preview_evidence_sha256
            ||before.local_head!=after.local_head
            ||before.repository!=after.repository
            ||before.local_branch!=after.local_branch{
            return Err("FIN03_LIVE_LOCAL_SOURCE_MOVED_DURING_READ");
        }
        // Still a read-only snapshot: a remote writer may change the
        // branch immediately afterward. An actual mutation requires a
        // separate atomic ref compare-and-swap with a one-use OS grant.
        out.remote_head_sha256=Some(live_a.clone());
        out.remote_ref_same_on_recheck=true;
        out.local_is_descendant_of_live_head=
            before.local_head.eq_ignore_ascii_case(&live_a)
            ||before.cached_tracking_head.as_deref()
                ==Some(live_a.as_str())
                &&before.local_relationship=="local_descendant_of_stored_tracking";
        out.branch_protection_observed=true;
        out.branch_rules_observed=true;
        out.policy_same_on_recheck=true;
        out.policy_response_sha256=Some(fingerprint(&[p,r].concat()));
        out.checked_unix_seconds=SystemTime::now()
            .duration_since(UNIX_EPOCH).map_err(|_|"FIN03_LIVE_CLOCK_INVALID")?
            .as_secs();
        out.error_code=None;
        out.authenticated_host_read=true;
        out.source_rechecked=true;
        Ok(out)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn branch_endpoint_validation_is_not_an_arbitrary_url(){
        assert_eq!(encode_branch("feature/work").unwrap(),"feature%2Fwork");
        for x in ["../main","main?access_token=abc","http://evil/","refs//heads",""]{
            assert!(encode_branch(x).is_err());
        }
        assert!(exact_ref(&serde_json::json!({
            "ref":"refs/heads/main","object":{"type":"commit","sha":"a".repeat(40)}
        }),"main").is_ok());
        assert!(exact_ref(&serde_json::json!({
            "ref":"refs/tags/main","object":{"type":"commit","sha":"a".repeat(40)}
        }),"main").is_err());
    }
    #[test]fn partial_or_missing_policy_is_unknown_not_approval(){
        assert!(policy_shape(&serde_json::json!({}),
            &serde_json::json!([{"type":"pull_request"}])).is_ok());
        assert!(policy_shape(&serde_json::json!({}),
            &serde_json::json!({"rules":[]})).is_err());
        assert!(policy_shape(&serde_json::json!([]),
            &serde_json::json!([])).is_err());
    }
}
