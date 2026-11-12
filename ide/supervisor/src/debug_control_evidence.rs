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
 requires_result_reconciliation:bool,
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
/// Exact original supervisor journal-byte identity, separate from its
/// parsed job summaries. A same-phase append still invalidates an audit
/// read if it races the action inventory.
fn original_debug_journal_digest(profile:&Path)->Result<String,&'static str>{
 let file=profile.join("debug-fixture-receipts.jsonl");
 let meta=match fs::symlink_metadata(&file){
  Ok(meta)=>meta,
  Err(e) if e.kind()==std::io::ErrorKind::NotFound=>
   return Ok(sha(b"fin04-pristine-no-debug-journal-yet-v1")),
  Err(_)=>return Err("FIN04_DEBUG_JOURNAL_IDENTITY_MISSING"),
 };
 if !meta.is_file()||meta.file_type().is_symlink()||
    meta.len()>64*1024*1024{
  return Err("FIN04_DEBUG_JOURNAL_IDENTITY_UNTRUSTED");
 }
 #[cfg(windows)]{
  use std::os::windows::fs::MetadataExt;
  if meta.file_attributes()&0x400!=0{
   return Err("FIN04_DEBUG_JOURNAL_IDENTITY_REPARSE_DENIED");
  }
 }
 let mut content=Vec::new();
 File::open(&file).map_err(|_|"FIN04_DEBUG_JOURNAL_IDENTITY_UNREADABLE")?
  .take(64*1024*1024+1).read_to_end(&mut content)
  .map_err(|_|"FIN04_DEBUG_JOURNAL_IDENTITY_UNREADABLE")?;
 if content.len()>64*1024*1024 || content.len() as u64!=meta.len(){
  return Err("FIN04_DEBUG_JOURNAL_IDENTITY_CHANGED");
 }
 Ok(sha(&content))
}
/// Bounded complete action-directory byte witness. Read before AND after
/// the parsed action scan; a same-size rewrite, new file, removal or foreign
/// project's action changes invalidate the entire observation. No action
/// paths, file contents or foreign identifiers are returned to the renderer.
fn action_directory_fingerprint(profile:&Path)->Result<String,&'static str>{
 let mut entries=Vec::<(String,String)>::new();
 let mut total=0u64;
 for item in fs::read_dir(profile).map_err(|_|"FIN04_ACTION_DIRECTORY_UNREADABLE")?{
  let entry=item.map_err(|_|"FIN04_ACTION_DIRECTORY_UNREADABLE")?;
  let name=entry.file_name();
  let name=name.to_str().ok_or("FIN04_ACTION_FILENAME_INVALID")?;
  if !name.starts_with("lu-debug-action-"){continue;}
  let action=name.strip_prefix("lu-debug-action-")
   .and_then(|tail|tail.strip_suffix(".ndjson"))
   .ok_or("FIN04_ACTION_FILENAME_INVALID")?;
  if !hex64(action) || entries.len()>=MAX_ACTIONS {
   return Err("FIN04_ACTION_MANIFEST_LIMIT_OR_NAME");
  }
  let path=entry.path();
  let meta=fs::symlink_metadata(&path)
   .map_err(|_|"FIN04_ACTION_MANIFEST_FILE_MISSING")?;
  if !meta.is_file() || meta.file_type().is_symlink() ||
     meta.len()==0 || meta.len()>MAX_FILE{
   return Err("FIN04_ACTION_MANIFEST_UNTRUSTED_FILE");
  }
  #[cfg(windows)]{
   use std::os::windows::fs::MetadataExt;
   if meta.file_attributes()&0x400!=0 {
    return Err("FIN04_ACTION_MANIFEST_REPARSE_DENIED");
   }
  }
  total=total.checked_add(meta.len())
   .ok_or("FIN04_ACTION_MANIFEST_BYTE_BUDGET")?;
  if total>32*1024*1024 {
   return Err("FIN04_ACTION_MANIFEST_BYTE_BUDGET");
  }
  let mut raw=Vec::new();
  File::open(&path).map_err(|_|"FIN04_ACTION_MANIFEST_UNREADABLE")?
   .take(MAX_FILE+1).read_to_end(&mut raw)
   .map_err(|_|"FIN04_ACTION_MANIFEST_UNREADABLE")?;
  if raw.len() as u64!=meta.len(){
   return Err("FIN04_ACTION_MANIFEST_CHANGED");
  }
  entries.push((name.to_owned(),sha(&raw)));
 }
 entries.sort_by(|a,b|a.0.cmp(&b.0));
 if entries.windows(2).any(|pair|pair[0].0==pair[1].0){
  return Err("FIN04_ACTION_MANIFEST_DUPLICATE");
 }
 let mut digest=Sha256::new();
 digest.update(b"fin04-action-directory-fingerprint-v1\\n");
 for (name,hash) in entries{
  digest.update((name.len() as u64).to_le_bytes());
  digest.update(name.as_bytes());
  digest.update(hash.as_bytes());
 }
 Ok(format!("{:x}",digest.finalize()))
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
 let latest=fs::symlink_metadata(path)
  .map_err(|_|"FIN04_ACTION_JOURNAL_CHANGED_OR_TRUNCATED")?;
 if !latest.is_file()||latest.file_type().is_symlink()||
    latest.len()!=meta.len() ||
    latest.modified().ok()!=meta.modified().ok(){
  return Err("FIN04_ACTION_JOURNAL_CHANGED_OR_REPLACED");
 }
 #[cfg(unix)]{
  use std::os::unix::fs::MetadataExt;
  if latest.ino()!=meta.ino()||latest.dev()!=meta.dev(){
   return Err("FIN04_ACTION_JOURNAL_CHANGED_OR_REPLACED");
  }
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
  requires_result_reconciliation:end.phase=="ResultUncertain",
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
 pub independent_result_reconciliation_required:bool,
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
 /// Privacy-safe profile-wide blocker: original in-flight actions in ANY
 /// project deny new protected debugger admission without exposing identity.
 pub global_os_cleanup_required:bool,
 pub global_unresolved_action_required:bool,
 /// These hashes pin a read-only review to the original on-disk evidence;
 /// neither one is executable permission or current OS proof.
 pub original_debug_journal_sha256:String,
 pub action_manifest_sha256:String,
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
 let journal_before=original_debug_journal_digest(trusted_profile)?;
 let actions_before=action_directory_fingerprint(trusted_profile)?;
 let jobs=supervisor.inspect().map_err(|_|"FIN04_ORIGINAL_DEBUG_JOURNAL_UNVERIFIED")?;
 // The caller is a live in-process host; another process could have
 // appended its original journal since this Supervisor was opened.
 // Independently re-open and replay the same original on-disk book.
 let on_disk=Supervisor::open(
  trusted_profile.join("debug-fixture-receipts.jsonl"))
  .and_then(|reader|reader.inspect())
  .map_err(|_|"FIN04_ORIGINAL_DEBUG_JOURNAL_REPLAY_FAILED")?;
 if jobs.len()!=on_disk.len() ||
    !jobs.iter().zip(on_disk.iter()).all(|(a,b)|
      a.job_id==b.job_id && a.project_id==b.project_id &&
      a.plan_sha256==b.plan_sha256 && a.phase==b.phase &&
      a.requires_reconciliation==b.requires_reconciliation){
  return Err("FIN04_ORIGINAL_DEBUG_SUPERVISOR_STATE_STALE");
 }
 // A pristine profile can lack its first append-only journal; a missing
 // journal once any native Job exists is a hard integrity violation.
 let journal_exists=fs::symlink_metadata(
  trusted_profile.join("debug-fixture-receipts.jsonl")).is_ok();
 if !journal_exists && !jobs.is_empty(){
  return Err("FIN04_MISSING_NONEMPTY_ORIGINAL_DEBUG_JOURNAL");
 }
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
 let mut global_action_cleanup=false;
 let mut global_action_reconciliation=false;
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
  global_action_cleanup |= parsed.requires_os_cleanup;
  global_action_reconciliation |=
   parsed.requires_os_cleanup || parsed.requires_result_reconciliation;
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
   independent_result_reconciliation_required:parsed.requires_result_reconciliation,
   independent_os_result_claimed:parsed.phase=="ResultVerified",
   result_reverified_from_current_kernel:false,
   debugger_execution_authorized:false,replay_authorized:false,
  });
 }
 let journal_after=original_debug_journal_digest(trusted_profile)?;
 let actions_after=action_directory_fingerprint(trusted_profile)?;
 if actions_before!=actions_after{
  return Err("FIN04_ACTION_DIRECTORY_CHANGED_DURING_INSPECTION");
 }
 let fresh=supervisor.inspect().map_err(|_|"FIN04_ORIGINAL_DEBUG_JOURNAL_UNVERIFIED")?;
 let replayed=Supervisor::open(
  trusted_profile.join("debug-fixture-receipts.jsonl"))
  .and_then(|reader|reader.inspect())
  .map_err(|_|"FIN04_FINAL_ORIGINAL_DEBUG_REPLAY_FAILED")?;
 if replayed.len()!=fresh.len() ||
    !replayed.iter().zip(fresh.iter()).all(|(a,b)|
      a.job_id==b.job_id && a.project_id==b.project_id &&
      a.plan_sha256==b.plan_sha256 && a.phase==b.phase &&
      a.requires_reconciliation==b.requires_reconciliation){
  return Err("FIN04_FINAL_DEBUG_SUPERVISOR_STATE_STALE");
 }
 if journal_before!=journal_after || jobs.len()!=fresh.len() ||
    !jobs.iter().zip(fresh.iter()).all(|(before,after)|
      before.job_id==after.job_id &&
      before.project_id==after.project_id &&
      before.plan_sha256==after.plan_sha256 &&
      before.phase==after.phase &&
      before.requires_reconciliation==after.requires_reconciliation){
  return Err("FIN04_DEBUG_JOURNAL_CHANGED_DURING_ACTION_INSPECTION");
 }
 all.sort_by(|a,b|a.action_id_sha256.cmp(&b.action_id_sha256));
 let requires_os_cleanup=all.iter().any(|r|r.native_os_cleanup_required) ||
  jobs.iter().any(|j|j.project_id==project_id && (j.requires_reconciliation ||
     !matches!(j.phase,JobPhase::Completed|JobPhase::Cancelled|JobPhase::Failed|JobPhase::Denied)));
 // All journal files were verified; reveal only selected-project counts
 // to the renderer. Foreign project identities and activity remain private.
 let project_action_count=all.len();
 let project_job_count=jobs.iter().filter(|job|job.project_id==project_id).count();
 let global_os_cleanup_required=global_action_cleanup ||
  jobs.iter().any(|j|j.requires_reconciliation ||
    !matches!(j.phase,JobPhase::Completed|JobPhase::Cancelled|
      JobPhase::Failed|JobPhase::Denied));
 let truncated=all.len()>MAX_VISIBLE;
 let visible=all.len().min(MAX_VISIBLE);
 all.truncate(MAX_VISIBLE);
 Ok(DebugControlEvidenceInventory{
  project_id:project_id.into(),original_debug_jobs_checked:project_job_count,
  original_action_files_checked:project_action_count,
  original_action_files_visible:visible,
  truncated,requires_os_cleanup,global_os_cleanup_required,
  global_unresolved_action_required:global_action_reconciliation,
  original_debug_journal_sha256:journal_before,
  action_manifest_sha256:actions_before,
  journal_chain_verified:true,
  native_owner_event_live_verified:false,global_kernel_custody_accepted:false,
  installed_debugger_controls_accepted:false,
  debugger_execution_authorized:false,replay_authorized:false,rows:all,
 })
}

