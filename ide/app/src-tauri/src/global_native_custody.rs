//! FIN-01 cross-supervisor admission fence.
//! This profile-wide marker serializes build/package and debugger admission
//! across *processes*, not merely separate Rust mutexes. It is NOT an OS
//! worker lease or permission to execute. Interrupted or uncertain operations
//! deliberately leave a marker for operator-reviewed recovery.
use std::{fs::{self,File,OpenOptions},io::{Read,Write},path::{Path,PathBuf}};
use lu_ide_execution_supervisor::{JobPhase,JobSummary};
use sha2::{Digest,Sha256};
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
/// Exact bytes of an original append-only receipt journal, bounded for
/// trust. This is NOT an independent OS process-exit witness.
fn journal_fingerprint(profile:&Path,domain:&str)->Result<Option<String>,String>{
 valid_profile(profile)?;
 let name=DOMAINS.iter().find(|(id,_)|*id==domain)
  .map(|(_,name)|*name).ok_or("FIN01_UNKNOWN_DOMAIN")?;
 let path=profile.join(name);
 let meta=match fs::symlink_metadata(&path){
  Ok(meta)=>meta,
  Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),
  Err(_)=>return Err("FIN01_JOURNAL_IDENTITY_UNREADABLE".into()),
 };
 if !meta.is_file() || meta.file_type().is_symlink() ||
    meta.len()>64*1024*1024 {
  return Err("FIN01_JOURNAL_IDENTITY_UNTRUSTED".into());
 }
 let file=File::open(&path).map_err(|_|"FIN01_JOURNAL_IDENTITY_UNREADABLE")?;
 let mut reader=file.take(64*1024*1024+1);
 let mut digest=Sha256::new();
 let mut total=0u64;
 let mut chunk=[0u8;32768];
 loop {
  let read=reader.read(&mut chunk).map_err(|_|"FIN01_JOURNAL_IDENTITY_UNREADABLE")?;
  if read==0 {break;}
  total+=read as u64;
  if total>64*1024*1024{return Err("FIN01_JOURNAL_IDENTITY_UNTRUSTED".into());}
  digest.update(&chunk[..read]);
 }
 if total!=meta.len(){return Err("FIN01_JOURNAL_IDENTITY_CHANGED".into());}
 Ok(Some(format!("{:x}",digest.finalize())))
}

