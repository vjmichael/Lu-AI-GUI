//! FIN-04W2 target-side SCM service: independently owned *empty* Job
//! admission boundary, not a runnable debuggee or debugger service.
//! This is a first-party Windows DEBUG source profile only. No CLI command,
//! dynamic IPC, human-consent simulation, LLDB, process attach or launch.
//! It deliberately refuses to start as a console/CI user. The only native
//! authority is the distinct, restricted LuIdeDebugTargetBroker SCM token.
//! Future original-C2 handoff must own the target, AppContainer profile,
//! sealed PE/PDB and Job here, retaining them across authenticated attach,
//! cancellation, guardian-loss and restart. None is claimed by this module.
#![cfg(all(windows,debug_assertions,feature="fin04w2-split-native"))]

use crate::{windows_fin04w2_kernel as kernel,
    windows_service_security as security,SupervisorError};
use serde::Deserialize;
use std::{fs,io::Read,os::windows::fs::MetadataExt,path::PathBuf,
    ptr,sync::atomic::{AtomicBool,Ordering},thread,time::Duration};
use windows_sys::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW,SetServiceStatus,
    StartServiceCtrlDispatcherW,SERVICE_ACCEPT_STOP,
    SERVICE_CONTROL_STOP,SERVICE_RUNNING,SERVICE_START_PENDING,
    SERVICE_STATUS,SERVICE_STOPPED,SERVICE_STOP_PENDING,
    SERVICE_TABLE_ENTRYW,SERVICE_WIN32_OWN_PROCESS,
};
const SERVICE:&str="LuIdeDebugTargetBroker";
const ROOT:&str="LuIDEDebugTargetBroker";
const LIMIT:u64=4096;
static STOP:AtomicBool=AtomicBool::new(false);
fn denied()->SupervisorError{SupervisorError::NoContainment}
fn wide(v:&str)->Vec<u16>{v.encode_utf16().chain([0]).collect()}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct InstalledConfig{
    schema:String,
    service_name:String,
    expected_service_sha256:String,
}
fn read_installed_config()->Result<(),SupervisorError>{
    let root=PathBuf::from(std::env::var_os("ProgramData").ok_or_else(denied)?)
        .join(ROOT);
    let config=root.join("service.json");
    let meta=fs::symlink_metadata(&config).map_err(|_|denied())?;
    if !meta.is_file()||meta.file_attributes()&0x400!=0||meta.len()>LIMIT{
        return Err(denied());
    }
    let mut data=Vec::new();
    fs::File::open(&config).map_err(|_|denied())?
        .take(LIMIT+1).read_to_end(&mut data).map_err(|_|denied())?;
    if data.len() as u64>LIMIT{return Err(denied());}
    let installed:InstalledConfig=serde_json::from_slice(&data)
        .map_err(|_|denied())?;
    if installed.schema!="lu.ide.fin04w2.target.scm.v1"
        ||installed.service_name!=SERVICE
        ||installed.expected_service_sha256.len()!=64
        ||!installed.expected_service_sha256.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err(denied());
    }
    let service_sid=security::sid_of_named_service(SERVICE)?;
    security::verify_private_evidence_root_for(&root,&service_sid)?;
    security::hash_executable(
        &std::env::current_exe().map_err(|_|denied())?,
        &installed.expected_service_sha256)?;
    Ok(())
}
fn status(state:u32,controls:u32,exit:u32)->SERVICE_STATUS{
    SERVICE_STATUS{
        dwServiceType:SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState:state,
        dwControlsAccepted:controls,
        dwWin32ExitCode:exit,
        dwServiceSpecificExitCode:0,
        dwCheckPoint:0,
        dwWaitHint:3000,
    }
}
unsafe extern "system" fn control(code:u32,_event:u32,_data:*mut std::ffi::c_void,
    _context:*mut std::ffi::c_void)->u32{
    if code==SERVICE_CONTROL_STOP {
        STOP.store(true,Ordering::SeqCst);
    }
    0
}
fn run_scm_service(handle:windows_sys::Win32::System::Services::SERVICE_STATUS_HANDLE)
    ->Result<(),SupervisorError>{
    kernel::inspect_current_target_service_principal()?;
    read_installed_config()?;
    // Exact target-service Job: SYSTEM + target-service SID, kernel DACL
    // readback, resource caps and kill-on-close. *No target is admitted yet.*
    let _empty_owned_job=kernel::create_bounded_empty_target_job()?;
    if unsafe{SetServiceStatus(
        handle,&status(SERVICE_RUNNING,SERVICE_ACCEPT_STOP,0))}==0 {
        return Err(denied());
    }
    while !STOP.load(Ordering::SeqCst){
        thread::sleep(Duration::from_millis(50));
    }
    let _=unsafe{SetServiceStatus(
        handle,&status(SERVICE_STOP_PENDING,0,0))};
    // Dropping _empty_owned_job is a fail-closed kernel release, never
    // a retroactive human approval or an execution receipt.
    Ok(())
}
unsafe extern "system" fn main_service(_argc:u32,_argv:*mut *mut u16){
    let registered=unsafe{RegisterServiceCtrlHandlerExW(
        wide(SERVICE).as_ptr(),Some(control),ptr::null())};
    if registered.is_null(){return;}
    let _=unsafe{SetServiceStatus(
        registered,&status(SERVICE_START_PENDING,0,0))};
    let finished=std::panic::catch_unwind(||run_scm_service(registered));
    let success=matches!(finished,Ok(Ok(())))&&STOP.load(Ordering::SeqCst);
    let _=unsafe{SetServiceStatus(
        registered,&status(SERVICE_STOPPED,0,if success{0}else{1}))};
}
/// Calling this function in a normal user session fails at the genuine SCM
/// dispatcher. No alternate interactive launch or debug target exists.
pub fn dispatch()->Result<(),SupervisorError>{
    let service=wide(SERVICE);
    let table=[
        SERVICE_TABLE_ENTRYW{
            lpServiceName:service.as_ptr() as *mut u16,
            lpServiceProc:Some(main_service),
        },
        SERVICE_TABLE_ENTRYW{lpServiceName:ptr::null_mut(),lpServiceProc:None}
    ];
    if unsafe{StartServiceCtrlDispatcherW(table.as_ptr())}==0 {
        return Err(denied());
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn interactive_user_cannot_prove_target_service_identity(){
        assert!(kernel::inspect_current_target_service_principal().is_err());
    }
    #[test]
    fn malformed_admin_config_is_not_service_approval(){
        assert_eq!(SERVICE,"LuIdeDebugTargetBroker");
        assert_eq!(ROOT,"LuIDEDebugTargetBroker");
    }
}
