//! Native, read-only Windows SCM supervisor identity witness.
//!
//! This observes the *actual* running SCM service via OpenSCManager /
//! OpenService / QueryServiceStatusEx and retains its process handle and
//! verified embedded-signed executable File. It is NOT an OS Job custody
//! receipt and never authorizes a tool, restart, credential rotation or
//! installer admission. Lu Core must independently pin image/cert digests.
#![cfg(windows)]

use std::{ffi::c_void,fs::File,io,mem,ptr};
use crate::{ipc_pipe_peer::{observe_host_process,WindowsPipePeer},
    ipc_signed_package::open_signed_image};

type Handle=*mut c_void;
const SC_MANAGER_CONNECT:u32=0x0001;
const SERVICE_QUERY_STATUS:u32=0x0004;
const SC_STATUS_PROCESS_INFO:u32=0;
const SERVICE_WIN32_OWN_PROCESS:u32=0x00000010;
const SERVICE_WIN32_SHARE_PROCESS:u32=0x00000020;
const SERVICE_RUNNING:u32=4;
const PROCESS_QUERY_LIMITED_INFORMATION:u32=0x1000;
const SYNCHRONIZE:u32=0x00100000;
const WAIT_TIMEOUT:u32=0x00000102;
const SYSTEM_SID:&str="S-1-5-18";
const SUPERVISOR_SERVICE:&str="LuCoreIpcSupervisor";

#[repr(C)]
#[derive(Default)]
struct ServiceStatusProcess{
    service_type:u32,
    current_state:u32,
    controls_accepted:u32,
    win32_exit_code:u32,
    service_specific_exit_code:u32,
    check_point:u32,
    wait_hint:u32,
    process_id:u32,
    service_flags:u32,
}
const _:()={assert!(mem::size_of::<ServiceStatusProcess>()==36);};

#[link(name="advapi32")]
unsafe extern "system"{
    fn OpenSCManagerW(machine:*const u16,database:*const u16,access:u32)->Handle;
    fn OpenServiceW(manager:Handle,name:*const u16,access:u32)->Handle;
    fn QueryServiceStatusEx(service:Handle,level:u32,data:*mut u8,
        size:u32,needed:*mut u32)->i32;
    fn CloseServiceHandle(handle:Handle)->i32;
}
#[link(name="kernel32")]
unsafe extern "system"{
    fn OpenProcess(access:u32,inherit:i32,pid:u32)->Handle;
    fn GetProcessId(handle:Handle)->u32;
    fn WaitForSingleObject(handle:Handle,millis:u32)->u32;
    fn CloseHandle(handle:Handle)->i32;
}
fn refused(reason:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::PermissionDenied,reason)
}
fn valid_running_service(s:&ServiceStatusProcess)->bool{
    s.service_type & SERVICE_WIN32_OWN_PROCESS !=0
    && s.service_type & SERVICE_WIN32_SHARE_PROCESS ==0
    && s.current_state==SERVICE_RUNNING
    && s.process_id!=0
}
struct ServiceHandle(Handle);
impl Drop for ServiceHandle {
    fn drop(&mut self){if !self.0.is_null(){unsafe{CloseServiceHandle(self.0)};}}
}
struct ProcessHandle(Handle);
impl Drop for ProcessHandle {
    fn drop(&mut self){if !self.0.is_null(){unsafe{CloseHandle(self.0)};}}
}
unsafe impl Send for ServiceHandle {}
unsafe impl Sync for ServiceHandle {}
unsafe impl Send for ProcessHandle {}
unsafe impl Sync for ProcessHandle {}

fn status(service:Handle)->io::Result<ServiceStatusProcess>{
    let mut response=ServiceStatusProcess::default();
    let mut used=0u32;
    let ok=unsafe{QueryServiceStatusEx(
        service,SC_STATUS_PROCESS_INFO,(&mut response as *mut ServiceStatusProcess).cast(),
        mem::size_of::<ServiceStatusProcess>() as u32,&mut used
    )};
    if ok==0{return Err(io::Error::last_os_error());}
    if used!=mem::size_of::<ServiceStatusProcess>() as u32
       ||!valid_running_service(&response){
       return Err(refused("Lu Core supervisor is not an owned running SCM service"));
    }
    Ok(response)
}
fn open_fixed_service()->io::Result<(ServiceHandle,ServiceHandle)>{
    let manager=unsafe{OpenSCManagerW(ptr::null(),ptr::null(),SC_MANAGER_CONNECT)};
    if manager.is_null(){return Err(io::Error::last_os_error());}
    let manager=ServiceHandle(manager);
    let name:Vec<u16>=SUPERVISOR_SERVICE.encode_utf16()
        .chain(std::iter::once(0)).collect();
    let handle=unsafe{OpenServiceW(manager.0,name.as_ptr(),SERVICE_QUERY_STATUS)};
    if handle.is_null(){return Err(io::Error::last_os_error());}
    Ok((manager,ServiceHandle(handle)))
}

