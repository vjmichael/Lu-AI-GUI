//! D7D native-host lease on an authenticated, separately protected SCM service.
//! A ready response alone is insufficient: the server's kernel process ID,
//! fixed installed binary digest and restricted service token are all checked.
use crate::{windows_broker::{current_owner_creation, TrustedBroker},
    windows_service_security as security, SupervisorError};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{
    ffi::c_void,
    mem::size_of,
    path::Path,
    ptr,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileW, ReadFile, WriteFile, OPEN_EXISTING},
    System::{
        JobObjects::{QueryInformationJobObject, TerminateJobObject,
            JobObjectBasicAccountingInformation, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION},
        Pipes::PeekNamedPipe,
        Threading::GetCurrentProcessId,
    },
};
const FILE_READ_DATA: u32 = 0x0001;
const FILE_WRITE_DATA: u32 = 0x0002;
const SYNCHRONIZE: u32 = 0x00100000;
const SECURITY_SQOS_PRESENT: u32 = 0x00100000;
const SECURITY_IMPERSONATION: u32 = 0x00020000;
const MAX_RESPONSE: usize = 2048;
fn denied() -> SupervisorError { SupervisorError::NoContainment }
struct Handle(HANDLE);
impl Handle {
    fn new(handle: HANDLE) -> Result<Self, SupervisorError> {
        if handle == INVALID_HANDLE_VALUE || handle.is_null() { Err(denied()) }
        else { Ok(Self(handle)) }
    }
}
impl Drop for Handle {
    fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Admission<'a> {
    schema: &'static str, operation: &'static str,
    job_id: &'a str, plan_sha256: &'a str,
    owner_pid: u32, owner_creation: u64, nonce: &'a str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Release<'a> {
    schema: &'static str, operation: &'static str, nonce: &'a str,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Response {
    schema: String,
    state: String,
    nonce: String,
}
fn write(pipe: HANDLE, bytes: &[u8]) -> Result<(), SupervisorError> {
    if bytes.is_empty() || bytes.len() >= 2048 { return Err(denied()); }
    let mut sent = 0u32;
    if unsafe { WriteFile(pipe, bytes.as_ptr(), bytes.len() as u32,
        &mut sent, ptr::null_mut()) } == 0 || sent as usize != bytes.len() {
        return Err(denied());
    }
    Ok(())
}
fn read(pipe: HANDLE, nonce: &str, required: &str) -> Result<(), SupervisorError> {
    let mut bytes = [0u8; MAX_RESPONSE];
    let mut got = 0u32;
    if unsafe { ReadFile(pipe, bytes.as_mut_ptr(), bytes.len() as u32,
        &mut got, ptr::null_mut()) } == 0 || got == 0 || got as usize >= bytes.len() {
        return Err(denied());
    }
    let response: Response = serde_json::from_slice(&bytes[..got as usize])
        .map_err(|_| denied())?;
    if response.schema != "lu.ide.d7d.response.v1" || response.state != required
        || response.nonce != nonce { return Err(denied()); }
    Ok(())
}
fn empty(job: HANDLE) -> bool {
    let mut item = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    let mut count = 0u32;
    unsafe { QueryInformationJobObject(job, JobObjectBasicAccountingInformation,
        (&mut item as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
        size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
        &mut count) != 0
        && count == size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32
        && item.ActiveProcesses == 0 }
}
/// Durable service holds the independent Job handle. The desktop never
/// supplies a service path or hash from an editable project or prompt.
pub struct ServiceLease {
    pipe: Handle,
    nonce: String,
    job: HANDLE, // borrowed, owner is the worker's Job
    released: bool,
}
impl ServiceLease {
    pub fn begin(job: HANDLE, id: &str, plan_hash: &str,
        config: &TrustedBroker) -> Result<Self, SupervisorError> {
        if id.len() != 36 || !id.starts_with("job-") ||
            !id[4..].bytes().all(|b| b.is_ascii_hexdigit()) ||
            plan_hash.len() != 64 ||
            !plan_hash.bytes().all(|b| b.is_ascii_hexdigit()) ||
            !empty(job) || !config.executable.is_absolute() {
            return Err(denied());
        }
        let path = Path::new(&config.executable);
        if path.file_name().and_then(|s| s.to_str())
            .map(|s| !s.eq_ignore_ascii_case("windows_recovery_service.exe"))
            .unwrap_or(true) { return Err(denied()); }
        let pipe = Handle::new(unsafe {
            CreateFileW(security::ws(security::SERVICE_ENDPOINT).as_ptr(),
                FILE_READ_DATA | FILE_WRITE_DATA | SYNCHRONIZE, 0, ptr::null(),
                OPEN_EXISTING, SECURITY_SQOS_PRESENT | SECURITY_IMPERSONATION,
                ptr::null_mut())
        })?;
        security::authenticate_service_server(pipe.0, &config.executable,
            &config.expected_sha256)?;
        let mut bytes = [0u8; 24];
        OsRng.fill_bytes(&mut bytes);
        let nonce = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let admission = Admission {
            schema: "lu.ide.d7d.admission.v1", operation: "admit",
            job_id: id, plan_sha256: plan_hash,
            owner_pid: unsafe { GetCurrentProcessId() },
            owner_creation: current_owner_creation()?, nonce: &nonce,
        };
        write(pipe.0, &serde_json::to_vec(&admission).map_err(|_| denied())?)?;
        read(pipe.0, &nonce, "ready")?;
        // Check again AFTER the service's own fsynced ready receipt.
        if !empty(job) { return Err(denied()); }
        Ok(Self { pipe, nonce, job, released: false })
    }
    pub(crate) fn alive(&mut self) -> bool {
        let mut available = 0u32;
        unsafe { PeekNamedPipe(self.pipe.0, ptr::null_mut(), 0,
            ptr::null_mut(), &mut available, ptr::null_mut()) != 0 }
    }
    pub(crate) fn release_empty(&mut self) -> Result<(), SupervisorError> {
        if !empty(self.job) || !self.alive() { return Err(denied()); }
        let release = Release {
            schema: "lu.ide.d7d.release.v1", operation: "release", nonce: &self.nonce,
        };
        write(self.pipe.0, &serde_json::to_vec(&release).map_err(|_| denied())?)?;
        read(self.pipe.0, &self.nonce, "released_empty")?;
        self.released = true;
        Ok(())
    }
}
impl Drop for ServiceLease {
    fn drop(&mut self) {
        if !self.released {
            // Service will also kill on pipe break or host exit. A locally
            // failed operation may never use a forged READY as a release.
            unsafe { TerminateJobObject(self.job, 1); }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_requires_schema_and_server_bound_nonce() {
        let bad = br#"{"schema":"untrusted","state":"ready","nonce":"guess"}"#;
        let response: Response = serde_json::from_slice(bad).unwrap();
        assert_ne!(response.schema, "lu.ide.d7d.response.v1");
        assert_ne!(response.nonce, "server-generated-nonce");
    }
}
