//! FIN-04W2 default-OFF native split process preparation.
//!
//! This is a component for the independently authenticated SCM service,
//! NOT an executable broker, public IPC method, release worker or WFP grant.
//! It creates two distinctly Job-contained SUSPENDED processes:
//! 1. reviewed debug target under a fresh zero-capability AppContainer;
//! 2. reviewed LLDB-DAP under the restricted service token, OUTSIDE lowbox.
//! The service owns both Job handles, process handles, stdin/stdout pipes,
//! original one-use receipt and cancellation/restart. A missing positive
//! authorization or WFP proof must prevent resume and attach.
//!
//! This module has deliberately NO public PID input, no arbitrary attach,
//! no shell, no process resumption and no general OS command dispatcher.
#![cfg(all(windows, debug_assertions, feature="fin04w2-split-native"))]

use crate::{windows_fin04w2_kernel::{self,EmptyAuditJob},
    windows_fin04w2_split,windows_service_security,windows_worker,
    SupervisorError};
use std::{
    ffi::OsStr, fs, mem::size_of, os::windows::ffi::OsStrExt,
    path::{Path,PathBuf},ptr,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle,HANDLE,WAIT_OBJECT_0},
    Security::{SECURITY_ATTRIBUTES,SECURITY_CAPABILITIES},
    System::{
        JobObjects::IsProcessInJob,
        Pipes::CreatePipe,
        Threading::{
            CreateProcessW,GetProcessId,SetHandleInformation,
            TerminateProcess,UpdateProcThreadAttribute,WaitForSingleObject,
            CREATE_NO_WINDOW,CREATE_SUSPENDED,CREATE_UNICODE_ENVIRONMENT,
            EXTENDED_STARTUPINFO_PRESENT,PROCESS_INFORMATION,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            STARTF_USESTDHANDLES,STARTUPINFOEXW,
        },
    },
};

const JOB_LIST:usize=131085;
const HANDLE_LIST:usize=131074;
const HANDLE_INHERIT:u32=1;
fn denied()->SupervisorError {SupervisorError::NoContainment}
fn wide(os:&OsStr)->Vec<u16>{os.encode_wide().chain([0]).collect()}
struct Owned(HANDLE);
impl Owned {
    fn new(handle:HANDLE)->Result<Self,SupervisorError>{
        if handle.is_null(){Err(denied())}else{Ok(Self(handle))}
    }
}
impl Drop for Owned {
    fn drop(&mut self){if !self.0.is_null(){unsafe{CloseHandle(self.0);}}}
}
fn contained(process:HANDLE,job:HANDLE)->Result<(),SupervisorError>{
    let mut actual=0i32;
    if unsafe{IsProcessInJob(process,job,&mut actual)}==0||actual!=1{
        return Err(denied());
    }
    Ok(())
}
fn job_attribute(attrs:&mut windows_worker::Attributes,job:HANDLE)
    ->Result<(),SupervisorError>{
    let one=[job];
    if unsafe{UpdateProcThreadAttribute(attrs.raw(),0,JOB_LIST,
        one.as_ptr().cast(),size_of::<HANDLE>(),
        ptr::null_mut(),ptr::null())}==0{return Err(denied());}
    Ok(())
}
fn environment(root:&Path,bin:&Path)->Result<Vec<u16>,SupervisorError>{
    let system=std::env::var_os("SystemRoot").ok_or_else(denied)?;
    let system=Path::new(&system);
    if !root.is_absolute()||!bin.is_absolute()||!system.is_absolute(){
        return Err(denied());
    }
    let env=format!(
        "APPDATA={0}\0HOME={0}\0LOCALAPPDATA={0}\0PATH={1};{2}\0SystemRoot={3}\0TEMP={0}\0TMP={0}\0USERPROFILE={0}\0\0",
        root.display(),bin.display(),system.join("System32").display(),
        system.display(),
    );
    let chars:Vec<u16>=env.encode_utf16().collect();
    if chars.len()>32767{return Err(denied());}
    Ok(chars)
}

/// A process is still suspended. Dropping its lease terminates it and waits
/// for actual OS exit; independent Job kill/recovery is still required.
struct Pending {
    process:Owned,
    thread:Owned,
}
impl Pending {
    fn from_information(pi:PROCESS_INFORMATION)->Result<Self,SupervisorError>{
        let process=Owned::new(pi.hProcess)?;
        let thread=Owned::new(pi.hThread)?;
        Ok(Self{process,thread})
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        unsafe{
            TerminateProcess(self.process.0,77);
            let _=WaitForSingleObject(self.process.0,5000);
        }
    }
}

