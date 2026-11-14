//! FIN-04W2 split debuggee identity: native service evidence ONLY.
//! An outside-AppContainer LLDB-DAP must attach only to the target
//! created and retained by the independently authorized SCM service.
//! Never accept a caller-provided PID, path or DAP attach request.
//! Native WFP, service-owned debugger launch and stopped-RIP acceptance
//! remain mandatory; a verified PID is not itself a debugger grant.
#![cfg(all(windows, debug_assertions, feature="fin04w2-split-native"))]

use crate::{fin04w2_authority, windows_fin04w2_kernel,
    windows_service_security, JobPhase, JobPlan, SupervisorError};
use std::{mem::size_of, path::Path, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle,HANDLE},
    Security::{EqualSid,GetTokenInformation,IsTokenRestricted,TokenRestrictedSids,TokenAppContainerSid,
        TokenCapabilities,TokenIsAppContainer,PSID,TOKEN_QUERY},
    System::{JobObjects::IsProcessInJob,
        Threading::{GetProcessId,OpenProcessToken,QueryFullProcessImageNameW}},
};

fn denied()->SupervisorError {SupervisorError::NoContainment}
struct OwnedToken(HANDLE);
impl Drop for OwnedToken {
    fn drop(&mut self) {
        if !self.0.is_null() {unsafe{CloseHandle(self.0);}}
    }
}
#[repr(C)]
struct SidEntry {sid:PSID}

/// LLDB-DAP is a distinct process OUTSIDE AppContainer. Both actual Job
/// handles are held by the independent SCM service, never by the UI.
/// A target Job cannot be used as the debugger Job (or vice versa).
pub(crate) fn attest_separate_debugger_worker(
    debugger:HANDLE, debugger_job:HANDLE,
    target:HANDLE, target_job:HANDLE,
)->Result<(),SupervisorError>{
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    if debugger.is_null()||debugger_job.is_null()
        ||target.is_null()||target_job.is_null()
        ||debugger==target||debugger_job==target_job {
        return Err(denied());
    }
    let mut in_debug=0i32;
    let mut in_target=0i32;
    let mut crosses_target=1i32;
    let mut crosses_debug=1i32;
    if unsafe{IsProcessInJob(debugger,debugger_job,&mut in_debug)}==0
        ||unsafe{IsProcessInJob(target,target_job,&mut in_target)}==0
        ||unsafe{IsProcessInJob(debugger,target_job,&mut crosses_target)}==0
        ||unsafe{IsProcessInJob(target,debugger_job,&mut crosses_debug)}==0
        ||in_debug!=1||in_target!=1||crosses_target!=0||crosses_debug!=0 {
        return Err(denied());
    }
    let debugger_pid=unsafe{GetProcessId(debugger)};
    let target_pid=unsafe{GetProcessId(target)};
    if debugger_pid==0||target_pid==0||debugger_pid==target_pid {
        return Err(denied());
    }
    let mut raw=ptr::null_mut();
    if unsafe{OpenProcessToken(debugger,TOKEN_QUERY,&mut raw)}==0{
        return Err(denied());
    }
    let token=OwnedToken(raw);
    let mut is_appcontainer=1u32;
    let mut len=0u32;
    if unsafe{
        GetTokenInformation(token.0,TokenIsAppContainer,
            (&mut is_appcontainer as *mut u32).cast(),
            size_of::<u32>() as u32,&mut len)
    }==0||len!=size_of::<u32>() as u32||is_appcontainer!=0
        ||unsafe{IsTokenRestricted(token.0)}==0 {
        return Err(denied());
    }
    // Actual worker must retain the debugger service-specific restricting
    // SID; LocalService alone is not sufficient authority.
    let sid=windows_service_security::sid_of_named_service("LuIdeDebuggerBroker")?;
    if !windows_fin04w2_kernel::contains_sid(
        token.0,TokenRestrictedSids,sid.as_ptr().cast_mut().cast(),
    )? {
        return Err(denied());
    }
    Ok(())
}

/// Service-owned pair only. No client-controlled PID; the target's live
/// kernel image, package SID, zero capabilities and original journal are
/// verified after the independent debugger identity and Job isolation.
pub(crate) fn attest_split_pair(
    debugger:HANDLE, debugger_job:HANDLE,
    target:HANDLE, target_job:HANDLE, expected_sid:PSID,
    approved_image:&Path, image_sha256:&str,
    original_journal:&Path, original_job_id:&str,
    plan:&JobPlan, plan_sha256:&str,
)->Result<u32,SupervisorError>{
    attest_separate_debugger_worker(
        debugger,debugger_job,target,target_job,
    )?;
    attest_service_held_appcontainer_target(
        target,target_job,expected_sid,approved_image,image_sha256,
        original_journal,original_job_id,plan,plan_sha256,
    )
}

