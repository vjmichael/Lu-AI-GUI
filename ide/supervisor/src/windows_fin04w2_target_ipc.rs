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
 /// G only requests the service to independently reobserve the original
 /// consumed one-use host grant and fsynced RUNNING journal before it resumes.
 pub(crate) fn grant_original_running(&self,pair:&IndependentScmPair)
  ->Result<(),SupervisorError>{
  pair.attest_target_server(self.pipe.0)?;
  let byte=[b'G'];let mut n=0u32;
  if unsafe{WriteFile(self.pipe.0,byte.as_ptr().cast(),1,&mut n,ptr::null_mut())}==0
      ||n!=1{return Err(denied());}
  let mut ack=[0u8;1];read_exact(self.pipe.0,&mut ack,Duration::from_secs(20))?;
  if ack!=[b'T']{return Err(denied());}
  self.reobserve(pair)
 }
 /// PC originates in the correlated LLDB-DAP stackTrace, but only the
 /// independent target SCM can compare it to a genuine Job member's RIP.
 pub(crate) fn corroborate_stopped_rip(&self,pair:&IndependentScmPair,rip:u64)
  ->Result<u32,SupervisorError>{
  if rip==0{return Err(denied());}
  pair.attest_target_server(self.pipe.0)?;
  let mut req=[0u8;9];req[0]=b'V';req[1..].copy_from_slice(&rip.to_le_bytes());
  let mut n=0u32;
  if unsafe{WriteFile(self.pipe.0,req.as_ptr().cast(),9,&mut n,ptr::null_mut())}==0
      ||n!=9{return Err(denied());}
  let mut proof=[0u8;13];read_exact(self.pipe.0,&mut proof,Duration::from_secs(20))?;
  if proof[0]!=b'P'||u64::from_le_bytes(proof[5..13].try_into().map_err(|_|denied())?)!=rip{
      return Err(denied());
  }
  let tid=u32::from_le_bytes(proof[1..5].try_into().map_err(|_|denied())?);
  if tid==0{return Err(denied());}
  self.reobserve(pair)?;
  Ok(tid)
 }
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
 if !matches!(out.len(),1|13|17|141){return Err(denied());}
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
/// This is deliberately pure: independent native tests may inject forged
/// PE/PDB, approval hash, session or PID responses without creating a Job.
fn verify_suspended_receipt(
 reply:&[u8],image:&str,pdb:&str,approval:&str,session:&str,
)->Result<u32,SupervisorError>{
 if reply.len()!=141||reply[0]!=b'S'||session.len()!=40
     ||!session.starts_with("session-")
     ||!session[8..].bytes().all(|b|b.is_ascii_hexdigit())
     ||&reply[101..141]!=session.as_bytes(){return Err(denied());}
 let pid=u32::from_le_bytes(reply[1..5].try_into().map_err(|_|denied())?);
 if pid==0{return Err(denied());}
 for (sha,bytes) in [
     (image,&reply[5..37]),(pdb,&reply[37..69]),
     (approval,&reply[69..101]),
 ]{
     if sha.len()!=64||!sha.bytes().all(|b|b.is_ascii_hexdigit()){
         return Err(denied());
     }
     for (i,b) in bytes.iter().enumerate(){
         if *b!=u8::from_str_radix(&sha[i*2..i*2+2],16)
             .map_err(|_|denied())?{return Err(denied());}
     }
 }
 Ok(pid)
}
pub(crate) fn prepare_original_target(
 pair:&IndependentScmPair,job_id:&str,plan:&JobPlan,
 expected_image_sha:&str,expected_pdb_sha:&str,
 expected_approval_sha:&str,expected_session:&str,
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
 let mut reply=[0u8;141];
 read_exact(pipe.0,&mut reply,Duration::from_secs(20))?;
 let observed=verify_suspended_receipt(
  &reply,expected_image_sha,expected_pdb_sha,
  expected_approval_sha,expected_session,
 )?;
 let lease=SuspendedTargetLease{pipe,os_observed_target_pid:observed};
 lease.reobserve(pair)?;
 Ok(lease)
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn forged_approval_session_or_target_digest_fails_without_a_kernel_grant(){
     let image="a".repeat(64);let pdb="b".repeat(64);
     let approval="c".repeat(64);let session=format!("session-{}","d".repeat(32));
     let mut reply=[0u8;141];reply[0]=b'S';reply[1..5].copy_from_slice(&42u32.to_le_bytes());
     for (off,hex) in [(5usize,&image),(37,&pdb),(69,&approval)]{
         for i in 0..32{
             reply[off+i]=u8::from_str_radix(&hex[i*2..i*2+2],16).unwrap();
         }
     }
     reply[101..].copy_from_slice(session.as_bytes());
     assert_eq!(verify_suspended_receipt(
         &reply,&image,&pdb,&approval,&session).unwrap(),42);
     let mut fake=reply;fake[69]^=1;
     assert!(verify_suspended_receipt(&fake,&image,&pdb,&approval,&session).is_err());
     let mut fake=reply;fake[141-1]^=1;
     assert!(verify_suspended_receipt(&fake,&image,&pdb,&approval,&session).is_err());
     let mut fake=reply;fake[5]^=1;
     assert!(verify_suspended_receipt(&fake,&image,&pdb,&approval,&session).is_err());
     let mut fake=reply;fake[1..5].fill(0);
     assert!(verify_suspended_receipt(&fake,&image,&pdb,&approval,&session).is_err());
     assert!(verify_suspended_receipt(&reply[..140],&image,&pdb,&approval,&session).is_err());
 }
 #[test]
 fn only_one_fixed_target_endpoint_without_client_pid_or_path(){
  assert_eq!(ENDPOINT,r"\\.\pipe\LuIdeDebugTargetBroker.v1");
  assert_eq!(fin04w2_c2::WIRE_BYTES,74);
  assert_eq!(READ|WRITE,3);
 }
}