pub struct GlobalAdmissionFence {
 marker:PathBuf,
 owner_bytes:Vec<u8>,
 domain:&'static str,
 journal_identity:Option<String>,
 released:bool,
}
impl GlobalAdmissionFence {
 pub fn acquire(profile:&Path,domain:&'static str)->Result<Self,String>{
  valid_profile(profile)?;
  if !matches!(domain,"build"|"debug"|"lsp"){return Err("FIN01_UNKNOWN_DOMAIN".into());}
  let baseline=journal_fingerprint(profile,domain)?;
  let marker=marker_path(profile);
  // create_new is the cross-process linearization point. Never unlink an
  // existing marker, regardless of its age or nominal owning PID.
  let mut options=OpenOptions::new();
  options.create_new(true).write(true);
  // The marker must never be briefly world-readable before chmod.
  #[cfg(unix)]{
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  let mut file=options.open(&marker)
   .map_err(|_|"FIN01_GLOBAL_ADMISSION_OCCUPIED_OR_UNCERTAIN".to_owned())?;
  let owner_bytes=format!("FIN01-V1:{}:{}:{:016x}\n",domain,std::process::id(),rand::random::<u64>()).into_bytes();
  file.write_all(&owner_bytes).and_then(|_|file.sync_all())
   .map_err(|_|"FIN01_GLOBAL_MARKER_SYNC_FAILED".to_owned())?;
  #[cfg(unix)]
  File::open(profile).and_then(|dir|dir.sync_all())
   .map_err(|_|"FIN01_GLOBAL_MARKER_DIRECTORY_SYNC_FAILED".to_owned())?;
  if journal_fingerprint(profile,domain)?!=baseline {
   return Err("FIN01_OWNER_JOURNAL_CHANGED_DURING_ACQUISITION".into());
  }
  Ok(Self{marker,owner_bytes,domain,journal_identity:baseline,released:false})
 }
 /// Called only after every known original supervisor job is terminal and
 /// independently reconciled. There is no automatic release on Drop.
 pub fn release_if_terminal(&mut self,own:&[JobSummary],other:&[JobSummary])
  ->Result<(),String>{
  if durable_busy(own) || durable_busy(other){
   return Err("FIN01_GLOBAL_CUSTODY_NOT_RECONCILED".into());
  }
  self.remove_exact_owner_marker()
 }
 /// Distinct *in-process only* cleanup for a denied admission which has not
 /// called Supervisor::prepare. It cannot recover a marker after restart,
 /// and cannot be used when any job has been newly prepared, even if an OS
 /// process has not yet been observed. Baseline/current identities cover the
 /// complete original job inventory (not a truncated renderer projection).
 pub fn release_before_prepare_if_unchanged(
  &mut self,before:&[JobSummary],after:&[JobSummary]
 ) ->Result<(),String>{
  if before.len()!=after.len() ||
     !before.iter().zip(after).all(|(a,b)|
      a.job_id==b.job_id && a.project_id==b.project_id &&
      a.plan_sha256==b.plan_sha256 && a.phase==b.phase &&
      a.requires_reconciliation==b.requires_reconciliation) {
    return Err("FIN01_NEW_JOURNAL_EVENT_BEFORE_ADMISSION_RELEASE".into());
  }
  let profile=self.marker.parent().ok_or("FIN01_PROFILE_UNAVAILABLE")?;
  if journal_fingerprint(profile,self.domain)?.as_ref()!=self.journal_identity.as_ref() {
   return Err("FIN01_OWNER_JOURNAL_BYTES_CHANGED_BEFORE_ADMISSION_RELEASE".into());
  }
  self.remove_exact_owner_marker()
 }
 fn remove_exact_owner_marker(&mut self)->Result<(),String>{
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
 /// Runtime owner check only. A marker is a *serialization receipt*,
 /// never an OS grant, but a lost/replaced marker must revoke subsequent
 /// LSP operations rather than assume continued exclusive admission.
 pub fn marker_owner_intact(&self)->Result<(),String>{
  if self.released {return Err("FIN01_GLOBAL_MARKER_ALREADY_RELEASED".into());}
  let meta=fs::symlink_metadata(&self.marker)
   .map_err(|_|"FIN01_GLOBAL_MARKER_LOST")?;
  if !meta.is_file() || meta.file_type().is_symlink() ||
     meta.len()!=self.owner_bytes.len() as u64 {
   return Err("FIN01_GLOBAL_MARKER_OWNER_CHANGED".into());
  }
  let bytes=fs::read(&self.marker)
   .map_err(|_|"FIN01_GLOBAL_MARKER_UNREADABLE")?;
  if bytes!=self.owner_bytes{
   return Err("FIN01_GLOBAL_MARKER_OWNER_CHANGED".into());
  }
  Ok(())
 }
 pub fn is_released(&self)->bool{self.released}
}

/// All source-domain journals are replayed from their original append-only
/// records. Build, debug and LSP have independent supervisors; no one journal
/// can attest to another supervisor's process tree.
const DOMAINS:[(&str,&str);3]=[
 ("build","build-job-receipts.jsonl"),
 ("debug","debug-fixture-receipts.jsonl"),
 ("lsp","lsp-job-receipts.jsonl"),
];
fn rows_for(profile:&Path,domain:&str)->Result<Vec<JobSummary>,String>{
 valid_profile(profile)?;
 let file=DOMAINS.iter().find(|(id,_)|*id==domain)
  .map(|(_,file)|*file).ok_or("FIN01_UNKNOWN_DOMAIN")?;
 lu_ide_execution_supervisor::Supervisor::open(profile.join(file))
  .and_then(|supervisor|supervisor.inspect())
  .map_err(|_|"FIN01_ORIGINAL_DOMAIN_JOURNAL_UNVERIFIED".into())
}
pub fn sibling_rows(profile:&Path,domain:&str)->Result<Vec<JobSummary>,String>{
 if !DOMAINS.iter().any(|(id,_)|*id==domain){
  return Err("FIN01_UNKNOWN_DOMAIN".into());
 }
 let mut out=Vec::new();
 for (id,_) in DOMAINS {
  if id!=domain {out.extend(rows_for(profile,id)?);}
 }
 Ok(out)
}
pub fn domain_rows(profile:&Path,domain:&str)->Result<Vec<JobSummary>,String>{
 rows_for(profile,domain)
}
/// A preflight refusal must happen before acquiring the global marker.
/// Preflight is only an optimization: each native owner must re-read all
/// other original journals *while holding* the exclusive marker.
pub fn preflight_global_idle(profile:&Path,domain:&str)->Result<(),String>{
 let own=domain_rows(profile,domain)?;
 let others=sibling_rows(profile,domain)?;
 if durable_busy(&own) || durable_busy(&others) {
  return Err("FIN01_PREVIOUS_NATIVE_WORKER_NOT_RECONCILED".into());
 }
 Ok(())
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
 fn denied_preparation_can_release_only_when_original_journal_is_unchanged(){
  let p=fixture();
  let mut fence=GlobalAdmissionFence::acquire(&p,"build").unwrap();
  let before=[job(JobPhase::Completed,false)];
  let changed=[job(JobPhase::Prepared,false)];
  assert_eq!(fence.release_before_prepare_if_unchanged(&before,&changed)
    .unwrap_err(),"FIN01_NEW_JOURNAL_EVENT_BEFORE_ADMISSION_RELEASE");
  assert!(global_custody_unresolved(&p).unwrap());
  fence.release_before_prepare_if_unchanged(&before,&before).unwrap();
  assert!(!global_custody_unresolved(&p).unwrap());
  fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn same_phase_changed_journal_bytes_cannot_unlock_denied_admission(){
  let p=fixture();
  let mut lease=GlobalAdmissionFence::acquire(&p,"build").unwrap();
  let jobs:Vec<JobSummary>=Vec::new();
  // This mutation does not prepare a job in the test state; the original
  // bytes still changed and must fail the exact owner-journal release.
  fs::write(p.join("build-job-receipts.jsonl"),b"unexpected_journal_bytes").unwrap();
  assert_eq!(lease.release_before_prepare_if_unchanged(&jobs,&jobs)
   .unwrap_err(),"FIN01_OWNER_JOURNAL_BYTES_CHANGED_BEFORE_ADMISSION_RELEASE");
  assert!(global_custody_unresolved(&p).unwrap());
  fs::remove_dir_all(p).unwrap();
 }
 #[cfg(feature="native-lsp")]
 #[test]
 fn independently_replayed_lsp_job_blocks_build_and_debug_admission(){
  let p=fixture();
  let supervisor=lu_ide_execution_supervisor::Supervisor::open(
     p.join("lsp-job-receipts.jsonl")).unwrap();
  let plan=lu_ide_execution_supervisor::JobPlan{
   project_id:format!("ide-{}","b".repeat(32)),
   recipe_id:"lsp_rust_analyzer".into(),source_head:"c".repeat(40),
   input_snapshot_sha256:"d".repeat(64),manifest_sha256:"e".repeat(64),
   toolchain_sha256:"f".repeat(64),debug_target_binding_sha256:None,
   runtime_limit_ms:300000,memory_limit_bytes:512*1024*1024,
   process_limit:4,log_limit_bytes:64*1024,
   effect_class:"language_analysis_isolated".into(),
  };
  let prepared=supervisor.prepare(plan).unwrap();
  assert_eq!(prepared.phase,JobPhase::Prepared);
  assert!(durable_busy(&sibling_rows(&p,"build").unwrap()));
  assert!(durable_busy(&sibling_rows(&p,"debug").unwrap()));
  assert_eq!(preflight_global_idle(&p,"build").unwrap_err(),
    "FIN01_PREVIOUS_NATIVE_WORKER_NOT_RECONCILED");
  assert!(!global_custody_unresolved(&p).unwrap());
  let assessment=assess_global_recovery(&p).unwrap();
  assert_eq!(assessment.lsp_journal_pending,1);
  assert!(!assessment.original_journals_reconciled);
  assert!(!assessment.marker_removal_authorized);
  fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn active_lsp_owner_cannot_continue_after_marker_replacement(){
  let p=fixture();
  let lease=GlobalAdmissionFence::acquire(&p,"lsp").unwrap();
  lease.marker_owner_intact().unwrap();
  fs::write(marker_path(&p),b"replaced-by-foreign-owner").unwrap();
  assert!(lease.marker_owner_intact().is_err());
  assert!(global_custody_unresolved(&p).unwrap());
  drop(lease);
  fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn native_clearance_rejects_original_prepared_job_even_with_marker(){
  let p=fixture();
  let supervisor=lu_ide_execution_supervisor::Supervisor::open(
    p.join("build-job-receipts.jsonl")).unwrap();
  let plan=lu_ide_execution_supervisor::JobPlan{
   project_id:format!("ide-{}","b".repeat(32)),
   recipe_id:"lu_ide_host_tests".into(),source_head:"a".repeat(40),
   input_snapshot_sha256:"c".repeat(64),manifest_sha256:"d".repeat(64),
   toolchain_sha256:"e".repeat(64),debug_target_binding_sha256:None,
   runtime_limit_ms:60_000,memory_limit_bytes:128*1024*1024,
   process_limit:4,log_limit_bytes:16*1024,
   effect_class:"test_build_isolated".into(),
  };
  let prepared=supervisor.prepare(plan).unwrap();
  assert_eq!(prepared.phase,JobPhase::Prepared);
  let _lease=GlobalAdmissionFence::acquire(&p,"build").unwrap();
  assert_eq!(clear_stale_marker_with_kernel_proof(&p).unwrap_err(),
    "FIN01_ORIGINAL_JOURNAL_NOT_TERMINAL");
  assert!(global_custody_unresolved(&p).unwrap());
  fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn malformed_abandoned_recovery_lock_never_becomes_clearance_authority(){
  let p=fixture();
  let _lease=GlobalAdmissionFence::acquire(&p,"build").unwrap();
  fs::write(p.join("fin01-global-clearance-v1.lock"),b"untrusted-owner").unwrap();
  assert_eq!(clear_stale_marker_with_kernel_proof(&p).unwrap_err(),
    "FIN01_RECOVERY_LOCK_INVALID");
  assert!(global_custody_unresolved(&p).unwrap());
  fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn current_live_marker_owner_cannot_be_cleared_from_journal_state(){
  let p=fixture();
  let lease=GlobalAdmissionFence::acquire(&p,"build").unwrap();
  let outcome=clear_stale_marker_with_kernel_proof(&p);
  assert_eq!(outcome.unwrap_err(),"FIN01_LIVE_OWNER_CANNOT_CLEAR_MARKER");
  assert!(global_custody_unresolved(&p).unwrap());
  // A rejected request never strands its separate normal recovery guard.
  assert!(!p.join("fin01-global-clearance-v1.lock").exists());
  drop(lease);fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn concurrent_stale_marker_clearance_is_exclusive(){
  let p=fixture();
  let _lease=GlobalAdmissionFence::acquire(&p,"debug").unwrap();
  let _guard=RecoveryGuard::begin(&p).unwrap();
  assert_eq!(clear_stale_marker_with_kernel_proof(&p).unwrap_err(),
    "FIN01_RECOVERY_OWNER_STILL_ALIVE");
  assert!(global_custody_unresolved(&p).unwrap());
  fs::remove_dir_all(p).unwrap();
 }
 #[test]
 fn changed_marker_or_unreconciled_job_refuses_global_clearance(){
  let p=fixture();
  let lease=GlobalAdmissionFence::acquire(&p,"lsp").unwrap();
  let (original,meta)=verified_marker_bytes(&p).unwrap();
  fs::write(marker_path(&p),b"tampered-global-marker").unwrap();
  assert!(same_marker(&p,&original,&meta).is_err());
  drop(lease);fs::remove_dir_all(&p).unwrap();
  fs::create_dir_all(&p).unwrap();
  let marker=format!("FIN01-V1:build:{}:{:016x}\\n",
    std::process::id(),42u64);
  fs::write(marker_path(&p),marker).unwrap();
  let rows=vec![job(JobPhase::Running,true)];
  assert!(durable_busy(&rows));
  assert_eq!(clear_stale_marker_with_kernel_proof(&p).unwrap_err(),
    "FIN01_LIVE_OWNER_CANNOT_CLEAR_MARKER");
  assert!(global_custody_unresolved(&p).unwrap());
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
 pub lsp_journal_pending:usize,
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
     !matches!(parts[1],"build"|"debug"|"lsp") ||
     parts[3].len()!=16 ||
     !parts[3].bytes().all(|x|x.is_ascii_hexdigit() && !x.is_ascii_uppercase()){
      return Err("FIN01_GLOBAL_MARKER_FORMAT_INVALID".into());
  }
  let parsed=parts[2].parse::<u32>()
   .map_err(|_|"FIN01_GLOBAL_MARKER_OWNER_INVALID")?;
  if parsed==0{return Err("FIN01_GLOBAL_MARKER_OWNER_INVALID".into());}
  domain=Some(parts[1].to_owned());pid=Some(parsed);
 }
 let build=domain_rows(profile,"build")?;
 let debug=domain_rows(profile,"debug")?;
 let lsp=domain_rows(profile,"lsp")?;
 let pending=|rows:&[JobSummary]|rows.iter().filter(|x|x.requires_reconciliation ||
    !matches!(x.phase,JobPhase::Completed|JobPhase::Cancelled|
      JobPhase::Failed|JobPhase::Denied)).count();
 let build_pending=pending(&build);
 let debug_pending=pending(&debug);
 let lsp_pending=pending(&lsp);
 let reconciled=build_pending==0 && debug_pending==0 && lsp_pending==0;
 Ok(RecoveryAssessment{
  marker_present:present,marker_domain:domain,marker_owner_pid:pid,
  build_journal_pending:build_pending,debug_journal_pending:debug_pending,
  lsp_journal_pending:lsp_pending,
  original_journals_reconciled:reconciled,
  native_os_release_independently_verified:false,
  operator_review_required:present || !reconciled,
  marker_removal_authorized:false,auto_retry_authorized:false,
  code:if !present && reconciled{"FIN01_GLOBAL_NO_UNRESOLVED_CUSTODY_OBSERVED"}
   else if !reconciled{"FIN01_ORIGINAL_JOURNAL_RECOVERY_REQUIRED"}
   else{"FIN01_ORIGINAL_OS_RELEASE_PROOF_REQUIRED"},
 })
}

#[derive(Debug,Clone,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub struct KernelClearanceReceipt {
 pub code:&'static str,
 pub platform:&'static str,
 pub original_job_count:usize,
 pub kernel_objects_empty:usize,
 pub kernel_objects_absent:usize,
 pub marker_sha256:String,
 pub original_journals_sha256:[Option<String>;3],
 pub replay_authorized:bool,
 pub execution_authorized:bool,
 pub original_os_kernel_verified:bool,
 pub previous_owner_exited:bool,
}
/// Separate cross-process recovery mutex. Its unexpected crash residue
/// prevents another clearance; it does NOT clear or authorize the real
/// native admission marker.
/// The existing global marker remains mandatory throughout recovery
/// lock reaping, preventing an abandoned sidecar from becoming an execution
/// bypass. A live guard owner or unverifiable native host always denies.
fn recover_abandoned_recovery_guard(profile:&Path,path:&Path)->Result<(),String>{
 if !marker_present(profile)?{
  return Err("FIN01_RECOVERY_LOCK_WITHOUT_GLOBAL_MARKER".into());
 }
 let meta=fs::symlink_metadata(path)
  .map_err(|_|"FIN01_RECOVERY_LOCK_UNREADABLE")?;
 if !meta.is_file() || meta.file_type().is_symlink() ||
    meta.len()==0 || meta.len()>96 {
  return Err("FIN01_RECOVERY_LOCK_UNTRUSTED".into());
 }
 let bytes=fs::read(path).map_err(|_|"FIN01_RECOVERY_LOCK_UNREADABLE")?;
 let body=std::str::from_utf8(&bytes)
  .map_err(|_|"FIN01_RECOVERY_LOCK_INVALID")?;
 let body=body.strip_suffix('\n')
  .ok_or("FIN01_RECOVERY_LOCK_INVALID")?;
 let parts:Vec<_>=body.split(':').collect();
 if parts.len()!=3 || parts[0]!="FIN01-RECOVERY" ||
    parts[2].len()!=16 ||
    !parts[2].bytes().all(|b|b.is_ascii_hexdigit() && !b.is_ascii_uppercase()){
  return Err("FIN01_RECOVERY_LOCK_INVALID".into());
 }
 let owner=parts[1].parse::<u32>()
  .map_err(|_|"FIN01_RECOVERY_LOCK_INVALID")?;
 if owner==0 || owner==std::process::id(){
  return Err("FIN01_RECOVERY_OWNER_STILL_ALIVE".into());
 }
 // An earlier interrupted clearance may have left an empty known
 // kernel group. The kernel verifier must receive ALL original job IDs,
 // not an empty fake inventory that rejects a legitimate residual group.
 let mut originals=Vec::new();
 for (domain,_) in DOMAINS {
  let rows=domain_rows(profile,domain)?;
  if durable_busy(&rows){
   return Err("FIN01_RECOVERY_GUARD_ORIGINAL_JOBS_STILL_ACTIVE".into());
  }
  originals.extend(rows);
 }
 #[cfg(windows)]
 lu_ide_execution_supervisor::windows_worker
  ::verify_global_worker_quiescence(owner,&originals)
  .map_err(|_|"FIN01_ABANDONED_RECOVERY_OWNER_KERNEL_UNVERIFIED")?;
 #[cfg(target_os="linux")]
 lu_ide_execution_supervisor::linux_worker
  ::verify_global_worker_quiescence(owner,&originals)
  .map_err(|_|"FIN01_ABANDONED_RECOVERY_OWNER_KERNEL_UNVERIFIED")?;
 let after=fs::symlink_metadata(path)
  .map_err(|_|"FIN01_RECOVERY_LOCK_CHANGED")?;
 if fs::read(path).map_err(|_|"FIN01_RECOVERY_LOCK_CHANGED")?!=bytes ||
    after.len()!=meta.len() || after.modified().ok()!=meta.modified().ok() ||
    !marker_present(profile)?{
  return Err("FIN01_RECOVERY_LOCK_CHANGED".into());
 }
 #[cfg(unix)]{
  use std::os::unix::fs::MetadataExt;
  if after.ino()!=meta.ino() || after.dev()!=meta.dev(){
   return Err("FIN01_RECOVERY_LOCK_REPLACED".into());
  }
 }
 fs::remove_file(path).map_err(|_|"FIN01_RECOVERY_LOCK_REAP_FAILED")?;
 #[cfg(unix)]
 File::open(profile).and_then(|d|d.sync_all())
  .map_err(|_|"FIN01_RECOVERY_LOCK_REAP_SYNC_FAILED")?;
 Ok(())
}
struct RecoveryGuard {path:PathBuf, nonce:Vec<u8>}
impl RecoveryGuard{
 fn begin(profile:&Path)->Result<Self,String>{
  let path=profile.join("fin01-global-clearance-v1.lock");
  let mut options=OpenOptions::new();
  options.create_new(true).write(true);
  #[cfg(unix)]{
   use std::os::unix::fs::OpenOptionsExt;
   options.mode(0o600);
  }
  let mut file=match options.open(&path) {
   Ok(file)=>file,
   Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>{
    // A prior process may have crashed during clearance. Never unlink a
    // stale *recovery* mutex from its age: validate its original owner as
    // exited through the same native kernel path. The actual global marker
    // must still be present and is never touched by this function.
    recover_abandoned_recovery_guard(profile,&path)?;
    options.open(&path)
     .map_err(|_|"FIN01_RECOVERY_ALREADY_IN_PROGRESS_OR_INTERRUPTED")?
   }
   Err(_)=>return Err("FIN01_RECOVERY_ALREADY_IN_PROGRESS_OR_INTERRUPTED".into()),
  };
  let nonce=format!("FIN01-RECOVERY:{}:{:016x}\n",
    std::process::id(),rand::random::<u64>()).into_bytes();
  file.write_all(&nonce).and_then(|_|file.sync_all())
   .map_err(|_|"FIN01_RECOVERY_LOCK_SYNC_UNVERIFIED")?;
  #[cfg(unix)]
  File::open(profile).and_then(|d|d.sync_all())
   .map_err(|_|"FIN01_RECOVERY_LOCK_DIRECTORY_UNVERIFIED")?;
  Ok(Self{path,nonce})
 }
}
impl Drop for RecoveryGuard{
 fn drop(&mut self){
  // Releasing a failed/rejected request is safe: the actual global
  // admission marker remains intact and no worker can be admitted.
  if fs::read(&self.path).ok().as_deref()==Some(self.nonce.as_slice()){
   let _=fs::remove_file(&self.path);
  }
 }
}
fn verified_marker_bytes(profile:&Path)->Result<(Vec<u8>,fs::Metadata),String>{
 if !marker_present(profile)? {return Err("FIN01_NO_STALE_MARKER".into());}
 let path=marker_path(profile);
 let metadata=fs::symlink_metadata(&path)
  .map_err(|_|"FIN01_STALE_MARKER_UNREADABLE")?;
 let bytes=fs::read(&path).map_err(|_|"FIN01_STALE_MARKER_UNREADABLE")?;
 if bytes.is_empty() || bytes.len()>MAX_MARKER as usize ||
    !metadata.is_file() || metadata.file_type().is_symlink() ||
    bytes.len() as u64!=metadata.len(){
  return Err("FIN01_STALE_MARKER_UNTRUSTED".into());
 }
 Ok((bytes,metadata))
}
fn same_marker(profile:&Path,expected:&[u8],meta:&fs::Metadata)->Result<(),String>{
 let (actual,now)=verified_marker_bytes(profile)?;
 if actual!=expected || now.len()!=meta.len() ||
    now.modified().ok()!=meta.modified().ok() {
  return Err("FIN01_STALE_MARKER_CHANGED".into());
 }
 #[cfg(unix)]{
  use std::os::unix::fs::MetadataExt;
  if now.ino()!=meta.ino() || now.dev()!=meta.dev(){
   return Err("FIN01_STALE_MARKER_REPLACED".into());
  }
 }
 Ok(())
}
/// Called only by the OS-confirmed Tauri host action after an explicit
/// native user dialog. A trusted "ok" boolean or journal phase is never
/// accepted as kernel proof; this function performs the kernel query.
pub fn clear_stale_marker_with_kernel_proof(
 profile:&Path
)->Result<KernelClearanceReceipt,String>{
 valid_profile(profile)?;
 let _guard=RecoveryGuard::begin(profile)?;
 let (marker,metadata)=verified_marker_bytes(profile)?;
 let assessment=assess_global_recovery(profile)?;
 if !assessment.marker_present || !assessment.original_journals_reconciled ||
    assessment.build_journal_pending!=0 || assessment.debug_journal_pending!=0 ||
    assessment.lsp_journal_pending!=0 {
  return Err("FIN01_ORIGINAL_JOURNAL_NOT_TERMINAL".into());
 }
 let owner_pid=assessment.marker_owner_pid
  .ok_or("FIN01_STALE_OWNER_ID_MISSING")?;
 if owner_pid==std::process::id(){
  return Err("FIN01_LIVE_OWNER_CANNOT_CLEAR_MARKER".into());
 }
 let baseline=[
  journal_fingerprint(profile,"build")?,
  journal_fingerprint(profile,"debug")?,
  journal_fingerprint(profile,"lsp")?,
 ];
 let mut jobs=Vec::new();
 for (domain,_) in DOMAINS {
  let rows=domain_rows(profile,domain)?;
  if durable_busy(&rows){return Err("FIN01_GLOBAL_WORKER_NOT_RECONCILED".into());}
  jobs.extend(rows);
 }
 if jobs.len()>4096{return Err("FIN01_KERNEL_AUDIT_BUDGET_EXCEEDED".into());}
 same_marker(profile,&marker,&metadata)?;
 // These platform calls read the real OS process and kernel containment
 // objects; no model/webview-supplied witness is accepted.
 #[cfg(windows)]
 let witness=lu_ide_execution_supervisor::windows_worker
  ::verify_global_worker_quiescence(owner_pid,&jobs)
  .map_err(|_|"FIN01_WINDOWS_KERNEL_CUSTODY_UNVERIFIED")?;
 #[cfg(target_os="linux")]
 let witness=lu_ide_execution_supervisor::linux_worker
  ::verify_global_worker_quiescence(owner_pid,&jobs)
  .map_err(|_|"FIN01_LINUX_KERNEL_CUSTODY_UNVERIFIED")?;
 if !witness.marker_owner_exited ||
    witness.examined_job_identities!=jobs.len() ||
    witness.kernel_objects_empty+witness.kernel_objects_absent!=jobs.len(){
  return Err("FIN01_KERNEL_WITNESS_INCOMPLETE".into());
 }
 // Recheck exact original bytes after kernel read, closing the simple
 // journal-change and same-owner marker substitution windows.
 for (i,(domain,_)) in DOMAINS.iter().enumerate(){
  if journal_fingerprint(profile,domain)?!=baseline[i] ||
     durable_busy(&domain_rows(profile,domain)?){
   return Err("FIN01_JOURNAL_CHANGED_AFTER_KERNEL_WITNESS".into());
  }
 }
 same_marker(profile,&marker,&metadata)?;
 // Durably append a minimal OS-provenance record BEFORE unlinking. A crash
 // in this interval leaves the conservative marker in place. No OS proof
 // or operator approval can be manufactured by WebView field values.
 let digest=format!("{:x}",Sha256::digest(&marker));
 let audit=profile.join("fin01-global-clearance-evidence-v1.jsonl");
 let meta=fs::symlink_metadata(&audit);
 match meta {
  Ok(m) if !m.is_file() || m.file_type().is_symlink() ||
      m.len()>8*1024*1024 =>return Err("FIN01_RECOVERY_AUDIT_UNTRUSTED".into()),
  Err(e) if e.kind()!=std::io::ErrorKind::NotFound=>
      return Err("FIN01_RECOVERY_AUDIT_UNREADABLE".into()),
  _=>{},
 }
 let witness_record=serde_json::json!({
  "schema":"fin01.global-stale-clearance.kernel-v1",
  "stage":"native_kernel_quiescence_verified_before_marker_unlink",
  "markerSha256":digest,
  "originalDomain":assessment.marker_domain,
  "kernel":witness.platform,
  "jobCount":jobs.len(),
  "kernelObjectsEmpty":witness.kernel_objects_empty,
  "kernelObjectsAbsent":witness.kernel_objects_absent,
  "ownerExited":true,
  "originalJournalSha256":baseline.clone(),
  "executionAuthorized":false,
  "autoRetryAuthorized":false
 });
 let serialized=serde_json::to_vec(&witness_record)
  .map_err(|_|"FIN01_RECOVERY_EVIDENCE_SERIALIZE_FAILED")?;
 let mut options=OpenOptions::new();
 options.create(true).append(true);
 #[cfg(unix)]{
  use std::os::unix::fs::OpenOptionsExt;
  options.mode(0o600);
 }
 let mut file=options.open(&audit)
  .map_err(|_|"FIN01_RECOVERY_AUDIT_UNAVAILABLE")?;
 file.write_all(&serialized).and_then(|_|file.write_all(b"\n"))
  .and_then(|_|file.sync_all())
  .map_err(|_|"FIN01_RECOVERY_AUDIT_SYNC_FAILED")?;
 // Requery the real kernel immediately before unlink, after durable evidence
 // was fsynced. A helper returning only a copied journal status would not
 // satisfy this gate.
 #[cfg(windows)]
 let fresh=lu_ide_execution_supervisor::windows_worker
  ::verify_global_worker_quiescence(owner_pid,&jobs)
  .map_err(|_|"FIN01_FINAL_WINDOWS_KERNEL_CUSTODY_UNVERIFIED")?;
 #[cfg(target_os="linux")]
 let fresh=lu_ide_execution_supervisor::linux_worker
  ::verify_global_worker_quiescence(owner_pid,&jobs)
  .map_err(|_|"FIN01_FINAL_LINUX_KERNEL_CUSTODY_UNVERIFIED")?;
 if !fresh.marker_owner_exited || fresh.platform!=witness.platform ||
    fresh.examined_job_identities!=jobs.len() ||
    fresh.kernel_objects_empty+fresh.kernel_objects_absent!=jobs.len(){
  return Err("FIN01_FINAL_KERNEL_WITNESS_INCOMPLETE".into());
 }
 for (i,(domain,_)) in DOMAINS.iter().enumerate(){
  if journal_fingerprint(profile,domain)?!=baseline[i] ||
     durable_busy(&domain_rows(profile,domain)?){
   return Err("FIN01_FINAL_JOURNAL_CHANGED".into());
  }
 }
 same_marker(profile,&marker,&metadata)?;
 fs::remove_file(marker_path(profile))
  .map_err(|_|"FIN01_STALE_MARKER_RELEASE_FAILED")?;
 #[cfg(unix)]
 File::open(profile).and_then(|dir|dir.sync_all())
  .map_err(|_|"FIN01_STALE_MARKER_DIRECTORY_SYNC_FAILED")?;
 Ok(KernelClearanceReceipt{
  code:"FIN01_NATIVE_KERNEL_STALE_MARKER_CLEARED",
  platform:witness.platform,original_job_count:jobs.len(),
  kernel_objects_empty:witness.kernel_objects_empty,
  kernel_objects_absent:witness.kernel_objects_absent,
  marker_sha256:digest,original_journals_sha256:baseline,
  replay_authorized:false,execution_authorized:false,
  original_os_kernel_verified:true,previous_owner_exited:true,
 })
}