/// A held *live service identity* only. No public constructor or mutable
/// service name. A checked SCM process is NOT proof of a prior Job's state.
/// Caller must keep this handle and also independently pin its signature and
/// exact executable digest to trusted installed package evidence.
pub struct RunningScmSupervisor{
    _manager:ServiceHandle,
    service:ServiceHandle,
    process:ProcessHandle,
    witness:WindowsPipePeer,
    image_file:File,
    signer_der:Vec<u8>,
}
impl RunningScmSupervisor{
    /// The path must originate in a separately approved installer record,
    /// never a DLC IPC Hello, user-selected PID or an unsigned file.
    pub fn observe(expected_absolute_image:&str)->io::Result<Self>{
        if !std::path::Path::new(expected_absolute_image).is_absolute(){
            return Err(refused("SCM service executable path must be absolute"));
        }
        let (manager,service)=open_fixed_service()?;
        let first=status(service.0)?;
        let before=observe_host_process(first.process_id)?;
        if before.user_sid!=SYSTEM_SID ||before.session_id!=0
           ||!before.image_path.eq_ignore_ascii_case(expected_absolute_image){
            return Err(refused("SCM service identity differs from protected install"));
        }
        let process=unsafe{OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION|SYNCHRONIZE,0,before.pid)};
        if process.is_null(){return Err(io::Error::last_os_error());}
        let process=ProcessHandle(process);
        if unsafe{GetProcessId(process.0)}!=before.pid
            ||unsafe{WaitForSingleObject(process.0,0)}!=WAIT_TIMEOUT{
            return Err(refused("SCM supervisor original generation is not live"));
        }
        let (image_file,signer_der)=open_signed_image(expected_absolute_image)?;
        let after=observe_host_process(before.pid)?;
        let second=status(service.0)?;
        if after!=before ||second.process_id!=first.process_id{
            return Err(refused("SCM supervisor generation changed during admission"));
        }
        Ok(Self{_manager:manager,service,process,witness:after,
            image_file,signer_der})
    }
    pub fn original_process_witness(&self)->WindowsPipePeer{
        self.witness.clone()
    }
    pub fn image_file(&self)->&File {&self.image_file}
    pub fn signer_certificate_der(&self)->&[u8] {&self.signer_der}
    pub fn process_id(&self)->u32 {self.witness.pid}
    pub fn process_creation_filetime(&self)->u64 {self.witness.creation_filetime}
    /// Recheck SCM and the original process handle before *each* later use.
    pub fn check_alive(&self)->io::Result<()>{
        let original=status(self.service.0)?;
        if original.process_id!=self.witness.pid ||
           unsafe{GetProcessId(self.process.0)}!=self.witness.pid ||
           unsafe{WaitForSingleObject(self.process.0,0)}!=WAIT_TIMEOUT ||
           observe_host_process(self.witness.pid)?!=self.witness{
            return Err(refused("SCM supervisor stopped or its process generation changed"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn stopped_shared_or_pidless_service_is_never_trusted(){
        let mut s=ServiceStatusProcess{
            service_type:SERVICE_WIN32_OWN_PROCESS,
            current_state:SERVICE_RUNNING,process_id:1234,..Default::default()
        };
        assert!(valid_running_service(&s));
        s.current_state=1;assert!(!valid_running_service(&s));
        s.current_state=SERVICE_RUNNING;s.process_id=0;
        assert!(!valid_running_service(&s));
        s.process_id=1234;s.service_type=SERVICE_WIN32_SHARE_PROCESS;
        assert!(!valid_running_service(&s));
        s.service_type=SERVICE_WIN32_OWN_PROCESS|SERVICE_WIN32_SHARE_PROCESS;
        assert!(!valid_running_service(&s));
    }
    #[test]
    fn fabricated_or_relative_service_path_is_rejected(){
        assert!(RunningScmSupervisor::observe("relative-supervisor.exe").is_err());
        // An ordinary GitHub runner does not have the product's approved
        // service; if one *is* installed, an obviously unrelated image still
        // cannot match its SCM-pinned actual process image.
        assert!(RunningScmSupervisor::observe(
            r"C:\Windows\System32\not-the-lu-supervisor.exe").is_err());
    }
}
