//! FIN-03 / 6E-02: public, read-only PR review state. Never merge authority.
use crate::IdeWorkspaceRegistry;
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, io::Read, time::{Duration,SystemTime,UNIX_EPOCH}};
const LIMIT:u64=160*1024;
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct ReviewStateEvidence {
 pub schema_version:u32,pub project_id:String,pub number:u64,
 pub state:&'static str,pub head_sha:Option<String>,
 pub current_approvals:usize,pub changes_requested:usize,
 pub stale_reviewers:usize,pub listing_partial:bool,
 pub source_rechecked:bool,pub error_code:Option<&'static str>,
 pub branch_policy_verified:bool,pub merge_authorized:bool,
 pub review_submitted:bool,pub credentials_used:bool,
 pub remote_modified:bool,pub installed_app_accepted:bool,
}
fn blank(project:&str,n:u64)->ReviewStateEvidence{
 ReviewStateEvidence{schema_version:1,project_id:project.to_owned(),number:n,
 state:"unavailable",head_sha:None,current_approvals:0,changes_requested:0,
 stale_reviewers:0,listing_partial:false,source_rechecked:false,error_code:None,
 branch_policy_verified:false,merge_authorized:false,review_submitted:false,
 credentials_used:false,remote_modified:false,installed_app_accepted:false}
}
fn parse(value:&Value,head:&str,partial:bool)->Result<(usize,usize,usize),&'static str>{
 let items=value.as_array().ok_or("G06E02_RESPONSE_SHAPE")?;
 if items.len()>100{return Err("G06E02_REVIEW_LIMIT")}
 let mut reviewers:BTreeMap<u64,(u64,&str,bool)>=BTreeMap::new();
 for item in items {
  let id=item.get("id").and_then(Value::as_u64).filter(|v|*v>0)
    .ok_or("G06E02_REVIEW_ID")?;
  let user=item.get("user").and_then(|v|v.get("id"))
    .and_then(Value::as_u64).filter(|v|*v>0)
    .ok_or("G06E02_REVIEW_USER")?;
  let state=item.get("state").and_then(Value::as_str)
    .ok_or("G06E02_REVIEW_STATE")?;
  if !matches!(state,"APPROVED"|"CHANGES_REQUESTED"|"DISMISSED"|
      "COMMENTED"|"PENDING"){return Err("G06E02_UNKNOWN_REVIEW_STATE")}
  let commit=item.get("commit_id").and_then(Value::as_str)
    .ok_or("G06E02_REVIEW_COMMIT")?;
  if commit.len()!=40 || !commit.bytes().all(|b|b.is_ascii_hexdigit()){
    return Err("G06E02_REVIEW_COMMIT_INVALID")
  }
  let current=commit.eq_ignore_ascii_case(head);
  if reviewers.get(&user).is_none_or(|(prev,_,_)|id>*prev){
   reviewers.insert(user,(id,state,current));
  }
 }
 let mut approved=0;let mut changes=0;let mut stale=0;
 for (_,state,current) in reviewers.into_values(){
  if !current{stale+=1;continue}
  match state{"APPROVED"=>approved+=1,"CHANGES_REQUESTED"=>changes+=1,_=>{}}
 }
 // Later unseen pages could dismiss or supersede current reviews.
 if partial {return Ok((0,0,stale))}
 Ok((approved,changes,stale))
}
impl IdeWorkspaceRegistry {
 pub fn inspect_github_review_state(&self,project:&str,number:u64)->ReviewStateEvidence{
  let mut out=blank(project,number);
  if number==0 || number>1_000_000_000{
   out.error_code=Some("G06E02_PR_NUMBER_DENIED");return out
  }
  let before=self.inspect_github_pull_requests(project);
  if !before.source_rechecked || before.listing_partial{
   out.error_code=Some("G06E02_ORIGIN_PR_UNVERIFIED");return out
  }
  let Some(pr)=before.pulls.iter().find(|p|
    p.number==number && p.local_vs_pr_head=="same_commit") else{
   out.error_code=Some("G06E02_CURRENT_PR_MISSING");return out
  };
  let Some(repository)=before.repository.as_deref() else{return out};
  let head=pr.head_sha.clone();out.head_sha=Some(head.clone());
  let url=format!("https://api.github.com/repos/{repository}/pulls/{number}/reviews?per_page=100");
  let agent=ureq::AgentBuilder::new().redirects(0)
    .timeout_connect(Duration::from_secs(4))
    .timeout_read(Duration::from_secs(6))
    .timeout_write(Duration::from_secs(4)).build();
  let response=match agent.get(&url)
    .set("User-Agent","Lu-IDE-review/0.1")
    .set("Accept","application/vnd.github+json").call(){
      Ok(r)=>r,Err(_)=>{out.error_code=Some("G06E02_PUBLIC_NETWORK_UNAVAILABLE");return out}
    };
  out.listing_partial=response.header("Link")
    .is_some_and(|v|v.contains("rel=\"next\""));
  let mut bytes=Vec::new();
  if response.into_reader().take(LIMIT+1).read_to_end(&mut bytes).is_err() ||
      bytes.len() as u64>LIMIT {
    out.error_code=Some("G06E02_BODY_DENIED");return out
  }
  let parsed=match serde_json::from_slice::<Value>(&bytes){
   Ok(v)=>v,Err(_)=>{out.error_code=Some("G06E02_JSON_DENIED");return out}
  };
  let (a,c,s)=match parse(&parsed,&head,out.listing_partial){
   Ok(v)=>v,Err(code)=>{out.error_code=Some(code);return out}
  };
  let after=self.inspect_github_pull_requests(project);
  if !after.source_rechecked || after.repository!=before.repository ||
     after.current_branch!=before.current_branch ||
     after.local_head!=before.local_head ||
     !after.pulls.iter().any(|p|p.number==number &&
       p.head_sha==head && p.local_vs_pr_head=="same_commit"){
   out.error_code=Some("G06E02_SOURCE_CHANGED");return out
  }
  out.source_rechecked=true;
  out.current_approvals=a;out.changes_requested=c;out.stale_reviewers=s;
  out.state=if out.listing_partial{"partial_review_listing"}
    else if c>0{"changes_requested_observed"}
    else if a>0{"approvals_observed_policy_unverified"}
    else{"no_current_approvals_observed"};
  let _observed=SystemTime::now().duration_since(UNIX_EPOCH).ok();
  out
 }
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn last_review_and_stale_commit_are_never_approval(){
  let sha="a".repeat(40);
  let v=serde_json::json!([
   {"id":1,"user":{"id":8},"state":"APPROVED","commit_id":sha},
   {"id":2,"user":{"id":8},"state":"DISMISSED","commit_id":sha},
   {"id":3,"user":{"id":9},"state":"APPROVED","commit_id":"b".repeat(40)}
  ]);
  assert_eq!(parse(&v,&"a".repeat(40),false).unwrap(),(0,0,1));
  assert_eq!(parse(&v,&"a".repeat(40),true).unwrap().0,0);
 }
}
