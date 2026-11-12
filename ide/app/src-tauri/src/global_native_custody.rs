//! FIN-01 cross-supervisor admission fence.
//! This profile-wide marker serializes build/package and debugger admission
//! across *processes*, not merely separate Rust mutexes. It is NOT an OS
//! worker lease or permission to execute. Interrupted or uncertain operations
//! deliberately leave a marker for operator-reviewed recovery.
use std::{fs::{self,File,OpenOptions},io::{Read,Write},path::{Path,PathBuf}};
use lu_ide_execution_supervisor::{JobPhase,JobSummary};
const MARKER:&str="fin01-global-native-custody-v1.lock";
const MAX_MARKER:u64=128;
fn marker_path(profile:&Path)->PathBuf {profile.join(MARKER)}
fn valid_profile(profile:&Path)->Result<(),String>{
 let meta=fs::symlink_metadata(profile).map_err(|_|"FIN01_PROFILE_UNAVAILABLE")?;
 if !profile.is_absolute() || !meta.is_dir() || meta.file_type().is_symlink(){
  return Err("FIN01_PROFILE_UNTRUSTED".into());
 }
 #[cfg(windows)] {
  use std::os::windows::fs::MetadataExt;
  if meta.file_attributes()&0x400!=0{return Err("FIN01_PROFILE_REPARSE_DENIED".into());}
 }
 Ok(())
}
fn marker_present(profile:&Path)->Result<bool,String>{
 valid_profile(profile)?;
 match fs::symlink_metadata(marker_path(profile)){
  Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(false),
  Err(_)=>Err("FIN01_GLOBAL_MARKER_UNREADABLE".into()),
  Ok(meta)=>{
   if !meta.is_file() || meta.file_type().is_symlink() ||
      meta.len()==0 || meta.len()>MAX_MARKER {
    return Err("FIN01_GLOBAL_MARKER_UNTRUSTED".into());
   }
   #[cfg(windows)] {
    use std::os::windows::fs::MetadataExt;
    if meta.file_attributes()&0x400!=0 {
      return Err("FIN01_GLOBAL_MARKER_REPARSE_DENIED".into());
    }
   }
   Ok(true)
  }
 }
}
pub fn global_custody_unresolved(profile:&Path)->Result<bool,String>{
 marker_present(profile)
}
pub fn durable_busy(rows:&[JobSummary])->bool{
 rows.iter().any(|row|row.requires_reconciliation ||
  !matches!(row.phase,JobPhase::Completed|JobPhase::Cancelled|JobPhase::Failed|JobPhase::Denied))
}
pub struct GlobalAdmissionFence {
 marker:PathBuf,
 owner_bytes:Vec<u8>,
 released:bool,
}
impl GlobalAdmissionFence {
 pub fn acquire(profile:&Path,domain:&'static str)->Result<Self,String>{
  valid_profile(profile)?;
  if !matches!(domain,"build"|"debug"){return Err("FIN01_UNKNOWN_DOMAIN".into());}
  let marker=marker_path(profile);
  // create_new is the cross-process linearization point. Never unlink an
  // existing marker, regardless of its age or nominal owning PID.
  let mut file=OpenOptions::new().create_new(true).write(true).open(&marker)
   .map_err(|_|"FIN01_GLOBAL_ADMISSION_OCCUPIED_OR_UNCERTAIN".to_owned())?;
  #[cfg(unix)]{
    use std::os::unix::fs::PermissionsExt;
    if fs::set_permissions(&marker,fs::Permissions::from_mode(0o600)).is_err(){
      return Err("FIN01_GLOBAL_MARKER_PERMISSIONS".into());
    }
  }
  let owner_bytes=format!("FIN01-V1:{}:{}:{:016x}\n",domain,std::process::id(),rand::random::<u64>()).into_bytes();
  file.write_all(&owner_bytes).and_then(|_|file.sync_all())
   .map_err(|_|"FIN01_GLOBAL_MARKER_SYNC_FAILED".to_owned())?;
  #[cfg(unix)]
  File::open(profile).and_then(|dir|dir.sync_all())
   .map_err(|_|"FIN01_GLOBAL_MARKER_DIRECTORY_SYNC_FAILED".to_owned())?;
  Ok(Self{marker,owner_bytes,released:false})
 }
 /// Called only after every known original supervisor job is terminal and
 /// independently reconciled. There is no automatic release on Drop.
 pub fn release_if_terminal(&mut self,own:&[JobSummary],other:&[JobSummary])
  ->Result<(),String>{
  if durable_busy(own) || durable_busy(other){
   return Err("FIN01_GLOBAL_CUSTODY_NOT_RECONCILED".into());
  }
  let metadata=fs::symlink_metadata(&self.marker)
   .map_err(|_|"FIN01_GLOBAL_MARKER_MISSING")?;
  if !metadata.is_file() || metadata.file_type().is_symlink() ||
     metadata.len()!=self.owner_bytes.len() as u64{
   return Err("FIN01_GLOBAL_MARKER_CHANGED".into());
  }
  let mut bytes=Vec::new();
  File::open(&self.marker).map_err(|_|"FIN01_GLOBAL_MARKER_UNREADABLE")?
   .take(MAX_MARKER+1).read_to_end(&mut bytes)
   .map_err(|_|"FIN01_GLOBAL_MARKER_UNREADABLE")?;
  if bytes!=self.owner_bytes{return Err("FIN01_GLOBAL_MARKER_CHANGED".into());}
  // Compare a second time immediately before unlink. A replaced marker may
  // never be mistaken for our original one-use admission ticket.
  let latest=fs::symlink_metadata(&self.marker)
   .map_err(|_|"FIN01_GLOBAL_MARKER_CHANGED")?;
  if !latest.is_file() || latest.file_type().is_symlink() ||
     latest.len()!=metadata.len() || latest.modified().ok()!=metadata.modified().ok(){
    return Err("FIN01_GLOBAL_MARKER_CHANGED".into());
  }
  fs::remove_file(&self.marker).map_err(|_|"FIN01_GLOBAL_MARKER_CLEAR_FAILED")?;
  #[cfg(unix)]
  {
   let parent=self.marker.parent().ok_or("FIN01_GLOBAL_MARKER_PARENT_MISSING")?;
   File::open(parent).and_then(|dir|dir.sync_all())
    .map_err(|_|"FIN01_GLOBAL_MARKER_REMOVAL_SYNC_FAILED")?;
  }
  self.released=true;
  Ok(())
 }
 pub fn is_released(&self)->bool{self.released}
}