/// The native SCM service must already own BOTH handles and the profile SID.
/// It is responsible for preserving custody through attach/cancel/recovery.
/// A user- or adapter-provided numeric PID cannot enter this function.
pub(crate) fn attest_service_held_appcontainer_target(
    process:HANDLE, job:HANDLE, expected_sid:PSID,
    approved_image:&Path, image_sha256:&str,
    original_journal:&Path, original_job_id:&str,
    original_plan:&JobPlan, original_plan_sha256:&str,
)->Result<u32,SupervisorError>{
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    if process.is_null()||job.is_null()||expected_sid.is_null()
        ||!approved_image.is_absolute()
        ||original_plan.recipe_id!="dap_lldb_project"
        ||original_plan.effect_class!="debug_inspection_isolated"
        ||original_plan.debug_target_binding_sha256.is_none()
    {return Err(denied());}
    fin04w2_authority::verify_original_execution_phase(
        original_journal,original_job_id,original_plan,
        original_plan_sha256,JobPhase::Running,
    )?;
    windows_service_security::hash_executable(approved_image,image_sha256)?;
    let mut included=0i32;
    if unsafe{IsProcessInJob(process,job,&mut included)}==0||included==0 {
        return Err(denied());
    }
    let mut observed_path=[0u16;32768];
    let mut n=observed_path.len() as u32;
    if unsafe{
        QueryFullProcessImageNameW(process,0,observed_path.as_mut_ptr(),&mut n)
    }==0||n==0||n as usize>=observed_path.len(){return Err(denied());}
    let actual=String::from_utf16(&observed_path[..n as usize])
        .map_err(|_|denied())?;
    if !actual.eq_ignore_ascii_case(approved_image.to_str().ok_or_else(denied)?) {
        return Err(denied());
    }
    let mut raw=ptr::null_mut();
    if unsafe{OpenProcessToken(process,TOKEN_QUERY,&mut raw)}==0{
        return Err(denied());
    }
    let token=OwnedToken(raw);
    let mut lowbox=0u32;
    let mut got=0u32;
    if unsafe{GetTokenInformation(token.0,TokenIsAppContainer,
        (&mut lowbox as *mut u32).cast(),size_of::<u32>() as u32,&mut got)
    }==0||got!=size_of::<u32>() as u32||lowbox!=1{return Err(denied());}
    let mut needed=0u32;
    unsafe{GetTokenInformation(token.0,TokenAppContainerSid,ptr::null_mut(),0,&mut needed);}
    if needed<size_of::<SidEntry>() as u32||needed>4096{return Err(denied());}
    let mut sid=vec![0usize;(needed as usize).div_ceil(size_of::<usize>())];
    if unsafe{GetTokenInformation(token.0,TokenAppContainerSid,
        sid.as_mut_ptr().cast(),needed,&mut got)
    }==0||got!=needed{return Err(denied());}
    let found=unsafe{(*(sid.as_ptr() as *const SidEntry)).sid};
    if found.is_null()||unsafe{EqualSid(found,expected_sid)}==0{return Err(denied());}
    // The actual child token must declare zero AppContainer capabilities.
    let mut cap_size=0u32;
    unsafe{GetTokenInformation(token.0,TokenCapabilities,ptr::null_mut(),0,&mut cap_size);}
    if cap_size<4||cap_size>65536{return Err(denied());}
    let mut caps=vec![0usize;(cap_size as usize).div_ceil(size_of::<usize>())];
    if unsafe{GetTokenInformation(token.0,TokenCapabilities,
        caps.as_mut_ptr().cast(),cap_size,&mut got)
    }==0||got<4||unsafe{*(caps.as_ptr() as *const u32)}!=0{return Err(denied());}
    let pid=unsafe{GetProcessId(process)};
    if pid==0||pid==u32::MAX{return Err(denied());}
    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_runner_is_not_the_independent_debugger_service() {
        let plan=JobPlan {
            project_id:"local".into(),recipe_id:"dap_lldb_project".into(),
            source_head:"a".repeat(40),input_snapshot_sha256:"b".repeat(64),
            manifest_sha256:"c".repeat(64),toolchain_sha256:"d".repeat(64),
            debug_target_binding_sha256:Some("e".repeat(64)),
            runtime_limit_ms:1000,memory_limit_bytes:16*1024*1024,
            process_limit:2,log_limit_bytes:1024,
            effect_class:"debug_inspection_isolated".into(),
        };
        assert!(attest_service_held_appcontainer_target(
            ptr::null_mut(),ptr::null_mut(),ptr::null_mut(),
            Path::new(r"C:\Windows\not-a-debuggee.exe"),&"f".repeat(64),
            Path::new(r"C:\not-a-journal"),
            "job-00000000000000000000000000000000",&plan,&"a".repeat(64),
        ).is_err());
    }
}
