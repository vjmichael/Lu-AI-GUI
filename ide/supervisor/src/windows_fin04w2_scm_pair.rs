//! FIN-04W2: two *different* Windows SCM service processes, never just two
//! Jobs owned by one process. This is a kernel read-only attestor, NOT an
//! admission grant, debugger attach API, PID selection or service installer.
//! The trusted installer supplies exact separate service image pins; neither
//! the WebView nor the native DAP parser can supply these paths or hashes.
//! Positive SCM identity alone is not enough for original C2 authorization,
//! AppContainer/Job custody, WFP, execution, cancellation or recovered RIP.
#![cfg(all(windows,debug_assertions,feature="fin04w2-split-native"))]

use crate::{windows_service_security as security, SupervisorError};
use std::{mem::size_of,path::{Path,PathBuf},ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle,HANDLE,WAIT_TIMEOUT},
    Security::{
        CreateWellKnownSid,EqualSid,GetTokenInformation,IsTokenRestricted,
        TokenIsAppContainer,TokenUser,WinLocalServiceSid,TOKEN_QUERY,TOKEN_USER,
    },
    System::{
        Pipes::{GetNamedPipeClientProcessId,GetNamedPipeServerProcessId},
        Services::{
            CloseServiceHandle,OpenSCManagerW,OpenServiceW,QueryServiceStatusEx,
            SC_MANAGER_CONNECT,SC_STATUS_PROCESS_INFO,SERVICE_QUERY_STATUS,
            SERVICE_RUNNING,SERVICE_STATUS_PROCESS,SERVICE_WIN32_OWN_PROCESS,
        },
        Threading::{
            OpenProcess,OpenProcessToken,QueryFullProcessImageNameW,
            WaitForSingleObject,
        },
    },
};
const DEBUGGER_SERVICE:&str="LuIdeDebuggerBroker";
const TARGET_SERVICE:&str="LuIdeDebugTargetBroker";
const QUERY_PROCESS:u32=0x1000;
const SYNCHRONIZE:u32=0x0010_0000;
fn denied()->SupervisorError{SupervisorError::NoContainment}
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain([0]).collect()}
struct ServiceHandle(*mut std::ffi::c_void);
impl Drop for ServiceHandle{
    fn drop(&mut self){if !self.0.is_null(){unsafe{CloseServiceHandle(self.0);}}}
}
struct ProcessHandle(HANDLE);
impl Drop for ProcessHandle{
    fn drop(&mut self){if !self.0.is_null(){unsafe{CloseHandle(self.0);}}}
}
fn status(service: *mut std::ffi::c_void)->Result<SERVICE_STATUS_PROCESS,SupervisorError>{
    let mut value=SERVICE_STATUS_PROCESS::default();
    let mut size=0u32;
    if unsafe{QueryServiceStatusEx(service,SC_STATUS_PROCESS_INFO,
        (&mut value as *mut SERVICE_STATUS_PROCESS).cast(),
        size_of::<SERVICE_STATUS_PROCESS>() as u32,&mut size)}==0
        ||size!=size_of::<SERVICE_STATUS_PROCESS>() as u32
        ||value.dwCurrentState!=SERVICE_RUNNING
        ||value.dwServiceType!=SERVICE_WIN32_OWN_PROCESS
        ||value.dwProcessId==0{
        return Err(denied());
    }
    Ok(value)
}
fn live_image(process:HANDLE)->Result<PathBuf,SupervisorError>{
    let mut buf=[0u16;32768];
    let mut count=buf.len() as u32;
    if unsafe{QueryFullProcessImageNameW(process,0,buf.as_mut_ptr(),&mut count)}==0
        ||count==0||count as usize>=buf.len(){return Err(denied());}
    let path=String::from_utf16(&buf[..count as usize]).map_err(|_|denied())?;
    Ok(PathBuf::from(path))
}
fn restricted_service_token(process:HANDLE,name:&str)->Result<(),SupervisorError>{
    let mut raw=ptr::null_mut();
    if unsafe{OpenProcessToken(process,TOKEN_QUERY,&mut raw)}==0||raw.is_null(){
        return Err(denied());
    }
    let token=ProcessHandle(raw);
    if unsafe{IsTokenRestricted(token.0)}==0{return Err(denied());}
    let mut app=1u32;
    let mut got=0u32;
    if unsafe{GetTokenInformation(token.0,TokenIsAppContainer,
        (&mut app as *mut u32).cast(),size_of::<u32>() as u32,&mut got)}==0
        ||got!=size_of::<u32>() as u32||app!=0{return Err(denied());}
    let mut needed=0u32;
    unsafe{GetTokenInformation(token.0,TokenUser,ptr::null_mut(),0,&mut needed);}
    if needed<size_of::<TOKEN_USER>() as u32||needed>1024{return Err(denied());}
    let mut user=vec![0usize;(needed as usize).div_ceil(size_of::<usize>())];
    if unsafe{GetTokenInformation(token.0,TokenUser,
        user.as_mut_ptr().cast(),needed,&mut got)}==0||got!=needed{
        return Err(denied());
    }
    let user=unsafe{&*(user.as_ptr() as *const TOKEN_USER)};
    let mut local=[0usize;12];
    let mut sid_size=(local.len()*size_of::<usize>()) as u32;
    if unsafe{CreateWellKnownSid(WinLocalServiceSid,ptr::null_mut(),
        local.as_mut_ptr().cast(),&mut sid_size)}==0
        ||sid_size==0||sid_size>96
        ||unsafe{EqualSid(user.User.Sid,local.as_mut_ptr().cast())}==0{
        return Err(denied());
    }
    match name{
        DEBUGGER_SERVICE=>security::verify_restricted_debugger_service_sid(token.0),
        TARGET_SERVICE=>security::verify_restricted_target_service_sid(token.0),
        _=>Err(denied()),
    }
}
struct LiveService {
    _scm:ServiceHandle,
    service:ServiceHandle,
    process:ProcessHandle,
    pid:u32,
    pinned_image:PathBuf,
    pinned_sha:String,
    name:&'static str,
}
impl LiveService {
    fn open(name:&'static str,pinned_image:&Path,expected_sha:&str)
        ->Result<Self,SupervisorError>{
        if (name!=DEBUGGER_SERVICE&&name!=TARGET_SERVICE)
            ||!pinned_image.is_absolute()||expected_sha.len()!=64
            ||!expected_sha.bytes().all(|x|x.is_ascii_hexdigit()){
            return Err(denied());
        }
        let scm=ServiceHandle(unsafe{
            OpenSCManagerW(ptr::null(),ptr::null(),SC_MANAGER_CONNECT)
        });
        if scm.0.is_null(){return Err(denied());}
        let service=ServiceHandle(unsafe{
            OpenServiceW(scm.0,wide(name).as_ptr(),SERVICE_QUERY_STATUS)
        });
        if service.0.is_null(){return Err(denied());}
        let observed=status(service.0)?;
        let process=ProcessHandle(unsafe{
            OpenProcess(QUERY_PROCESS|SYNCHRONIZE,0,observed.dwProcessId)
        });
        if process.0.is_null(){return Err(denied());}
        if unsafe{WaitForSingleObject(process.0,0)}!=WAIT_TIMEOUT
            ||live_image(process.0)?.as_os_str()
                .to_string_lossy().to_lowercase()
              !=pinned_image.as_os_str().to_string_lossy().to_lowercase(){
            return Err(denied());
        }
        // The actual running SCM process, not merely a configurable file,
        // is independently hashed and kernel-token attested.
        security::hash_executable(pinned_image,expected_sha)?;
        restricted_service_token(process.0,name)?;
        if status(service.0)?.dwProcessId!=observed.dwProcessId{
            return Err(denied());
        }
        Ok(Self{_scm:scm,service,process,pid:observed.dwProcessId,
            pinned_image:pinned_image.to_path_buf(),
            pinned_sha:expected_sha.to_owned(),name})
    }
    fn reobserve(&self)->Result<(),SupervisorError>{
        if status(self.service.0)?.dwProcessId!=self.pid
            ||unsafe{WaitForSingleObject(self.process.0,0)}!=WAIT_TIMEOUT
            ||live_image(self.process.0)?.as_os_str()
                .to_string_lossy().to_lowercase()
              !=self.pinned_image.as_os_str().to_string_lossy().to_lowercase(){
            return Err(denied());
        }
        // A query handle alone does not freeze the on-disk service image.
        // Recheck the exact administrator-pinned executable digest at each
        // observation boundary, alongside the live PID and token identity.
        security::hash_executable(&self.pinned_image,&self.pinned_sha)?;
        restricted_service_token(self.process.0,self.name)
    }
}
/// The OS-observed processes and retained query handles cannot be built from
/// caller-provided PIDs, flags, JSON or a preexisting Job handle.
pub(crate) struct IndependentScmPair{
    debugger:LiveService,
    target:LiveService,
}
impl IndependentScmPair {
    /// Target-side proof: client identity is the kernel's named-pipe PID,
    /// matched against the independently verified debugger SCM process.
    pub(crate) fn attest_debugger_client(&self,pipe:HANDLE)->Result<(),SupervisorError>{
        let mut pid=0u32;
        if unsafe{GetNamedPipeClientProcessId(pipe,&mut pid)}==0
            ||pid==0||pid!=self.debugger.pid{return Err(denied());}
        self.reobserve()
    }
    /// Debugger-side proof: server identity is the kernel's named-pipe PID,
    /// matched against the independently verified target SCM process.
    pub(crate) fn attest_target_server(&self,pipe:HANDLE)->Result<(),SupervisorError>{
        let mut pid=0u32;
        if unsafe{GetNamedPipeServerProcessId(pipe,&mut pid)}==0
            ||pid==0||pid!=self.target.pid{return Err(denied());}
        self.reobserve()
    }
    pub(crate) fn reobserve(&self)->Result<(),SupervisorError>{
        if self.debugger.pid==self.target.pid
            ||self.debugger.name==self.target.name{return Err(denied());}
        self.debugger.reobserve()?;
        self.target.reobserve()?;
        Ok(())
    }
    // No function returning a PID or executable authorization; a separate
    // authenticated C2 service protocol still has to prove each child's Job.
}
pub(crate) fn inspect_independent_service_pair(
    debugger_image:&Path,debugger_sha:&str,
    target_image:&Path,target_sha:&str,
)->Result<IndependentScmPair,SupervisorError>{
    if debugger_image==target_image{return Err(denied());}
    let debugger=LiveService::open(DEBUGGER_SERVICE,debugger_image,debugger_sha)?;
    let target=LiveService::open(TARGET_SERVICE,target_image,target_sha)?;
    let result=IndependentScmPair{debugger,target};
    result.reobserve()?;
    Ok(result)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn refuses_missing_target_service_not_synthetic_pid(){
        assert!(inspect_independent_service_pair(
            Path::new("C:\\nonexistent-debugger.exe"),&"0".repeat(64),
            Path::new("C:\\nonexistent-target.exe"),&"1".repeat(64)).is_err());
    }
    #[test]
    fn refuses_same_image_for_both_services(){
        let p=Path::new("C:\\same.exe");
        assert!(inspect_independent_service_pair(
            p,&"0".repeat(64),p,&"0".repeat(64)).is_err());
    }
}
