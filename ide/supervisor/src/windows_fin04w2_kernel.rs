//! FIN-04W2-B Windows kernel identity *preflight* only.
//! This cannot launch LLDB, create a service, open IPC or authorize a Job.
//! The independent debugger SCM broker, Job lease, ACL/WFP denial and
//! original one-use approval have separate mandatory acceptance gates.
//! Even a successful preflight is NEVER an execution or source-line receipt.
#![cfg(all(windows, debug_assertions, feature = "fin04w2-protocol"))]

use crate::SupervisorError;
use std::{ffi::OsStr, mem::size_of, os::windows::ffi::OsStrExt, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Security::{
        CheckTokenMembership, CreateWellKnownSid, EqualSid, GetTokenInformation,
        IsTokenRestricted, TokenIsAppContainer, TokenUser, WinLocalServiceSid,
        TOKEN_QUERY, TOKEN_USER,
    },
    Security::Authorization::LookupAccountNameW,
    System::Threading::{GetCurrentProcess, IsProcessInJob, OpenProcessToken},
};

fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}
struct Token(HANDLE);
impl Drop for Token {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0); }
        }
    }
}
fn service_sid() -> Result<Vec<usize>, SupervisorError> {
    let name: Vec<u16> = OsStr::new("NT SERVICE\\LuIdeDebuggerBroker")
        .encode_wide().chain([0]).collect();
    let mut sid_len = 0u32;
    let mut domain_len = 0u32;
    let mut sid_type = 0i32;
    unsafe {
        LookupAccountNameW(ptr::null(), name.as_ptr(), ptr::null_mut(),
            &mut sid_len, ptr::null_mut(), &mut domain_len, &mut sid_type);
    }
    if !(8..=512).contains(&sid_len) || domain_len > 512 {
        return Err(denied());
    }
    let mut sid = vec![0usize; (sid_len as usize).div_ceil(size_of::<usize>())];
    let mut domain = vec![0u16; domain_len as usize];
    if unsafe {
        LookupAccountNameW(ptr::null(), name.as_ptr(), sid.as_mut_ptr().cast(),
            &mut sid_len, domain.as_mut_ptr(), &mut domain_len, &mut sid_type)
    } == 0 {
        return Err(denied());
    }
    Ok(sid)
}

/// Verify the **current actual process** is already executing under a
/// non-AppContainer, restricted LocalService token with the NEW debugger
/// service SID and is already assigned to a kernel Job. No self-reported
/// service name, environment variable, user SID, or caller-supplied PID.
/// This is insufficient until the broker separately verifies its *named* Job,
/// original fsynced SCM receipt, sealed bytes and OS ACL/WFP egress denial.
pub(crate) fn inspect_current_worker_prerequisites() -> Result<(), SupervisorError> {
    let mut raw = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
        return Err(denied());
    }
    let token = Token(raw);
    if unsafe { IsTokenRestricted(token.0) } == 0 {
        return Err(denied());
    }
    let mut is_appcontainer = 1u32;
    let mut got = 0u32;
    if unsafe {
        GetTokenInformation(token.0, TokenIsAppContainer,
            (&mut is_appcontainer as *mut u32).cast(),
            size_of::<u32>() as u32, &mut got)
    } == 0 || got != size_of::<u32>() as u32 || is_appcontainer != 0 {
        return Err(denied());
    }
    let mut user_len = 0u32;
    unsafe { GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut user_len); }
    if user_len < size_of::<TOKEN_USER>() as u32 || user_len > 1024 {
        return Err(denied());
    }
    let mut user = vec![0usize; (user_len as usize).div_ceil(size_of::<usize>())];
    if unsafe {
        GetTokenInformation(token.0, TokenUser, user.as_mut_ptr().cast(),
            user_len, &mut got)
    } == 0 || got != user_len {
        return Err(denied());
    }
    let user = unsafe { &*(user.as_ptr() as *const TOKEN_USER) };
    let mut local_service = vec![0usize; 96usize.div_ceil(size_of::<usize>())];
    let mut local_len = 96u32;
    if unsafe {
        CreateWellKnownSid(WinLocalServiceSid, ptr::null_mut(),
            local_service.as_mut_ptr().cast(), &mut local_len)
    } == 0 || local_len == 0 || local_len > 96
        || unsafe { EqualSid(user.User.Sid, local_service.as_mut_ptr().cast()) } == 0 {
        return Err(denied());
    }
    let mut service = service_sid()?;
    let mut enabled = 0i32;
    if unsafe {
        CheckTokenMembership(token.0, service.as_mut_ptr().cast(), &mut enabled)
    } == 0 || enabled == 0 {
        return Err(denied());
    }
    let mut job_member = 0i32;
    if unsafe { IsProcessInJob(GetCurrentProcess(), ptr::null_mut(), &mut job_member) } == 0
        || job_member == 0 {
        return Err(denied());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interactive_ci_runner_is_not_the_independent_restricted_service_worker() {
        // The GitHub runner is intentionally NOT the installed independently
        // restricted LuIdeDebuggerBroker identity. A deny is required, even
        // when a parent CI runner happens to be inside some Job Object.
        assert!(inspect_current_worker_prerequisites().is_err());
    }
}
