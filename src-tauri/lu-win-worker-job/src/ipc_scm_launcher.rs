//! SCM-only fixed, signed Windows AppContainer executor preparation.
//!
//! No arbitrary executable, PID, argv, environment, working directory or
//! command is accepted. Installer must pre-create a protected AppContainer
//! profile and exact native worker image pins. We use the actual signed Core
//! pipe peer's user token, strip privileges, and ask Windows to create an
//! AppContainer process SUSPENDED. The Job keeper independently checks the
//! primary TokenIsAppContainer and OS process generation BEFORE resume.
//!
//! Not an effect dispatcher or installed acceptance; a fixed worker must
//! separately receive an approved typed task through its own protected input.
#![cfg(windows)]
use std::{ffi::{c_void,OsStr},io,mem,os::windows::ffi::OsStrExt,
    ptr};
use crate::{
    ipc_installer_enrollment::read_installed_scm_executor_pins,
    ipc_pipe_peer::{observe_host_process,process_is_appcontainer},
    ipc_scm_receipts::hash,
    ipc_signed_package::open_signed_image,
};
use std::io::{Read,Seek,SeekFrom};

type Handle=*mut c_void;
const PROCESS_QUERY_LIMITED_INFORMATION:u32=0x1000;
const TOKEN_QUERY:u32=8;
const TOKEN_DUPLICATE:u32=2;
const TOKEN_ALL_ACCESS:u32=0x000F_01FF;
const SECURITY_IMPERSONATION:u32=2;
const TOKEN_PRIMARY:u32=1;
const DISABLE_MAX_PRIVILEGE:u32=1;
const EXTENDED_STARTUPINFO_PRESENT:u32=0x0008_0000;
const CREATE_SUSPENDED:u32=0x0000_0004;
const CREATE_NO_WINDOW:u32=0x0800_0000;
const ATTR_SECURITY_CAPABILITIES:usize=0x0002_0009;
const APP_CONTAINER_PROFILE:&str="LuCoreIpcExecutorV1";
const WAIT_OBJECT_0:u32=0;
const MAX_IMAGE:u64=128*1024*1024;
const MAX_ATTRIBUTES:usize=64*1024;

