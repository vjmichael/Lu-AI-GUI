//! FIN-04 read-only inspection of ORIGINAL D6D native action receipts.
//! No DAP transport, consent constructor, replay, or execution authority.
//! A persisted action journal is audit evidence, never a live control slot.
use crate::{JobPhase,JobSummary,Supervisor};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{collections::BTreeSet,fs::{self,File},io::Read,path::Path};

const MAX_FILE:u64=32*1024;
const MAX_ACTIONS:usize=4096;
const MAX_EVENTS:usize=12;
const MAX_VISIBLE:usize=32;
fn sha(bytes:&[u8])->String {format!("{:x}",Sha256::digest(bytes))}
fn hex64(v:&str)->bool {v.len()==64 && v.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())}
fn job_id(v:&str)->bool {
 v.len()==36&&v.starts_with("job-")&&v[4..].bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionEvent{
 version:u32,ordinal:u64,action_id_sha256:String,
 preview_sha256:String,operation_sha256:String,job_id:String,
 plan_sha256:String,owner_event_sha256:String,owner_sequence:u64,
 phase:String,outcome_code:String,observed_unix_ms:u64,
 previous_hash:String,entry_hash:String,
}
#[derive(Debug,Clone)]
struct VerifiedAction {
 job_id:String,plan:String,action:String,preview:String,
 owner_event:String,sequence:u64,phase:String,last_hash:String,
 requires_os_cleanup:bool,
}
fn allowed_outcome(e:&ActionEvent)->bool{
 match e.phase.as_str(){
  "Prepared"=>e.outcome_code=="awaiting_fresh_native_operator",
  "Approved"=>matches!(e.outcome_code.as_str(),"ci_only_not_installed_consent"|"native_human_confirmed"),
  "Denied"=>e.outcome_code=="native_operator_declined",
  "Consumed"=>e.outcome_code=="one_use_slot_consumed_before_dispatch",
  "Revoked"=>matches!(e.outcome_code.as_str(),"wrong_owner_or_review"|"expired"|
    "stale_owner_or_preview"|"expired_before_consumption"|"stop_or_operator_revoked"),
  "ResultUncertain"=>e.outcome_code=="requires_independent_result_reconciliation_no_replay",
  "ResultVerified"=>e.outcome_code
    .strip_prefix("ci_independent_os_step:")
    .or_else(||e.outcome_code.strip_prefix("native_dialog_independent_os_step:"))
    .is_some_and(hex64),
  _=>false,
 }
}
fn read_verified(path:&Path,action:&str)->Result<VerifiedAction,&'static str>{
 let meta=fs::symlink_metadata(path).map_err(|_|"FIN04_ACTION_JOURNAL_UNAVAILABLE")?;
 if !meta.is_file() || meta.file_type().is_symlink() ||
     meta.len()==0 || meta.len()>MAX_FILE{
  return Err("FIN04_ACTION_JOURNAL_UNTRUSTED");
 }
 #[cfg(windows)]{
  use std::os::windows::fs::MetadataExt;
  if meta.file_attributes()&0x400!=0{return Err("FIN04_ACTION_JOURNAL_REPARSE_DENIED");}
 }
 let mut bytes=Vec::new();
 File::open(path).map_err(|_|"FIN04_ACTION_JOURNAL_UNREADABLE")?
  .take(MAX_FILE+1).read_to_end(&mut bytes)
  .map_err(|_|"FIN04_ACTION_JOURNAL_UNREADABLE")?;
 if bytes.is_empty() || bytes.len()>MAX_FILE as usize ||
    bytes.len() as u64 !=meta.len() || !bytes.ends_with(b"\n"){
  return Err("FIN04_ACTION_JOURNAL_CHANGED_OR_TRUNCATED");
 }
 let mut first:Option<ActionEvent>=None;
 let mut previous="0".repeat(64);
 let mut old_phase="";
 let mut last_time=None;
 let mut final_event=None;
 let mut count=0usize;
 for raw in bytes.split(|b|*b==b'\n').filter(|row|!row.is_empty()){
  count+=1;
  if count>MAX_EVENTS{return Err("FIN04_ACTION_JOURNAL_TOO_MANY_EVENTS");}
  let event:ActionEvent=serde_json::from_slice(raw)
   .map_err(|_|"FIN04_ACTION_JOURNAL_INVALID_RECORD")?;
  let mut digest_event=event.clone();
  digest_event.entry_hash.clear();
  let canonical=serde_json::to_vec(&digest_event)
   .map_err(|_|"FIN04_ACTION_JOURNAL_UNHASHABLE")?;
  if event.version!=1 || event.ordinal!=count as u64 ||
     !hex64(&event.entry_hash) || event.entry_hash!=sha(&canonical) ||
     event.previous_hash!=previous || event.action_id_sha256!=action ||
     !hex64(&event.preview_sha256) || !hex64(&event.operation_sha256) ||
     !hex64(&event.plan_sha256) || !hex64(&event.owner_event_sha256) ||
     !job_id(&event.job_id) || event.owner_sequence==0 ||
     !allowed_outcome(&event) ||
     last_time.is_some_and(|time|event.observed_unix_ms<time){
    return Err("FIN04_ACTION_JOURNAL_HASH_OR_IDENTITY_INVALID");
  }
  if let Some(original)=&first {
   if event.job_id!=original.job_id ||
      event.plan_sha256!=original.plan_sha256 ||
      event.operation_sha256!=original.operation_sha256 ||
      event.preview_sha256!=original.preview_sha256 ||
      event.owner_event_sha256!=original.owner_event_sha256 ||
      event.owner_sequence!=original.owner_sequence {
    return Err("FIN04_ACTION_JOURNAL_OWNER_CHANGED");
   }
  } else {
   first=Some(event.clone());
  }
  if !matches!((old_phase,event.phase.as_str()),
   ("","Prepared")|("Prepared","Approved")|("Prepared","Denied")|
   ("Prepared","Revoked")|("Approved","Consumed")|("Approved","Revoked")|
   ("Consumed","ResultVerified")|("Consumed","ResultUncertain")){
    return Err("FIN04_ACTION_JOURNAL_INVALID_TRANSITION");
  }
  previous=event.entry_hash.clone();
  last_time=Some(event.observed_unix_ms);
  old_phase=match event.phase.as_str(){
   "Prepared"=>"Prepared","Approved"=>"Approved","Denied"=>"Denied",
   "Consumed"=>"Consumed","Revoked"=>"Revoked",
   "ResultVerified"=>"ResultVerified","ResultUncertain"=>"ResultUncertain",
   _=>return Err("FIN04_ACTION_JOURNAL_UNSUPPORTED_PHASE"),
  };
  final_event=Some(event);
 }
 let first=first.ok_or("FIN04_ACTION_JOURNAL_EMPTY")?;
 let end=final_event.ok_or("FIN04_ACTION_JOURNAL_EMPTY")?;
 Ok(VerifiedAction{
  job_id:first.job_id,plan:first.plan_sha256,
  action:first.action_id_sha256,preview:first.preview_sha256,
  owner_event:first.owner_event_sha256,sequence:first.owner_sequence,
  phase:end.phase.clone(),last_hash:end.entry_hash,
  requires_os_cleanup:matches!(end.phase.as_str(),"Prepared"|"Approved"|"Consumed"),
 })
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct DebugControlEvidenceRow{
 pub job_id:String,
 pub action_id_sha256:String,
 pub preview_sha256:String,
 pub owner_event_sha256:String,
 pub owner_sequence:u64,
 pub phase:String,
 pub last_receipt_sha256:String,
 pub native_os_cleanup_required:bool,
 pub independent_os_result_claimed:bool,
 pub result_reverified_from_current_kernel:bool,
 pub debugger_execution_authorized:bool,
 pub replay_authorized:bool,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct DebugControlEvidenceInventory{
 pub project_id:String,
 pub original_debug_jobs_checked:usize,
 pub original_action_files_checked:usize,
 pub original_action_files_visible:usize,
 pub truncated:bool,
 pub requires_os_cleanup:bool,
 pub journal_chain_verified:bool,
 pub native_owner_event_live_verified:bool,
 pub global_kernel_custody_accepted:bool,
 pub installed_debugger_controls_accepted:bool,
 pub debugger_execution_authorized:bool,
 pub replay_authorized:bool,
 pub rows:Vec<DebugControlEvidenceRow>,
}
/// Replay the original global debugger journal first. Then independently
/// verify *each* private action log, including logs for foreign projects:
/// corruption outside the selected project is a denial, never an omission.
/// Scope/redaction is applied only after original Job/plan identity checks.
pub fn inspect_debug_control_evidence(
 trusted_profile:&Path,supervisor:&Supervisor,project_id:&str
)->Result<DebugControlEvidenceInventory,&'static str>{
 if !trusted_profile.is_absolute() || !project_id.starts_with("ide-") ||
    project_id.len()!=36 || !project_id[4..].bytes().all(|b|
      b.is_ascii_hexdigit()&&!b.is_ascii_uppercase()){
  return Err("FIN04_EVIDENCE_PROJECT_INVALID");
 }
 let root=fs::symlink_metadata(trusted_profile)
  .map_err(|_|"FIN04_EVIDENCE_PROFILE_MISSING")?;
 if !root.is_dir() || root.file_type().is_symlink(){
  return Err("FIN04_EVIDENCE_PROFILE_UNTRUSTED");
 }
 #[cfg(windows)]{
  use std::os::windows::fs::MetadataExt;
  if root.file_attributes()&0x400!=0{return Err("FIN04_EVIDENCE_PROFILE_REPARSE_DENIED");}
 }
 let jobs=supervisor.inspect().map_err(|_|"FIN04_ORIGINAL_DEBUG_JOURNAL_UNVERIFIED")?;
 if jobs.len()>8192{return Err("FIN04_ORIGINAL_DEBUG_JOB_LIMIT");}
 let mut identities=BTreeSet::new();
 for j in &jobs{
  if !job_id(&j.job_id) || !hex64(&j.plan_sha256) ||
     !identities.insert(j.job_id.as_str()){
   return Err("FIN04_DEBUG_JOB_IDENTITY_UNVERIFIABLE");
  }
 }
 let mut seen=BTreeSet::new();
 let mut all=Vec::new();
 let mut checked=0usize;
 for item in fs::read_dir(trusted_profile).map_err(|_|"FIN04_ACTION_DIRECTORY_UNREADABLE")? {
  let entry=item.map_err(|_|"FIN04_ACTION_DIRECTORY_UNREADABLE")?;
  let name=entry.file_name();
  let name=name.to_str().ok_or("FIN04_ACTION_FILENAME_INVALID")?;
  if !name.starts_with("lu-debug-action-"){continue;}
  let action=name.strip_prefix("lu-debug-action-")
   .and_then(|tail|tail.strip_suffix(".ndjson"))
   .ok_or("FIN04_ACTION_FILENAME_INVALID")?;
  if !hex64(action) || !seen.insert(action.to_owned()) {
   return Err("FIN04_ACTION_FILENAME_INVALID");
  }
  checked+=1;
  if checked>MAX_ACTIONS{return Err("FIN04_ACTION_FILE_LIMIT");}
  let parsed=read_verified(&entry.path(),action)?;
  let original=jobs.iter().find(|job|job.job_id==parsed.job_id)
   .ok_or("FIN04_ACTION_ORIGINAL_JOB_MISSING")?;
  if original.plan_sha256!=parsed.plan {
   return Err("FIN04_ACTION_ORIGINAL_PLAN_MISMATCH");
  }
  if parsed.requires_os_cleanup && !original.requires_reconciliation &&
      matches!(original.phase,JobPhase::Cancelled|JobPhase::Completed|
        JobPhase::Failed|JobPhase::Denied){
   // A terminal job does not erase an in-flight native action; it remains
   // original audit evidence requiring separate OS-only action cleanup.
  }
  if original.project_id!=project_id {continue;}
  all.push(DebugControlEvidenceRow{
   job_id:parsed.job_id,action_id_sha256:parsed.action,
   preview_sha256:parsed.preview,
   owner_event_sha256:parsed.owner_event,
   owner_sequence:parsed.sequence,phase:parsed.phase.clone(),
   last_receipt_sha256:parsed.last_hash,
   native_os_cleanup_required:parsed.requires_os_cleanup,
   independent_os_result_claimed:parsed.phase=="ResultVerified",
   result_reverified_from_current_kernel:false,
   debugger_execution_authorized:false,replay_authorized:false,
  });
 }
 all.sort_by(|a,b|a.action_id_sha256.cmp(&b.action_id_sha256));
 let requires_os_cleanup=all.iter().any(|r|r.native_os_cleanup_required) ||
  jobs.iter().any(|j|j.project_id==project_id && (j.requires_reconciliation ||
     !matches!(j.phase,JobPhase::Completed|JobPhase::Cancelled|JobPhase::Failed|JobPhase::Denied)));
 let truncated=all.len()>MAX_VISIBLE;
 let visible=all.len().min(MAX_VISIBLE);
 all.truncate(MAX_VISIBLE);
 Ok(DebugControlEvidenceInventory{
  project_id:project_id.into(),original_debug_jobs_checked:jobs.len(),
  original_action_files_checked:checked,original_action_files_visible:visible,
  truncated,requires_os_cleanup,journal_chain_verified:true,
  native_owner_event_live_verified:false,global_kernel_custody_accepted:false,
  installed_debugger_controls_accepted:false,
  debugger_execution_authorized:false,replay_authorized:false,rows:all,
 })
}