#[cfg(test)]
mod tests{
 use super::*;
 use crate::JobPlan;
 use std::{fs, path::PathBuf};
 fn fixture()->(PathBuf,Supervisor,String,String){
  let home=std::env::temp_dir().join(format!("fin04-native-audit-{:032x}",rand::random::<u128>()));
  fs::create_dir_all(&home).unwrap();
  let supervisor=Supervisor::open(home.join("debug-fixture-receipts.jsonl")).unwrap();
  let project=format!("ide-{}","a".repeat(32));
  let second=format!("ide-{}","b".repeat(32));
  (home,supervisor,project,second)
 }
 fn prepared(supervisor:&Supervisor,project:&str)->JobSummary{
  supervisor.prepare(JobPlan{
   project_id:project.to_owned(),recipe_id:"dap_lldb_fixed".into(),
   source_head:"a".repeat(40),input_snapshot_sha256:"b".repeat(64),
   manifest_sha256:"c".repeat(64),toolchain_sha256:"d".repeat(64),
   debug_target_binding_sha256:None,runtime_limit_ms:180000,
   memory_limit_bytes:512*1024*1024,process_limit:16,log_limit_bytes:65536,
   effect_class:"debug_inspection_isolated".into(),
  }).unwrap()
 }
 fn write_action(root:&Path,job:&JobSummary,phase:&str)->String{
  let action=sha(format!("job:{}",job.job_id).as_bytes());
  let preview=sha(b"review");
  let owner=sha(b"native owner");
  let mut events=Vec::<ActionEvent>::new();
  let chain=if phase=="ResultVerified" {
   vec![("Prepared","awaiting_fresh_native_operator".to_owned()),
    ("Approved","ci_only_not_installed_consent".into()),
    ("Consumed","one_use_slot_consumed_before_dispatch".into()),
    ("ResultVerified",format!("ci_independent_os_step:{}",sha(b"claim")))]
  }else{
   vec![("Prepared","awaiting_fresh_native_operator".to_owned())]
  };
  let mut prior="0".repeat(64);
  for (i,(phase,outcome)) in chain.into_iter().enumerate(){
   let mut row=ActionEvent{
    version:1,ordinal:(i+1) as u64,action_id_sha256:action.clone(),
    preview_sha256:preview.clone(),operation_sha256:sha(b"step-over"),
    job_id:job.job_id.clone(),plan_sha256:job.plan_sha256.clone(),
    owner_event_sha256:owner.clone(),owner_sequence:4,phase:phase.into(),
    outcome_code:outcome,observed_unix_ms:(i+1) as u64,
    previous_hash:prior,entry_hash:String::new(),
   };
   row.entry_hash=sha(&serde_json::to_vec(&row).unwrap());
   prior=row.entry_hash.clone();events.push(row);
  }
  let mut content=Vec::new();
  for e in &events{
   content.extend(serde_json::to_vec(e).unwrap());
   content.push(b'\n');
  }
  fs::write(root.join(format!("lu-debug-action-{action}.ndjson")),content).unwrap();
  action
 }
 #[test]
 fn release_read_only_empty_inventory_never_grants_execution(){
  let (root,supervisor,project,_)=fixture();
  let inventory=inspect_debug_control_evidence(&root,&supervisor,&project).unwrap();
  assert_eq!(inventory.original_action_files_checked,0);
  assert_eq!(inventory.original_debug_journal_sha256.len(),64);
  assert_eq!(inventory.action_manifest_sha256.len(),64);
  assert!(!inventory.global_os_cleanup_required);
  assert!(inventory.rows.is_empty());
  assert!(!inventory.debugger_execution_authorized);
  assert!(!inventory.replay_authorized);
  assert!(!inventory.global_kernel_custody_accepted);
  fs::remove_dir_all(root).unwrap();
 }
 #[test]
 fn original_hash_chained_action_links_to_the_exact_debugger_job(){
  let (root,supervisor,project,_)=fixture();
  let job=prepared(&supervisor,&project);
  let id=write_action(&root,&job,"Prepared");
  let inventory=inspect_debug_control_evidence(&root,&supervisor,&project).unwrap();
  assert_eq!(inventory.original_debug_jobs_checked,1);
  assert_eq!(inventory.original_action_files_checked,1);
  assert_eq!(inventory.rows.len(),1);
  assert_eq!(inventory.rows[0].action_id_sha256,id);
  assert_eq!(inventory.rows[0].job_id,job.job_id);
  assert_eq!(inventory.rows[0].phase,"Prepared");
  assert!(inventory.rows[0].native_os_cleanup_required);
  assert!(inventory.requires_os_cleanup);
  assert!(!inventory.rows[0].debugger_execution_authorized);
  fs::remove_dir_all(root).unwrap();
 }
 #[test]
 fn historical_claimed_result_is_not_current_kernel_or_replay_proof(){
  let (root,supervisor,project,_)=fixture();
  let job=prepared(&supervisor,&project);
  write_action(&root,&job,"ResultVerified");
  let inventory=inspect_debug_control_evidence(&root,&supervisor,&project).unwrap();
  assert_eq!(inventory.rows[0].phase,"ResultVerified");
  assert!(inventory.rows[0].independent_os_result_claimed);
  assert!(!inventory.rows[0].result_reverified_from_current_kernel);
  assert!(!inventory.installed_debugger_controls_accepted);
  assert!(!inventory.debugger_execution_authorized);
  fs::remove_dir_all(root).unwrap();
 }
 #[test]
 fn corrupt_foreign_action_is_denied_before_project_redaction(){
  let (root,supervisor,project,foreign)=fixture();
  let foreign_job=prepared(&supervisor,&foreign);
  let action=write_action(&root,&foreign_job,"Prepared");
  let file=root.join(format!("lu-debug-action-{action}.ndjson"));
  let mut bytes=fs::read(&file).unwrap();
  let index=bytes.iter().position(|b|*b==b'P').unwrap();
  bytes[index]=b'X';fs::write(&file,&bytes).unwrap();
  assert!(inspect_debug_control_evidence(&root,&supervisor,&project).is_err());
  fs::remove_dir_all(root).unwrap();
 }
 #[test]
 fn result_uncertain_requires_independent_result_reconciliation_not_os_claim(){
  let (root,supervisor,project,_)=fixture();
  let job=prepared(&supervisor,&project);
  let action=sha(format!("job:{}",job.job_id).as_bytes());
  let path=root.join(format!("lu-debug-action-{action}.ndjson"));
  let preview=sha(b"review");
  let owner=sha(b"native owner");
  let sequence=[
   ("Prepared","awaiting_fresh_native_operator".to_owned()),
   ("Approved","ci_only_not_installed_consent".into()),
   ("Consumed","one_use_slot_consumed_before_dispatch".into()),
   ("ResultUncertain","requires_independent_result_reconciliation_no_replay".into()),
  ];
  let mut previous="0".repeat(64);
  let mut output=Vec::new();
  for (i,(phase,outcome)) in sequence.iter().enumerate(){
   let mut event=ActionEvent{
    version:1,ordinal:(i+1) as u64,action_id_sha256:action.clone(),
    preview_sha256:preview.clone(),operation_sha256:sha(b"step-over"),
    job_id:job.job_id.clone(),plan_sha256:job.plan_sha256.clone(),
    owner_event_sha256:owner.clone(),owner_sequence:4,
    phase:(*phase).to_owned(),outcome_code:outcome.clone(),
    observed_unix_ms:(i+1) as u64,previous_hash:previous,
    entry_hash:String::new(),
   };
   event.entry_hash=sha(&serde_json::to_vec(&event).unwrap());
   previous=event.entry_hash.clone();
   output.extend(serde_json::to_vec(&event).unwrap());output.push(b'\n');
  }
  fs::write(&path,output).unwrap();
  let inventory=inspect_debug_control_evidence(&root,&supervisor,&project).unwrap();
  assert!(inventory.global_unresolved_action_required);
  assert!(inventory.global_os_cleanup_required);
  assert_eq!(inventory.rows[0].phase,"ResultUncertain");
  assert!(!inventory.rows[0].native_os_cleanup_required);
  assert!(inventory.rows[0].independent_result_reconciliation_required);
  assert!(!inventory.debugger_execution_authorized);
  fs::remove_dir_all(root).unwrap();
 }
 #[test]
 fn terminal_original_no_action_does_not_fabricate_recovery_blocker(){
  let (root,supervisor,project,_)=fixture();
  let job=prepared(&supervisor,&project);
  supervisor.cancel_before_start(&job.job_id).unwrap();
  let inventory=inspect_debug_control_evidence(&root,&supervisor,&project).unwrap();
  assert!(!inventory.global_os_cleanup_required);
  assert!(!inventory.requires_os_cleanup);
  assert_eq!(inventory.original_debug_jobs_checked,1);
  assert_eq!(inventory.original_action_files_checked,0);
  assert!(inventory.journal_chain_verified);
  assert!(!inventory.native_owner_event_live_verified);
  assert!(!inventory.debugger_execution_authorized);
  fs::remove_dir_all(root).unwrap();
 }
 #[test]
 fn in_memory_supervisor_rejects_original_disk_journal_advanced_by_other_host(){
  let (root,supervisor,project,_)=fixture();
  let second=Supervisor::open(root.join("debug-fixture-receipts.jsonl")).unwrap();
  let _job=prepared(&second,&project);
  assert_eq!(
   inspect_debug_control_evidence(&root,&supervisor,&project).unwrap_err(),
   "FIN04_ORIGINAL_DEBUG_SUPERVISOR_STATE_STALE"
  );
  fs::remove_dir_all(root).unwrap();
 }
 #[test]
 fn foreign_unfinished_action_blocks_global_debugger_without_disclosing_project(){
  let (root,supervisor,project,foreign)=fixture();
  let foreign_job=prepared(&supervisor,&foreign);
  write_action(&root,&foreign_job,"Prepared");
  let inventory=inspect_debug_control_evidence(&root,&supervisor,&project).unwrap();
  assert!(inventory.global_os_cleanup_required);
  assert!(!inventory.requires_os_cleanup);
  assert_eq!(inventory.original_debug_jobs_checked,0);
  assert_eq!(inventory.original_action_files_checked,0);
  assert!(inventory.rows.is_empty());
  assert!(!inventory.debugger_execution_authorized);
  fs::remove_dir_all(root).unwrap();
 }
 #[test]
 fn action_directory_fingerprint_changes_with_same_length_replacement(){
  let (root,supervisor,project,_)=fixture();
  let job=prepared(&supervisor,&project);
  let id=write_action(&root,&job,"Prepared");
  let baseline=action_directory_fingerprint(&root).unwrap();
  let file=root.join(format!("lu-debug-action-{id}.ndjson"));
  let mut raw=fs::read(&file).unwrap();
  let position=raw.iter().position(|b|*b==b'P').unwrap();
  raw[position]=b'R';
  fs::write(&file,raw).unwrap();
  let changed=action_directory_fingerprint(&root).unwrap();
  assert_ne!(baseline,changed);
  assert!(inspect_debug_control_evidence(&root,&supervisor,&project).is_err());
  fs::remove_dir_all(root).unwrap();
 }
 #[test]
 fn verified_action_manifest_hash_binds_all_original_action_bytes(){
  let (root,supervisor,project,_)=fixture();
  let job=prepared(&supervisor,&project);
  write_action(&root,&job,"Prepared");
  let first=inspect_debug_control_evidence(&root,&supervisor,&project).unwrap();
  let second=inspect_debug_control_evidence(&root,&supervisor,&project).unwrap();
  assert_eq!(first.original_debug_journal_sha256,second.original_debug_journal_sha256);
  assert_eq!(first.action_manifest_sha256,second.action_manifest_sha256);
  assert_eq!(first.action_manifest_sha256.len(),64);
  assert!(first.global_os_cleanup_required);
  assert!(first.requires_os_cleanup);
  fs::remove_dir_all(root).unwrap();
 }
 #[test]
 fn foreign_valid_action_is_not_project_visible(){
  let (root,supervisor,project,foreign)=fixture();
  let foreign_job=prepared(&supervisor,&foreign);
  write_action(&root,&foreign_job,"Prepared");
  let inventory=inspect_debug_control_evidence(&root,&supervisor,&project).unwrap();
  assert_eq!(inventory.original_action_files_checked,0);
  assert_eq!(inventory.original_action_files_visible,0);
  assert!(inventory.global_os_cleanup_required);
  assert!(inventory.rows.is_empty());
  fs::remove_dir_all(root).unwrap();
 }
}