#[repr(C)]
struct StartupInfoW{
    size:u32,reserved:*mut u16,desktop:*mut u16,title:*mut u16,
    x:u32,y:u32,width:u32,height:u32,chars_x:u32,chars_y:u32,
    fill:u32,flags:u32,show:u16,reserved2:u16,extra:*mut u8,
    stdin:Handle,stdout:Handle,stderr:Handle,
}
#[repr(C)]
struct StartupInfoExW{base:StartupInfoW,attributes:Handle}
#[repr(C)]
struct ProcessInformation{process:Handle,thread:Handle,pid:u32,tid:u32}
impl Default for ProcessInformation{
    fn default()->Self{
        Self{process:ptr::null_mut(),thread:ptr::null_mut(),pid:0,tid:0}
    }
}
#[repr(C)]
struct SecurityCapabilities{sid:Handle,capabilities:Handle,count:u32,reserved:u32}
#[link(name="kernel32")]
unsafe extern "system"{
    fn OpenProcess(access:u32,inherit:i32,pid:u32)->Handle;
    fn CloseHandle(handle:Handle)->i32;
    fn InitializeProcThreadAttributeList(list:Handle,count:u32,flags:u32,size:*mut usize)->i32;
    fn UpdateProcThreadAttribute(list:Handle,flags:u32,attribute:usize,
        value:Handle,size:usize,previous:Handle,returned:*mut usize)->i32;
    fn DeleteProcThreadAttributeList(list:Handle);
    fn TerminateProcess(process:Handle,exit_code:u32)->i32;
    fn WaitForSingleObject(process:Handle,millis:u32)->u32;
    fn GetProcessId(process:Handle)->u32;
}
#[link(name="advapi32")]
unsafe extern "system"{
    fn CreateProcessAsUserW(token:Handle,application:*const u16,
        command:*mut u16,process_attributes:Handle,thread_attributes:Handle,
        inherit:i32,flags:u32,environment:Handle,working_dir:*const u16,
        startup:*const StartupInfoW,info:*mut ProcessInformation)->i32;
    fn OpenProcessToken(process:Handle,access:u32,token:*mut Handle)->i32;
    fn DuplicateTokenEx(existing:Handle,desired_access:u32,
        attributes:Handle,impersonation_level:u32,token_type:u32,
        duplicated:*mut Handle)->i32;
    fn CreateRestrictedToken(existing:Handle,flags:u32,
        sid_count:u32,disabled_sids:Handle,privilege_count:u32,
        removed_privileges:Handle,restricted_sid_count:u32,
        restricted_sids:Handle,token:*mut Handle)->i32;
    fn FreeSid(sid:Handle)->Handle;
}
#[link(name="userenv")]
unsafe extern "system"{
    fn DeriveAppContainerSidFromAppContainerName(name:*const u16,sid:*mut Handle)->i32;
}
fn denied(reason:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::PermissionDenied,reason)
}
fn wide(value:&OsStr)->Vec<u16>{
    value.encode_wide().chain(std::iter::once(0)).collect()
}
struct Owned(Handle);
impl Drop for Owned{
    fn drop(&mut self){if !self.0.is_null(){unsafe{CloseHandle(self.0)};}}
}
struct OwnedSid(Handle);
impl Drop for OwnedSid{
    fn drop(&mut self){if !self.0.is_null(){unsafe{FreeSid(self.0)};}}
}
struct Attributes{bytes:Vec<usize>,ready:bool}
impl Attributes{
    fn new()->io::Result<Self>{
        let mut size=0usize;
        unsafe{InitializeProcThreadAttributeList(ptr::null_mut(),1,0,&mut size)};
        if size==0||size>MAX_ATTRIBUTES{return Err(denied("invalid Windows startup attributes"))}
        let mut this=Self{bytes:vec![0usize;size.div_ceil(mem::size_of::<usize>())],ready:false};
        if unsafe{InitializeProcThreadAttributeList(this.raw(),1,0,&mut size)}==0{
            return Err(io::Error::last_os_error())
        }
        this.ready=true;
        Ok(this)
    }
    fn raw(&mut self)->Handle{self.bytes.as_mut_ptr().cast()}
}
impl Drop for Attributes{
    fn drop(&mut self){
        if self.ready{unsafe{DeleteProcThreadAttributeList(self.raw())};}
    }
}
/// Sole owner of genuine native process+original suspended thread handles.
/// Never adopt an arbitrary PID or lease externally constructed processes.
pub struct FixedAppContainerChild {
    process:Owned,
    thread:Owned,
    pid:u32,
    reaped:bool,
}
unsafe impl Send for FixedAppContainerChild {}
impl FixedAppContainerChild{
    pub fn id(&self)->u32{self.pid}
    pub(crate) fn process_handle(&self)->Handle{self.process.0}
    pub(crate) fn thread_handle(&self)->Handle{self.thread.0}
    pub fn kill(&mut self)->io::Result<()>{
        if self.reaped{return Ok(())}
        // If already stopped, WaitForSingleObject confirms the true kernel
        // state; TerminateProcess returning failure alone is not success.
        if unsafe{WaitForSingleObject(self.process.0,0)}==WAIT_OBJECT_0{
            return Ok(())
        }
        if unsafe{TerminateProcess(self.process.0,1)}==0{
            return Err(io::Error::last_os_error())
        }
        Ok(())
    }
    pub fn wait(&mut self)->io::Result<()>{
        if self.reaped{return Ok(())}
        if unsafe{WaitForSingleObject(self.process.0,5000)}!=WAIT_OBJECT_0{
            return Err(denied("fixed native worker was not reaped"))
        }
        self.reaped=true;
        Ok(())
    }
}
impl Drop for FixedAppContainerChild{
    fn drop(&mut self){
        if !self.reaped{
            let _=self.kill();
            let _=self.wait();
        }
    }
}
fn image_digest(file:&std::fs::File)->io::Result<[u8;32]>{
    let mut fd=file.try_clone()?;
    fd.seek(SeekFrom::Start(0))?;
    let mut body=Vec::new();
    fd.take(MAX_IMAGE+1).read_to_end(&mut body)?;
    if body.is_empty()||body.len() as u64>MAX_IMAGE{
        return Err(denied("fixed native worker image is missing or too large"));
    }
    hash(&body)
}
/// Only the enrolled, service-verified live Lu Core peer's OS process PID.
/// The SCM must already have inspected and retained that exact signed
/// process on the original named-pipe message before calling this function.
pub fn spawn_fixed_suspended(core_peer_pid:u32)->io::Result<FixedAppContainerChild>{
    let peer=observe_host_process(core_peer_pid)?;
    if peer.pid==0||peer.user_sid=="S-1-5-18"||peer.session_id==0{
        return Err(denied("SCM worker must belong to actual user Core identity"));
    }
    let pins=read_installed_scm_executor_pins()?;
    let (image,signer)=open_signed_image(&pins.image_path)?;
    if image_digest(&image)?!=pins.image_sha256||
       hash(&signer)?!=pins.signer_sha256{
        return Err(denied("SCM fixed worker binary or publisher differs from installer"));
    }
    let core=unsafe{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,0,core_peer_pid)};
    if core.is_null(){return Err(io::Error::last_os_error());}
    let core=Owned(core);
    let mut token=ptr::null_mut();
    if unsafe{OpenProcessToken(core.0,TOKEN_QUERY|TOKEN_DUPLICATE,&mut token)}==0
        ||token.is_null(){return Err(denied("Core process token unavailable"));}
    let token=Owned(token);
    let mut duplicated=ptr::null_mut();
    if unsafe{DuplicateTokenEx(token.0,TOKEN_ALL_ACCESS,ptr::null_mut(),
        SECURITY_IMPERSONATION,TOKEN_PRIMARY,&mut duplicated)}==0
        ||duplicated.is_null(){return Err(denied("Core primary token duplication refused"));}
    let duplicated=Owned(duplicated);
    let mut reduced=ptr::null_mut();
    if unsafe{CreateRestrictedToken(duplicated.0,DISABLE_MAX_PRIVILEGE,
        0,ptr::null_mut(),0,ptr::null_mut(),0,ptr::null_mut(),&mut reduced)}==0
        ||reduced.is_null(){return Err(denied("Core restricted token creation failed"));}
    let reduced=Owned(reduced);
    let name=wide(OsStr::new(APP_CONTAINER_PROFILE));
    let mut sid=ptr::null_mut();
    if unsafe{DeriveAppContainerSidFromAppContainerName(name.as_ptr(),&mut sid)}!=0
        ||sid.is_null(){return Err(denied("installed AppContainer profile unavailable"));}
    let sid=OwnedSid(sid);
    let mut attrs=Attributes::new()?;
    let caps=SecurityCapabilities{
        sid:sid.0,capabilities:ptr::null_mut(),count:0,reserved:0
    };
    if unsafe{UpdateProcThreadAttribute(attrs.raw(),0,ATTR_SECURITY_CAPABILITIES,
        (&caps as *const SecurityCapabilities).cast_mut().cast(),
        mem::size_of::<SecurityCapabilities>(),ptr::null_mut(),ptr::null_mut())}==0{
        return Err(denied("AppContainer launch security capabilities failed"))
    }
    let mut startup:StartupInfoExW=unsafe{mem::zeroed()};
    startup.base.size=mem::size_of::<StartupInfoExW>() as u32;
    startup.attributes=attrs.raw();
    let mut information=ProcessInformation::default();
    let path=wide(pins.image_path.as_os_str());
    if unsafe{CreateProcessAsUserW(reduced.0,path.as_ptr(),ptr::null_mut(),
        ptr::null_mut(),ptr::null_mut(),0,
        CREATE_SUSPENDED|CREATE_NO_WINDOW|EXTENDED_STARTUPINFO_PRESENT,
        ptr::null_mut(),ptr::null(),&startup.base,&mut information)}==0{
        return Err(denied("Windows refused restricted AppContainer process creation"))
    }
    let mut child=FixedAppContainerChild{process:Owned(information.process),
        thread:Owned(information.thread),pid:information.pid,reaped:false};
    if child.pid==0||unsafe{GetProcessId(child.process.0)}!=child.pid ||
       observe_host_process(core_peer_pid)?!=peer ||
       !matches!(process_is_appcontainer(child.pid),Ok(true)){
        let _=child.kill();
        let _=child.wait();
        return Err(denied("Windows failed actual AppContainer token or peer generation proof"));
    }
    // Still suspended. Caller must assign to an SCM-owned kill-on-close
    // Job before the original thread may be resumed.
    Ok(child)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn no_installed_workstation_elevation_or_executable_injection(){
        let original=observe_host_process(std::process::id()).unwrap();
        if original.user_sid!="S-1-5-18"{
            assert!(spawn_fixed_suspended(0).is_err());
        }
        assert!(!APP_CONTAINER_PROFILE.is_empty());
    }
}
