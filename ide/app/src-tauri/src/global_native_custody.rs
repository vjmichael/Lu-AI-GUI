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
  let owner_bytes=format!("FIN01-V1:{}:{}\n",domain,std::process::id()).into_bytes();
  file.write_all(&owner_bytes).and_then(|_|file.sync_all())
   .map_err(|_|"FIN01_GLOBAL_MARKER_SYNC_FAILED".to_owned())?;
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
  fs::remove_file(&self.marker).map_err(|_|"FIN01_GLOBAL_MARKER_CLEAR_FAILED")?;
  self.released=true;
  Ok(())
 }
 pub fn is_released(&self)->bool{self.released}
}