/// Hold fresh package profile until the target has been killed or reaped.
pub(crate) struct PendingTarget {
    pending:Pending,
    profile:windows_worker::AppProfile,
    image:PathBuf,
    image_sha256:String,
    job:HANDLE,
}
impl PendingTarget {
    pub(crate) fn process(&self)->HANDLE {self.pending.process.0}
    pub(crate) fn job(&self)->HANDLE {self.job}
    pub(crate) fn sid(&self)->windows_sys::Win32::Security::PSID{self.profile.sid}
    pub(crate) fn image(&self)->&Path {&self.image}
    pub(crate) fn image_sha256(&self)->&str {&self.image_sha256}
}
/// Only a native SCM service can build this exact target in its own Job.
/// The reviewed source path and SHA originate in protected service config.
pub(crate) fn prepare_appcontainer_target(
    job:&EmptyAuditJob, original_job_id:&str,
    approved_image:&Path,expected_sha256:&str,
)->Result<PendingTarget,SupervisorError>{
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    windows_service_security::hash_executable(approved_image,expected_sha256)?;
    let profile=windows_worker::AppProfile::create(original_job_id)?;
    let local=profile.local_state()?;
    let image=local.join("debug-target.exe");
    if image.exists(){return Err(denied());}
    // File copies are never trusted by pathname alone: rehash staged bytes.
    fs::copy(approved_image,&image).map_err(|_|denied())?;
    windows_service_security::hash_executable(&image,expected_sha256)?;
    let mut attrs=windows_worker::Attributes::with_count(2)?;
    let mut cap=SECURITY_CAPABILITIES::default();
    cap.AppContainerSid=profile.sid;
    cap.Capabilities=ptr::null_mut();
    cap.CapabilityCount=0;
    if unsafe{UpdateProcThreadAttribute(attrs.raw(),0,
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
        (&cap as *const SECURITY_CAPABILITIES).cast(),
        size_of::<SECURITY_CAPABILITIES>(),
        ptr::null_mut(),ptr::null())}==0{return Err(denied());}
    let kernel_job=job.held_split_job_handle();
    job_attribute(&mut attrs,kernel_job)?;
    let exe=wide(image.as_os_str());
    let cwd=wide(local.as_os_str());
    let mut args=wide(OsStr::new(&format!("\"{}\"",image.display())));
    let env=environment(&local,&PathBuf::from(std::env::var_os("SystemRoot")
        .ok_or_else(denied)?).join("System32"))?;
    let mut startup=STARTUPINFOEXW::default();
    startup.StartupInfo.cb=size_of::<STARTUPINFOEXW>() as u32;
    startup.lpAttributeList=attrs.raw();
    let mut info=PROCESS_INFORMATION::default();
    if unsafe{CreateProcessW(
        exe.as_ptr(),args.as_mut_ptr(),ptr::null(),ptr::null(),0,
        CREATE_SUSPENDED|CREATE_NO_WINDOW|CREATE_UNICODE_ENVIRONMENT|
            EXTENDED_STARTUPINFO_PRESENT,
        env.as_ptr().cast(),cwd.as_ptr(),&startup.StartupInfo,&mut info,
    )}==0{return Err(denied());}
    let pending=Pending::from_information(info)?;
    contained(pending.process.0,kernel_job)?;
    windows_worker::verify_suspended_appcontainer_token(pending.process.0,profile.sid)?;
    Ok(PendingTarget {
        pending,profile,image,image_sha256:expected_sha256.to_owned(),
        job:kernel_job,
    })
}

