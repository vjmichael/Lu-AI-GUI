//! FIN03 6E-03: read-only public GitHub branch protection observation.
//! A 401/403/404 means unknown, NOT "branch unprotected".
use crate::IdeWorkspaceRegistry;
use serde::Serialize;
use serde_json::Value;
use std::{io::Read,time::Duration};
const MAX_BYTES:u64=128*1024;
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct BranchRulesEvidence {
    pub schema_version:u32,pub project_id:String,pub number:u64,
    pub repository:Option<String>,pub pr_head_sha:Option<String>,
    pub base_branch:Option<String>,pub base_sha:Option<String>,
    pub state:&'static str,pub error_code:Option<&'static str>,
    pub required_approvals:Option<u64>,
    pub require_code_owners:Option<bool>,
    pub requires_linear_history:Option<bool>,
    pub required_status_contexts:Vec<String>,
    pub rulesets_covered:bool,pub protection_complete:bool,
    pub source_rechecked:bool,
    pub read_only:bool,pub credentials_used:bool,
    pub remote_modified:bool,pub merge_authorized:bool,
    pub installed_app_accepted:bool,
}
fn blank(project:&str,number:u64)->BranchRulesEvidence{
    BranchRulesEvidence {
        schema_version:1,project_id:project.to_owned(),number,
        repository:None,pr_head_sha:None,base_branch:None,base_sha:None,
        state:"unavailable",error_code:None,required_approvals:None,
        require_code_owners:None,requires_linear_history:None,
        required_status_contexts:vec![],rulesets_covered:false,
        protection_complete:false,source_rechecked:false,
        read_only:true,credentials_used:false,remote_modified:false,
        merge_authorized:false,installed_app_accepted:false,
    }
}
fn safe_branch(branch:&str)->bool{
    branch.len()>0 && branch.len()<=160 &&
    !branch.starts_with('/') && !branch.ends_with('/') &&
    !branch.contains("..") && !branch.contains("//") &&
    branch.bytes().all(|c|c.is_ascii_alphanumeric() ||
        matches!(c,b'_'|b'-'|b'.'|b'/'))
}
fn encode(s:&str)->String{
    let mut out=String::new();
    for c in s.bytes(){
        if c.is_ascii_alphanumeric() || matches!(c,b'_'|b'-'|b'.'){
            out.push(c as char);
        }else{out.push_str(&format!("%{c:02X}"));}
    }
    out
}
fn parse_rules(value:&Value)->Result<(Option<u64>,Option<bool>,Option<bool>,Vec<String>),&'static str>{
    if !value.is_object(){return Err("FIN03_RULES_SHAPE_UNKNOWN");}
    let required=value.get("required_pull_request_reviews")
        .and_then(|v|v.get("required_approving_review_count"))
        .and_then(Value::as_u64).filter(|v|*v<=6);
    let owners=value.get("required_pull_request_reviews")
        .and_then(|v|v.get("require_code_owner_reviews")).and_then(Value::as_bool);
    let linear=value.get("required_linear_history")
        .and_then(|v|v.get("enabled")).and_then(Value::as_bool);
    let mut contexts=Vec::new();
    if let Some(list)=value.get("required_status_checks")
        .and_then(|v|v.get("contexts")).and_then(Value::as_array){
        if list.len()>64{return Err("FIN03_TOO_MANY_RULE_CONTEXTS");}
        for item in list {
            let name=item.as_str().ok_or("FIN03_RULE_CONTEXT_INVALID")?;
            if name.is_empty() || name.len()>120 ||
                name.chars().any(char::is_control){
                return Err("FIN03_RULE_CONTEXT_UNTRUSTED");
            }
            contexts.push(name.to_owned());
        }
    }
    // Missing fields remain None. This API cannot describe all GitHub
    // rulesets, bypass actors, required deployments or private policies.
    Ok((required,owners,linear,contexts))
}
impl IdeWorkspaceRegistry{
    pub fn inspect_github_branch_rules(&self,project:&str,number:u64)
        -> BranchRulesEvidence{
        let mut out=blank(project,number);
        if number==0 || number>1_000_000_000 {
            out.error_code=Some("FIN03_PR_NUMBER_INVALID");return out;
        }
        let before=self.inspect_github_pull_requests(project);
        if !before.source_rechecked || before.listing_partial {
            out.error_code=Some("FIN03_PR_LIST_UNVERIFIED");return out;
        }
        let Some(pr)=before.pulls.iter().find(|p|p.number==number &&
            p.local_vs_pr_head=="same_commit") else{
            out.error_code=Some("FIN03_SAME_HEAD_PR_NOT_FOUND");return out;
        };
        if !safe_branch(&pr.base_branch) {
            out.error_code=Some("FIN03_BASE_BRANCH_INVALID");return out;
        }
        let Some(repository)=before.repository.as_deref() else{return out};
        out.repository=Some(repository.to_owned());
        out.pr_head_sha=Some(pr.head_sha.clone());
        out.base_branch=Some(pr.base_branch.clone());
        out.base_sha=Some(pr.base_sha.clone());
        let url=format!("https://api.github.com/repos/{repository}/branches/{}/protection",
            encode(&pr.base_branch));
        let agent=ureq::AgentBuilder::new().redirects(0)
            .timeout_connect(Duration::from_secs(4))
            .timeout_read(Duration::from_secs(6))
            .timeout_write(Duration::from_secs(4)).build();
        let response=match agent.get(&url)
            .set("User-Agent","Lu-IDE-Branch-rules/0.1")
            .set("Accept","application/vnd.github+json").call(){
            Ok(r)=>r,
            Err(_)=>{
                out.state="access_unavailable";
                out.error_code=Some("FIN03_PROTECTION_NOT_PUBLICLY_VERIFIABLE");
                return out;
            }
        };
        let mut bytes=Vec::new();
        if response.into_reader().take(MAX_BYTES+1)
            .read_to_end(&mut bytes).is_err() ||
            bytes.len() as u64>MAX_BYTES{
            out.error_code=Some("FIN03_PROTECTION_RESPONSE_TOO_LARGE");return out;
        }
        let parsed=match serde_json::from_slice::<Value>(&bytes){
            Ok(v)=>v,Err(_)=>{out.error_code=Some("FIN03_PROTECTION_JSON_INVALID");return out;}
        };
        let (n,owners,linear,contexts)=match parse_rules(&parsed){
            Ok(v)=>v,Err(c)=>{out.error_code=Some(c);return out;}
        };
        let after=self.inspect_github_pull_requests(project);
        if !after.source_rechecked || after.listing_partial ||
            after.repository!=before.repository ||
            after.current_branch!=before.current_branch ||
            after.local_head!=before.local_head ||
            !after.pulls.iter().any(|p|p.number==number &&
                p.head_sha==pr.head_sha &&
                p.base_sha==pr.base_sha &&
                p.base_branch==pr.base_branch &&
                p.local_vs_pr_head=="same_commit"){
            out.error_code=Some("FIN03_PR_MOVED_DURING_RULE_CHECK");return out;
        }
        out.required_approvals=n;
        out.require_code_owners=owners;
        out.requires_linear_history=linear;
        out.required_status_contexts=contexts;
        out.source_rechecked=true;
        out.state="partial_public_rule_observation";
        out
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn public_rule_read_cannot_be_full_merge_policy(){
        let value=serde_json::json!({"required_pull_request_reviews":{
            "required_approving_review_count":2,
            "require_code_owner_reviews":true},
            "required_linear_history":{"enabled":true},
            "required_status_checks":{"contexts":["build","test"]}});
        let (n,o,l,c)=parse_rules(&value).unwrap();
        assert_eq!(n,Some(2));assert_eq!(o,Some(true));
        assert_eq!(l,Some(true));assert_eq!(c.len(),2);
        let empty=parse_rules(&serde_json::json!({})).unwrap();
        assert!(empty.0.is_none());
        assert!(!safe_branch("../escape"));
    }
}
