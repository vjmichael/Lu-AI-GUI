//! FIN-04W2: bounded debugger-SCM to target-SCM suspended-target lease.
//! Never a host, WebView, model, or arbitrary PID/command API.
//! Closing this pipe causes the target service to kill its suspended child.
#![cfg(all(windows,debug_assertions,feature="fin04w2-split-native"))]
use crate::{fin04w2_c2,windows_fin04w2_scm_pair::IndependentScmPair,JobPlan,SupervisorError};
use std::{ptr,thread,time::{Duration,Instant}};
use windows_sys::Win32::{
 Foundation::{CloseHandle,GetLastError,HANDLE,INVALID_HANDLE_VALUE,ERROR_FILE_NOT_FOUND,ERROR_PIPE_BUSY},
 Storage::FileSystem::{CreateFileW,ReadFile,WriteFile,OPEN_EXISTING},
 System::Pipes::PeekNamedPipe,
};
const ENDPOINT:&str=r"\\.\pipe\LuIdeDebugTargetBroker.v1";
const READ:u32=1;const WRITE:u32=2;
const SYNCHRONIZE:u32=0x0010_0000;
const SECURITY_SQOS_PRESENT:u32=0x0010_0000;
const SECURITY_IMPERSONATION:u32=0x0002_0000;
fn denied()->SupervisorError{SupervisorError::NoContainment}
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain([0]).collect()}
struct Pipe(HANDLE);
impl Drop for Pipe{
 fn drop(&mut self){if !self.0.is_null()&&self.0!=INVALID_HANDLE_VALUE{unsafe{CloseHandle(self.0);}}}
}
pub(crate) struct SuspendedTargetLease{pipe:Pipe,os_observed_target_pid:u32}
impl SuspendedTargetLease{
 pub(crate) fn observed_target_pid(&self)->u32{self.os_observed_target_pid}
 pub(crate) fn reobserve(&self,pair:&IndependentScmPair)->Result<(),SupervisorError>{
  pair.attest_target_server(self.pipe.0)?;
  let mut available=0u32;
  if unsafe{PeekNamedPipe(self.pipe.0,ptr::null_mut(),0,ptr::null_mut(),
      &mut available,ptr::null_mut())}==0||available!=0||self.os_observed_target_pid==0{
   return Err(denied());
  }
  Ok(())
 }
}
fn read_exact(pipe:HANDLE,out:&mut [u8],limit:Duration)->Result<(),SupervisorError>{
 if out.len()!=17&&out.len()!=69{return Err(denied());}
 let deadline=Instant::now()+limit;
 loop{
  if Instant::now()>=deadline{return Err(denied());}
  let mut available=0u32;
  if unsafe{PeekNamedPipe(pipe,ptr::null_mut(),0,ptr::null_mut(),
      &mut available,ptr::null_mut())}==0||available>out.len() as u32{return Err(denied());}
  if available==out.len() as u32{break;}
  thread::sleep(Duration::from_millis(20));
 }
 let mut read=0u32;
 if unsafe{ReadFile(pipe,out.as_mut_ptr().cast(),out.len() as u32,
      &mut read,ptr::null_mut())}==0||read!=out.len() as u32{return Err(denied());}
 Ok(())
}
pub(crate) fn prepare_original_target(
 pair:&IndependentScmPair,job_id:&str,plan:&JobPlan,
 expected_image_sha:&str,expected_pdb_sha:&str,
)->Result<SuspendedTargetLease,SupervisorError>{
 pair.reobserve()?;
 let until=Instant::now()+Duration::from_secs(8);
 let pipe=loop{
  if Instant::now()>=until{return Err(denied());}
  let raw=unsafe{CreateFileW(wide(ENDPOINT).as_ptr(),READ|WRITE|SYNCHRONIZE,0,
      ptr::null(),OPEN_EXISTING,SECURITY_SQOS_PRESENT|SECURITY_IMPERSONATION,
      ptr::null_mut())};
  if !raw.is_null()&&raw!=INVALID_HANDLE_VALUE{
   let result=Pipe(raw);
   pair.attest_target_server(result.0)?;
   break result;
  }
  let error=unsafe{GetLastError()};
  if error!=ERROR_FILE_NOT_FOUND&&error!=ERROR_PIPE_BUSY{return Err(denied());}
  thread::sleep(Duration::from_millis(25));
 };
 let mut challenge=[0u8;17];
 read_exact(pipe.0,&mut challenge,Duration::from_secs(5))?;
 if challenge[0]!=b'C'||challenge[1..].iter().all(|b|*b==0){return Err(denied());}
 let mut nonce=[0u8;16];nonce.copy_from_slice(&challenge[1..]);
 let packet=fin04w2_c2::approved_request(job_id,plan,&nonce)?;
 pair.attest_target_server(pipe.0)?;
 let mut written=0u32;
 if unsafe{WriteFile(pipe.0,packet.as_ptr().cast(),packet.len() as u32,
      &mut written,ptr::null_mut())}==0||written!=packet.len() as u32{
  return Err(denied());
 }
 let mut reply=[0u8;69];
 read_exact(pipe.0,&mut reply,Duration::from_secs(20))?;
 if reply[0]!=b'S'{return Err(denied());}
 let observed=u32::from_le_bytes(reply[1..5].try_into().map_err(|_|denied())?);
 if observed==0{return Err(denied());}
 // Both sealed native image and PDB digests are independently pinned by
 // the debugger-service administrator, not chosen by the pipe sender.
 for (sha,bytes) in [
  (expected_image_sha,&reply[5..37]),
  (expected_pdb_sha,&reply[37..69]),
 ]{
  if sha.len()!=64||!sha.bytes().all(|b|b.is_ascii_hexdigit()){
   return Err(denied());
  }
  for (i,b) in bytes.iter().enumerate(){
   if *b!=u8::from_str_radix(&sha[i*2..i*2+2],16).map_err(|_|denied())?{
    return Err(denied());
   }
  }
 }
 let lease=SuspendedTargetLease{pipe,os_observed_target_pid:observed};
 lease.reobserve(pair)?;
 Ok(lease)
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn only_one_fixed_target_endpoint_without_client_pid_or_path(){
  assert_eq!(ENDPOINT,r"\\.\pipe\LuIdeDebugTargetBroker.v1");
  assert_eq!(fin04w2_c2::WIRE_BYTES,74);
  assert_eq!(READ|WRITE,3);
 }
}