struct PipePair {read:Owned,write:Owned}
fn pipe()->Result<PipePair,SupervisorError>{
    let mut sa=SECURITY_ATTRIBUTES {
        nLength:size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor:ptr::null_mut(),bInheritHandle:1,
    };
    let mut read=ptr::null_mut();
    let mut write=ptr::null_mut();
    if unsafe{CreatePipe(&mut read,&mut write,&mut sa,0)}==0{
        return Err(denied());
    }
    Ok(PipePair{read:Owned::new(read)?,write:Owned::new(write)?})
}
fn private_handle(h:HANDLE)->Result<(),SupervisorError>{
    if unsafe{SetHandleInformation(h,HANDLE_INHERIT,0)}==0{
        Err(denied())
    }else{Ok(())}
}
/// LLDB worker remains outside AppContainer but inside an independent
/// service-restricted Job. Its three stdio handles are explicitly allowlisted.
pub(crate) struct PendingDebugger {
    pending:Pending,
    pub(crate) stdin_write:Owned,
    pub(crate) stdout_read:Owned,
    pub(crate) stderr_read:Owned,
    job:HANDLE,
}
impl PendingDebugger {
    pub(crate) fn process(&self)->HANDLE {self.pending.process.0}
    pub(crate) fn job(&self)->HANDLE {self.job}
}
pub(crate) fn prepare_external_debugger(
    job:&EmptyAuditJob, reviewed_lldb_dap:&Path, expected_sha256:&str,
)->Result<PendingDebugger,SupervisorError>{
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    windows_service_security::hash_executable(reviewed_lldb_dap,expected_sha256)?;
    if reviewed_lldb_dap.file_name()!=Some(OsStr::new("lldb-dap.exe")){
        return Err(denied());
    }
    let bundle=reviewed_lldb_dap.parent().ok_or_else(denied)?;
    if !bundle.is_absolute(){return Err(denied());}
    // The independent bundle preflight must pin the LLVM/Python DLL hashes;
    // file presence alone is not accepted as loader or release evidence.
    for dll in ["liblldb.dll","python310.dll"] {
        let path=bundle.join(dll);
        let meta=fs::symlink_metadata(&path).map_err(|_|denied())?;
        if !meta.is_file()
            || std::os::windows::fs::MetadataExt::file_attributes(&meta)&0x400!=0 {
            return Err(denied());
        }
    }
    let stdin=pipe()?;
    let stdout=pipe()?;
    let stderr=pipe()?;
    private_handle(stdin.write.0)?;
    private_handle(stdout.read.0)?;
    private_handle(stderr.read.0)?;
    let child_handles=[stdin.read.0,stdout.write.0,stderr.write.0];
    let mut attrs=windows_worker::Attributes::with_count(2)?;
    let kernel_job=job.held_split_job_handle();
    job_attribute(&mut attrs,kernel_job)?;
    if unsafe{UpdateProcThreadAttribute(attrs.raw(),0,HANDLE_LIST,
        child_handles.as_ptr().cast(),
        child_handles.len()*size_of::<HANDLE>(),
        ptr::null_mut(),ptr::null())}==0{return Err(denied());}
    let exe=wide(reviewed_lldb_dap.as_os_str());
    let cwd=wide(bundle.as_os_str());
    let mut args=wide(OsStr::new(&format!("\"{}\"",reviewed_lldb_dap.display())));
    let private_root=PathBuf::from(std::env::var_os("ProgramData")
        .ok_or_else(denied)?).join("LuIdeDebuggerBroker");
    let env=environment(&private_root,bundle)?;
    let mut startup=STARTUPINFOEXW::default();
    startup.StartupInfo.cb=size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags=STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput=stdin.read.0;
    startup.StartupInfo.hStdOutput=stdout.write.0;
    startup.StartupInfo.hStdError=stderr.write.0;
    startup.lpAttributeList=attrs.raw();
    let mut info=PROCESS_INFORMATION::default();
    if unsafe{CreateProcessW(
        exe.as_ptr(),args.as_mut_ptr(),ptr::null(),ptr::null(),1,
        CREATE_SUSPENDED|CREATE_NO_WINDOW|CREATE_UNICODE_ENVIRONMENT|
            EXTENDED_STARTUPINFO_PRESENT,
        env.as_ptr().cast(),cwd.as_ptr(),&startup.StartupInfo,&mut info,
    )}==0{return Err(denied());}
    let pending=Pending::from_information(info)?;
    contained(pending.process.0,kernel_job)?;
    // No lowbox attribute was supplied; independent service-SID attestation
    // checks the actual child token before any resume or attach.
    Ok(PendingDebugger{
        pending,stdin_write:stdin.write,stdout_read:stdout.read,
        stderr_read:stderr.read,job:kernel_job,
    })
}

/// Both first-party reviewed processes are staged SUSPENDED with different
/// Job leases. The service MUST verify original grant+WFP and externally
/// reobserve the two held tokens before resuming either process.
/// Nothing here accepts an arbitrary PID or grants a runnable debug session.
pub(crate) struct PendingSplitPair {
    pub(crate) debugger:PendingDebugger,
    pub(crate) target:PendingTarget,
}
pub(crate) fn prepare_pair(
    target_job:&EmptyAuditJob,debugger_job:&EmptyAuditJob,
    original_job_id:&str,
    target_image:&Path,target_sha256:&str,
    lldb_dap:&Path,lldb_sha256:&str,
)->Result<PendingSplitPair,SupervisorError>{
    if target_job.held_split_job_handle()==debugger_job.held_split_job_handle(){
        return Err(denied());
    }
    let target=prepare_appcontainer_target(
        target_job,original_job_id,target_image,target_sha256,
    )?;
    let debugger=prepare_external_debugger(
        debugger_job,lldb_dap,lldb_sha256,
    )?;
    windows_fin04w2_split::attest_separate_debugger_worker(
        debugger.process(),debugger.job(),target.process(),target.job(),
    )?;
    Ok(PendingSplitPair{debugger,target})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caller_without_installed_service_identity_cannot_prepare_split_target(){
        // Even with a syntactically approved path there is no host fallback.
        let result=windows_fin04w2_kernel::inspect_current_service_principal();
        assert!(result.is_err());
    }
}