pub fn sibling_rows(profile:&Path,domain:&str)->Result<Vec<JobSummary>,String>{
 valid_profile(profile)?;
 let file=match domain{
  "build"=>"debug-fixture-receipts.jsonl",
  "debug"=>"build-job-receipts.jsonl",
  _=>return Err("FIN01_UNKNOWN_DOMAIN".into()),
 };
 lu_ide_execution_supervisor::Supervisor::open(profile.join(file))
  .and_then(|s|s.inspect())
  .map_err(|_|"FIN01_SIBLING_JOURNAL_UNVERIFIED".into())
}

#[cfg(test)]
mod tests{
 use super::*;
 use std::{sync::{Arc,Barrier},thread};
 fn fixture()->PathBuf{
  let p=std::env::temp_dir().join(format!("fin01-global-{}-{:?}",
    std::process::id(),std::thread::current().id()));
  let _=fs::remove_dir_all(&p);
  fs::create_dir_all(&p).unwrap();
  p
 }
 fn job(phase:JobPhase,recovery:bool)->JobSummary{
  JobSummary{job_id:format!("job-{}","a".repeat(32)),
   project_id:format!("ide-{}","b".repeat(32)),
   plan_sha256:"c".repeat(64),phase,requires_reconciliation:recovery,
   execution_verified:false}
 }
 #[test]
 fn only_one_supervisor_claims_profile_even_across_threads(){
  let p=fixture();
  let lease=GlobalAdmissionFence::acquire(&p,"build").unwrap();
  let barrier=Arc::new(Barrier::new(2));
  let b=Arc::clone(&barrier);let x=p.clone();
  let competing=thread::spawn(move||{
   b.wait();
   GlobalAdmissionFence::acquire(&x,"debug").err()
  });
  barrier.wait();
  assert_eq!(competing.join().unwrap().as_deref(),
   Some("FIN01_GLOBAL_ADMISSION_OCCUPIED_OR_UNCERTAIN"));
  assert!(global_custody_unresolved(&p).unwrap());
  let mut lease=lease;
  assert!(lease.release_if_terminal(&[],&[]).is_ok());
  assert!(!global_custody_unresolved(&p).unwrap());
  drop(lease);fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn marker_survives_unwind_or_restart_without_auto_regrant(){
  let p=fixture();
  let lease=GlobalAdmissionFence::acquire(&p,"debug").unwrap();
  drop(lease);
  assert!(global_custody_unresolved(&p).unwrap());
  assert!(GlobalAdmissionFence::acquire(&p,"build").is_err());
  fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn terminal_cleanup_refuses_prepared_running_or_quarantined_sibling(){
  let p=fixture();
  let mut lease=GlobalAdmissionFence::acquire(&p,"build").unwrap();
  for (phase,recovery) in [
   (JobPhase::Prepared,false),(JobPhase::Running,false),
   (JobPhase::Failed,true),(JobPhase::CancelRequested,true)
  ]{
   assert_eq!(lease.release_if_terminal(&[],&[job(phase,recovery)])
     .unwrap_err(),"FIN01_GLOBAL_CUSTODY_NOT_RECONCILED");
   assert!(global_custody_unresolved(&p).unwrap());
  }
  assert!(lease.release_if_terminal(
   &[job(JobPhase::Completed,false)],
   &[job(JobPhase::Cancelled,false)]).is_ok());
  assert!(lease.is_released());
  assert!(!global_custody_unresolved(&p).unwrap());
  fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn recovery_assessment_preserves_os_uncertainty_after_clean_journals(){
  let p=fixture();
  let initial=assess_global_recovery(&p).unwrap();
  assert!(!initial.marker_present);
  assert!(initial.original_journals_reconciled);
  assert!(!initial.marker_removal_authorized);
  let lease=GlobalAdmissionFence::acquire(&p,"debug").unwrap();
  drop(lease);
  let interrupted=assess_global_recovery(&p).unwrap();
  assert!(interrupted.marker_present);
  assert_eq!(interrupted.marker_domain.as_deref(),Some("debug"));
  assert!(interrupted.original_journals_reconciled);
  assert_eq!(interrupted.code,"FIN01_ORIGINAL_OS_RELEASE_PROOF_REQUIRED");
  assert!(!interrupted.native_os_release_independently_verified);
  assert!(!interrupted.auto_retry_authorized);
  assert!(!interrupted.marker_removal_authorized);
  fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn marker_corruption_cannot_be_silently_reconciled(){
  let p=fixture();
  let mut lease=GlobalAdmissionFence::acquire(&p,"build").unwrap();
  fs::write(marker_path(&p),b"forged owner").unwrap();
  assert_eq!(lease.release_if_terminal(&[],&[]).unwrap_err(),
   "FIN01_GLOBAL_MARKER_CHANGED");
  assert!(global_custody_unresolved(&p).unwrap());
  fs::remove_dir_all(p).unwrap();
 }
}

#[derive(Debug,Clone,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub struct RecoveryAssessment {
 pub marker_present:bool,
 pub marker_domain:Option<String>,
 pub marker_owner_pid:Option<u32>,
 pub build_journal_pending:usize,
 pub debug_journal_pending:usize,
 pub original_journals_reconciled:bool,
 pub native_os_release_independently_verified:bool,
 pub operator_review_required:bool,
 pub marker_removal_authorized:bool,
 pub auto_retry_authorized:bool,
 pub code:&'static str,
}
/// Read-only host assessment; a serialized assessment is not an authorization
/// token and cannot clear the global marker or acknowledge OS process release.
pub fn assess_global_recovery(profile:&Path)->Result<RecoveryAssessment,String>{
 let present=marker_present(profile)?;
 let mut domain=None;
 let mut pid=None;
 if present {
  let bytes=fs::read(marker_path(profile))
   .map_err(|_|"FIN01_GLOBAL_MARKER_UNREADABLE")?;
  let marker=std::str::from_utf8(&bytes)
   .map_err(|_|"FIN01_GLOBAL_MARKER_FORMAT_INVALID")?;
  let body=marker.strip_suffix('\n')
   .ok_or("FIN01_GLOBAL_MARKER_FORMAT_INVALID")?;
  if body.contains('\n') || body.contains('\r'){
   return Err("FIN01_GLOBAL_MARKER_FORMAT_INVALID".into());
  }
  let parts:Vec<_>=body.split(':').collect();
  if parts.len()!=4 || parts[0]!="FIN01-V1" ||
     !matches!(parts[1],"build"|"debug") ||
     parts[3].len()!=16 ||
     !parts[3].bytes().all(|x|x.is_ascii_hexdigit() && !x.is_ascii_uppercase()){
      return Err("FIN01_GLOBAL_MARKER_FORMAT_INVALID".into());
  }
  let parsed=parts[2].parse::<u32>()
   .map_err(|_|"FIN01_GLOBAL_MARKER_OWNER_INVALID")?;
  if parsed==0{return Err("FIN01_GLOBAL_MARKER_OWNER_INVALID".into());}
  domain=Some(parts[1].to_owned());pid=Some(parsed);
 }
 let build=lu_ide_execution_supervisor::Supervisor::open(
     profile.join("build-job-receipts.jsonl"))
   .and_then(|s|s.inspect())
   .map_err(|_|"FIN01_BUILD_JOURNAL_UNVERIFIED")?;
 let debug=lu_ide_execution_supervisor::Supervisor::open(
     profile.join("debug-fixture-receipts.jsonl"))
   .and_then(|s|s.inspect())
   .map_err(|_|"FIN01_DEBUG_JOURNAL_UNVERIFIED")?;
 let pending=|rows:&[JobSummary]|rows.iter().filter(|x|x.requires_reconciliation ||
    !matches!(x.phase,JobPhase::Completed|JobPhase::Cancelled|
      JobPhase::Failed|JobPhase::Denied)).count();
 let build_pending=pending(&build);
 let debug_pending=pending(&debug);
 let reconciled=build_pending==0 && debug_pending==0;
 Ok(RecoveryAssessment{
  marker_present:present,marker_domain:domain,marker_owner_pid:pid,
  build_journal_pending:build_pending,debug_journal_pending:debug_pending,
  original_journals_reconciled:reconciled,
  native_os_release_independently_verified:false,
  operator_review_required:present || !reconciled,
  marker_removal_authorized:false,auto_retry_authorized:false,
  code:if !present && reconciled{"FIN01_GLOBAL_NO_UNRESOLVED_CUSTODY_OBSERVED"}
   else if !reconciled{"FIN01_ORIGINAL_JOURNAL_RECOVERY_REQUIRED"}
   else{"FIN01_ORIGINAL_OS_RELEASE_PROOF_REQUIRED"},
 })
}
