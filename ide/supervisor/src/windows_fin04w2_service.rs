//! FIN-04W2-A: independent SCM **identity-only** debugger broker.
//!
//! This is not a debugger launcher. No pipe listener, host instruction,
//! LLDB executable, debuggee PID, WFP grant, file ACL or Job-creation API
//! exists in this module. SCM running proves only the genuine restricted
//! LocalService service token; worker isolation remains a separate gate.
//! A desktop invocation cannot satisfy StartServiceCtrlDispatcherW.
#![cfg(all(windows, debug_assertions, feature = "fin04w2-scm"))]

use crate::{windows_fin04w2_kernel, SupervisorError};
use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};
use windows_sys::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW,
    SERVICE_ACCEPT_STOP, SERVICE_CONTROL_STOP, SERVICE_RUNNING, SERVICE_START_PENDING,
    SERVICE_STATUS, SERVICE_STOPPED, SERVICE_STOP_PENDING, SERVICE_TABLE_ENTRYW,
    SERVICE_WIN32_OWN_PROCESS,
};
const NAME: &str = "LuIdeDebuggerBroker";
static STOP: AtomicBool = AtomicBool::new(false);
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
fn denied() -> SupervisorError { SupervisorError::NoContainment }

unsafe extern "system" fn control(
    code: u32,
    _kind: u32,
    _data: *mut c_void,
    _context: *mut c_void,
) -> u32 {
    if code == SERVICE_CONTROL_STOP {
        STOP.store(true, Ordering::SeqCst);
    }
    0
}
fn status(state: u32, controls: u32, exit: u32) -> SERVICE_STATUS {
    SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: controls,
        dwWin32ExitCode: exit,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: if state == SERVICE_START_PENDING || state == SERVICE_STOP_PENDING { 15000 } else { 0 },
    }
}
unsafe extern "system" fn main_service(_argc: u32, _argv: *mut *mut u16) {
    let registered = RegisterServiceCtrlHandlerExW(wide(NAME).as_ptr(), Some(control), ptr::null());
    if registered.is_null() { return; }
    let _ = SetServiceStatus(registered, &status(SERVICE_START_PENDING, 0, 0));
    STOP.store(false, Ordering::SeqCst);
    // Native kernel attestation is a prerequisite to reporting RUNNING.
    // The guardian intentionally remains OUTSIDE the future child Job;
    // the worker's exact Job membership will be a different OS proof.
    if windows_fin04w2_kernel::inspect_current_service_principal().is_err() {
        let _ = SetServiceStatus(registered, &status(SERVICE_STOPPED, 0, 1));
        return;
    }
    if SetServiceStatus(registered, &status(SERVICE_RUNNING, SERVICE_ACCEPT_STOP, 0)) == 0 {
        let _ = SetServiceStatus(registered, &status(SERVICE_STOPPED, 0, 1));
        return;
    }
    // With no listener or launch path this service cannot accept an
    // untrusted debugger command. The next gate must add a separately
    // authenticated pipe and sealed receipt preflight before any worker.
    while !STOP.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }
    let _ = SetServiceStatus(registered, &status(SERVICE_STOP_PENDING, 0, 0));
    let _ = SetServiceStatus(registered, &status(SERVICE_STOPPED, 0, 0));
}

pub fn dispatch() -> Result<(), SupervisorError> {
    let name = wide(NAME);
    let entries = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_ptr() as *mut u16,
            lpServiceProc: Some(main_service),
        },
        SERVICE_TABLE_ENTRYW { lpServiceName: ptr::null_mut(), lpServiceProc: None },
    ];
    if unsafe { StartServiceCtrlDispatcherW(entries.as_ptr()) } == 0 {
        Err(denied())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn github_desktop_runner_must_not_attest_as_service() {
        assert!(crate::windows_fin04w2_kernel::inspect_current_service_principal().is_err());
    }
}
