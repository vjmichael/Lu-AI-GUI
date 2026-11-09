//! DEV-04.5D4 stage A: native, read-only Windows Job/SCM process and
//! loaded PE module witness. This is NOT a stopped-thread RIP witness, PDB
//! source-line verification, breakpoint authorization or installed acceptance.
//!
//! Only the owned kernel Job HANDLE of an admitted WindowsLspProcess may be
//! supplied. No PID, PE base, module name or executable path comes from DAP.
#![cfg(windows)]
use crate::SupervisorError;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    mem::size_of,
    os::windows::fs::MetadataExt,
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
        ERROR_NO_MORE_FILES, WAIT_TIMEOUT,
    },
    System::{
        Diagnostics::{
            Debug::{CheckRemoteDebuggerPresent, GetThreadContext, CONTEXT},
            ToolHelp::{
                CreateToolhelp32Snapshot, Module32FirstW, Module32NextW,
                Process32FirstW, Process32NextW, Thread32First, Thread32Next,
                MODULEENTRY32W, PROCESSENTRY32W, THREADENTRY32,
                TH32CS_SNAPPROCESS, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
                TH32CS_SNAPTHREAD,
            },
        },
        JobObjects::{QueryInformationJobObject, JobObjectBasicProcessIdList},
        Threading::{
            GetProcessId, GetProcessIdOfThread, OpenProcess, OpenThread,
            QueryFullProcessImageNameW, WaitForSingleObject,
            PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
            THREAD_GET_CONTEXT, THREAD_QUERY_LIMITED_INFORMATION,
        },
    },
};
const MAX_JOB_PROCESSES: usize = 64;
const MAX_MODULES: usize = 512;
const MAX_TARGET_THREADS: usize = 64;
const MAX_ENUMERATED_THREADS: usize = 65_536;
const AMD64_CONTEXT_CONTROL: u32 = 0x0010_0001;
const MAX_BINARY: u64 = 64 * 1024 * 1024;
fn denied() -> SupervisorError { SupervisorError::NoContainment }

#[derive(Debug, Clone)]
pub struct WindowsLoadedModuleWitness {
    // Native-only; no WebView serialization, process control or user grant.
    pub pid: u32,
    pub loaded_base: u64,
    pub loaded_size: u64,
    pub preferred_base: u64,
    pub preferred_pc: u64,
    pub observed_pc: u64,
    pub binary_sha256: String,
    pub job_member_verified: bool,
    pub process_exe_path_verified: bool,
    pub os_loaded_pe_image_verified: bool,
    // A ToolHelp module image covers a claimed PC; it DOES NOT attest
    // the actual stopped x64 thread-context RIP, or a PDB line record.
    pub os_thread_context_ip_verified: bool,
    pub independent_pdb_line_verified: bool,
    pub installed_app_accepted: bool,
}

/// A native-only, proof-carrying but **not yet accepted** DAP stop
/// candidate. The real Windows thread context and independent PDB line are
/// both still required before any verified source-line assertion.
#[derive(Debug, Clone)]
pub struct WindowsProtectedModuleLineage {
    pub module: WindowsLoadedModuleWitness,
    pub debugger_job_id: String,
    pub debugger_plan_sha256: String,
    pub debug_running_receipt_sha256: String,
    pub build_job_id: String,
    pub build_terminal_receipt_sha256: String,
    pub source_snapshot_sha256: String,
    pub target_binding_sha256: String,
    pub dap_claimed_source_line: u32,
    pub source_snapshot_integrity_verified: bool,
    pub source_line_mapping_verified: bool,
    pub stopped_thread_os_verified: bool,
    pub installed_app_accepted: bool,
}

/// One or more snapshots of thread state retrieved through query-only Win32
/// handles, scoped to the real job-owned process. This is NOT independently
/// certified as an OS debug-event stop; GetThreadContext on a running thread
/// may return invalid results. Both bools remain false until the separate
/// debug-event and PDB checks pass in the protected runtime.
#[derive(Clone, Debug)]
pub struct WindowsThreadRipSample {
    pub process_id: u32,
    pub thread_id: u32,
    pub observed_rip: u64,
    pub two_matching_samples: bool,
    pub job_membership_rechecked: bool,
    pub kernel_debug_event_independently_verified: bool,
    pub pdb_line_independently_verified: bool,
    pub debugger_control_authorized: bool,
}

#[repr(align(16))]
struct AlignedContext(CONTEXT);
fn read_thread_rip(process_id: u32, thread_id: u32) -> Result<u64, SupervisorError> {
    if process_id == 0 || thread_id == 0 { return Err(denied()); }
    let handle = Owned::from_win32(unsafe {
        OpenThread(THREAD_GET_CONTEXT | THREAD_QUERY_LIMITED_INFORMATION, 0, thread_id)
    })?;
    // A ToolHelp ID can be reused between enumeration and opening; the
    // real thread HANDLE must still belong to this trusted Job member.
    if unsafe { GetProcessIdOfThread(handle.0) } != process_id {
        return Err(denied());
    }
    let mut aligned = AlignedContext(unsafe { std::mem::zeroed() });
    aligned.0.ContextFlags = AMD64_CONTEXT_CONTROL;
    if unsafe { GetThreadContext(handle.0, &mut aligned.0) } == 0 {
        return Err(denied());
    }
    let rip = aligned.0.Rip;
    if rip == 0 || unsafe { GetProcessIdOfThread(handle.0) } != process_id {
        return Err(denied());
    }
    Ok(rip)
}
fn enumerate_target_threads(pid: u32) -> Result<Vec<u32>, SupervisorError> {
    let snapshot = Owned::from_win32(unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
    })?;
    let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<THREADENTRY32>() as u32;
    if unsafe { Thread32First(snapshot.0, &mut entry) } == 0 {
        return Err(denied());
    }
    let mut threads = Vec::new();
    let mut seen = BTreeSet::new();
    for scanned in 0..MAX_ENUMERATED_THREADS {
        if entry.th32OwnerProcessID == pid {
            let id = entry.th32ThreadID;
            if id == 0 || !seen.insert(id) || threads.len() >= MAX_TARGET_THREADS {
                return Err(denied());
            }
            threads.push(id);
        }
        entry.dwSize = size_of::<THREADENTRY32>() as u32;
        if unsafe { Thread32Next(snapshot.0, &mut entry) } == 0 {
            if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                return Err(denied());
            }
            if threads.is_empty() { return Err(denied()); }
            threads.sort_unstable(); // stable identity set, not snapshot order
            return Ok(threads);
        }
        if scanned + 1 == MAX_ENUMERATED_THREADS { return Err(denied()); }
    }
    Err(denied())
}
/// Query-only, bounded sampling while the DAP session claims to be stopped.
/// Cannot infer a native debug event or independently authorize a source line.
pub(crate) fn sample_job_thread_rip(
    job: HANDLE, module: &WindowsLoadedModuleWitness,
) -> Result<WindowsThreadRipSample, SupervisorError> {
    if !module.job_member_verified || !module.process_exe_path_verified
        || !module.os_loaded_pe_image_verified || module.observed_pc == 0
        || !member_ids(job)?.contains(&module.pid) {
        return Err(denied());
    }
    let original = enumerate_target_threads(module.pid)?;
    let mut found: Option<(u32, u64)> = None;
    for id in &original {
        // Do not SuspendThread, DebugActiveProcess, write target memory,
        // or attach a second debugger to LLDB's existing debug event.
        // Other threads may still be running, so unreadable/nonmatching
        // contexts are not source-level evidence; one unique match is required.
        let rip = match read_thread_rip(module.pid, *id) {
            Ok(rip) => rip,
            Err(_) => continue,
        };
        if rip == module.observed_pc {
            if found.replace((*id, rip)).is_some() { return Err(denied()); }
        }
    }
    let (thread_id, rip) = found.ok_or_else(denied)?;
    // A DAP continue, thread replacement or process exit invalidates this.
    if enumerate_target_threads(module.pid)? != original
        || read_thread_rip(module.pid, thread_id)? != rip
        || !member_ids(job)?.contains(&module.pid) {
        return Err(denied());
    }
    Ok(WindowsThreadRipSample {
        process_id: module.pid,
        thread_id,
        observed_rip: rip,
        two_matching_samples: true,
        job_membership_rechecked: true,
        kernel_debug_event_independently_verified: false,
        pdb_line_independently_verified: false,
        debugger_control_authorized: false,
    })
}

/// The owner TID comes ONLY from the first-party kernel event sideband,
/// already verified against an inherited pipe nonce and monotonic sequence.
/// Independently ensure this real Win32 thread belongs to Lu's original
/// protected LLDB process HANDLE. A DAP or model-supplied TID is not accepted.
#[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
pub(crate) fn verify_lldb_event_owner_thread(
    ll_process: HANDLE, owner_tid: u32,
) -> Result<(), SupervisorError> {
    if ll_process.is_null() || ll_process == INVALID_HANDLE_VALUE
        || owner_tid == 0 { return Err(denied()); }
    let pid = unsafe { GetProcessId(ll_process) };
    if pid == 0 || unsafe { WaitForSingleObject(ll_process, 0) } != WAIT_TIMEOUT {
        return Err(denied());
    }
    let thread = Owned::from_win32(unsafe {
        OpenThread(THREAD_QUERY_LIMITED_INFORMATION, 0, owner_tid)
    })?;
    if unsafe { GetProcessIdOfThread(thread.0) } != pid
        || unsafe { WaitForSingleObject(ll_process, 0) } != WAIT_TIMEOUT {
        return Err(denied());
    }
    Ok(())
}

/// A stronger OS-origin corroboration than LLDB-DAP's own messages:
/// the exact Job member is debugged and its kernel process-table parent is
/// the *native host-owned* admitted LLDB process. This does not expose the
/// kernel DEBUG_EVENT object or prove which LLDB thread received it.
#[derive(Clone, Debug)]
pub struct WindowsDebugPortCorroboration {
    pub(crate) debugger_pid: u32,
    pub(crate) debuggee_pid: u32,
    pub(crate) kernel_reports_debugger_attached: bool,
    pub(crate) debuggee_parent_is_owned_lldb: bool,
    pub(crate) job_membership_verified: bool,
    pub(crate) actual_debug_event_owner_attested: bool,
    pub(crate) debugger_control_authorized: bool,
}
fn kernel_parent_pid(pid: u32) -> Result<u32, SupervisorError> {
    let snapshot = Owned::from_win32(unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    })?;
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    if unsafe { Process32FirstW(snapshot.0, &mut entry) } == 0 { return Err(denied()); }
    let mut seen = false;
    let mut parent = 0;
    for index in 0..MAX_ENUMERATED_THREADS {
        if entry.th32ProcessID == pid {
            if seen { return Err(denied()); }
            seen = true;
            parent = entry.th32ParentProcessID;
        }
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
            if unsafe { GetLastError() } != ERROR_NO_MORE_FILES
                || !seen || parent == 0 { return Err(denied()); }
            return Ok(parent);
        }
        if index + 1 == MAX_ENUMERATED_THREADS { return Err(denied()); }
    }
    Err(denied())
}
/// Read-only independent OS checks. The debugger handle originates from
/// WindowsLspProcess, not the DAP frame/renderer. The Job handle is the one
/// created by the original SCM-admitted worker. No debug attach or privileged
/// target mutation is performed here.
pub(crate) fn verify_debug_port_and_owned_parent(
    job: HANDLE, debugger_process: HANDLE,
    module: &WindowsLoadedModuleWitness,
) -> Result<WindowsDebugPortCorroboration, SupervisorError> {
    if debugger_process.is_null() || debugger_process == INVALID_HANDLE_VALUE
        || !module.job_member_verified || !module.process_exe_path_verified
        || !module.os_loaded_pe_image_verified {
        return Err(denied());
    }
    let owner_pid = unsafe { GetProcessId(debugger_process) };
    if owner_pid == 0 || owner_pid == module.pid { return Err(denied()); }
    let initial_members = member_ids(job)?;
    if !initial_members.contains(&module.pid)
        || !initial_members.contains(&owner_pid)
        || unsafe { WaitForSingleObject(debugger_process, 0) } != WAIT_TIMEOUT {
        return Err(denied());
    }
    let target = Owned::from_win32(unsafe {
        OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, 0, module.pid)
    })?;
    let mut attached: i32 = 0;
    if unsafe { CheckRemoteDebuggerPresent(target.0, &mut attached) } == 0
        || attached == 0
        || kernel_parent_pid(module.pid)? != owner_pid {
        return Err(denied());
    }
    // A resumed, detached or replaced process invalidates the corroboration.
    let mut still_attached: i32 = 0;
    if unsafe { CheckRemoteDebuggerPresent(target.0, &mut still_attached) } == 0
        || still_attached == 0
        || kernel_parent_pid(module.pid)? != owner_pid
        || member_ids(job)? != initial_members
        || unsafe { WaitForSingleObject(target.0, 0) } != WAIT_TIMEOUT
        || unsafe { WaitForSingleObject(debugger_process, 0) } != WAIT_TIMEOUT {
        return Err(denied());
    }
    Ok(WindowsDebugPortCorroboration {
        debugger_pid: owner_pid,
        debuggee_pid: module.pid,
        kernel_reports_debugger_attached: true,
        debuggee_parent_is_owned_lldb: true,
        job_membership_verified: true,
        actual_debug_event_owner_attested: false,
        debugger_control_authorized: false,
    })
}

struct Owned(HANDLE);
impl Owned {
    fn from_win32(raw: HANDLE) -> Result<Self, SupervisorError> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(denied())
        } else {
            Ok(Self(raw))
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(self.0); }
        }
    }
}
#[repr(C)]
struct Pids {
    assigned: u32,
    listed: u32,
    ids: [usize; MAX_JOB_PROCESSES],
}
fn member_ids(job: HANDLE) -> Result<Vec<u32>, SupervisorError> {
    if job.is_null() || job == INVALID_HANDLE_VALUE { return Err(denied()); }
    let mut buffer = Pids {
        assigned: 0, listed: 0, ids: [0; MAX_JOB_PROCESSES],
    };
    let ok = unsafe {
        QueryInformationJobObject(
            job, JobObjectBasicProcessIdList,
            (&raw mut buffer).cast(), size_of::<Pids>() as u32,
            ptr::null_mut(),
        )
    };
    if ok == 0
        || buffer.assigned < 2
        || buffer.assigned as usize > MAX_JOB_PROCESSES
        || buffer.listed != buffer.assigned
    {
        return Err(denied());
    }
    let mut ids = Vec::new();
    let mut seen = BTreeSet::new();
    for value in buffer.ids.iter().take(buffer.listed as usize) {
        let pid = u32::try_from(*value).map_err(|_| denied())?;
        if pid == 0 || !seen.insert(pid) { return Err(denied()); }
        ids.push(pid);
    }
    Ok(ids)
}
fn normalized_path(text: &str) -> Result<String, SupervisorError> {
    if text.len() < 4 || text.len() > 2048
        || text.chars().any(char::is_control) {
        return Err(denied());
    }
    let stripped = text.strip_prefix(r"\\?\").unwrap_or(text);
    if stripped.starts_with(r"\\") || stripped.contains("/../")
        || stripped.contains(r"\..\")
        || stripped.contains(r"\.\") {
        return Err(denied());
    }
    Ok(stripped.replace('/', r"\").to_lowercase())
}
fn native_path(path: &Path) -> Result<String, SupervisorError> {
    normalized_path(path.to_str().ok_or_else(denied)?)
}
fn from_wide(buffer: &[u16]) -> Result<String, SupervisorError> {
    let end = buffer.iter().position(|c| *c == 0).ok_or_else(denied)?;
    String::from_utf16(&buffer[..end]).map_err(|_| denied())
}
fn query_executable(pid: u32) -> Result<(Owned, String), SupervisorError> {
    let process = Owned::from_win32(unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)
    })?;
    if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT {
        return Err(denied());
    }
    let mut data = [0u16; 2048];
    let mut len = data.len() as u32;
    if unsafe {
        QueryFullProcessImageNameW(process.0, 0, data.as_mut_ptr(), &mut len)
    } == 0 || len == 0 || len as usize >= data.len() {
        return Err(denied());
    }
    let path = String::from_utf16(&data[..len as usize]).map_err(|_| denied())?;
    Ok((process, normalized_path(&path)?))
}
fn sha_file(path: &Path) -> Result<String, SupervisorError> {
    let m = fs::symlink_metadata(path).map_err(|_| denied())?;
    if m.file_type().is_symlink()
        || m.file_attributes() & 0x400 != 0
        || !m.is_file() || m.len() == 0 || m.len() > MAX_BINARY {
        return Err(denied());
    }
    let mut file = File::open(path).map_err(|_| denied())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut n = 0u64;
    loop {
        let k = file.read(&mut buf).map_err(|_| denied())?;
        if k == 0 { break; }
        n = n.checked_add(k as u64).ok_or_else(denied)?;
        if n > MAX_BINARY { return Err(denied()); }
        hasher.update(&buf[..k]);
    }
    if n != m.len() { return Err(denied()); }
    let after = fs::symlink_metadata(path).map_err(|_| denied())?;
    if after.len() != m.len() || after.modified().ok() != m.modified().ok() {
        return Err(denied());
    }
    Ok(format!("{:x}", hasher.finalize()))
}
fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at.checked_add(2)?)?.try_into().ok()?))
}
fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at.checked_add(4)?)?.try_into().ok()?))
}
fn u64_at(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(data.get(at..at.checked_add(8)?)?.try_into().ok()?))
}
fn pe_preferred_image(data: &[u8]) -> Option<(u64, u64)> {
    if data.get(..2)? != b"MZ" { return None; }
    let pe = usize::try_from(u32_at(data, 0x3c)?).ok()?;
    if data.get(pe..pe.checked_add(4)?)? != b"PE\0\0" { return None; }
    let optional = pe.checked_add(24)?;
    if u16_at(data, optional)? != 0x20b { return None; }
    let size = u64::from(u32_at(data, optional.checked_add(56)?)?);
    let preferred = u64_at(data, optional.checked_add(24)?)?;
    if preferred < 0x10000 || size < 4096 || size > 512 * 1024 * 1024 {
        return None;
    }
    Some((preferred, size))
}
fn module_at(pid: u32, expected: &str, pc: u64, pe_size: u64)
    -> Result<Option<(u64, u64)>, SupervisorError>
{
    let snapshot = Owned::from_win32(unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)
    })?;
    let mut entry = MODULEENTRY32W::default();
    entry.dwSize = size_of::<MODULEENTRY32W>() as u32;
    if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
        return Err(denied());
    }
    let mut hits = Vec::new();
    for index in 0..MAX_MODULES {
        if entry.th32ProcessID != pid { return Err(denied()); }
        let module_path = normalized_path(&from_wide(&entry.szExePath)?)?;
        let base = entry.modBaseAddr as usize as u64;
        let size = u64::from(entry.modBaseSize);
        if module_path == expected && size == pe_size
            && base <= pc && pc < base.checked_add(size).ok_or_else(denied)? {
            hits.push((base, size));
        }
        entry.dwSize = size_of::<MODULEENTRY32W>() as u32;
        if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
            // A partially-read ToolHelp list is NOT module acceptance:
            // only the explicit end-of-module-list status is safe.
            if unsafe { GetLastError() } != ERROR_NO_MORE_FILES
                || hits.len() > 1 {
                return Err(denied());
            }
            return Ok(hits.pop());
        }
        if index + 1 == MAX_MODULES { return Err(denied()); }
    }
    Err(denied())
}
/// Read-only, kernel Job-bound review of a *claimed* DAP address. A true
/// stopped-thread PC still requires independent GetThreadContext evidence.
/// Windows event/LLDB state is NEVER inferred from a module-list hit.
pub(crate) fn observe_job_module(
    job: HANDLE, expected_binary: &Path, expected_sha: &str, pc: u64,
) -> Result<WindowsLoadedModuleWitness, SupervisorError> {
    if !expected_binary.is_absolute() || pc == 0
        || expected_sha.len() != 64
        || !expected_sha.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || sha_file(expected_binary)? != expected_sha {
        return Err(denied());
    }
    let bytes = fs::read(expected_binary).map_err(|_| denied())?;
    if bytes.len() as u64 > MAX_BINARY { return Err(denied()); }
    let (preferred_base, size) = pe_preferred_image(&bytes).ok_or_else(denied)?;
    let expected_path = native_path(expected_binary)?;
    let prior_members = member_ids(job)?;
    let mut result = None;
    for pid in &prior_members {
        let (process, image_path) = query_executable(*pid)?;
        if image_path != expected_path { continue; }
        let loaded = module_at(*pid, &expected_path, pc, size)?;
        let Some((loaded_base, loaded_size)) = loaded else { continue; };
        let preferred_pc = preferred_base
            .checked_add(pc.checked_sub(loaded_base).ok_or_else(denied)?)
            .ok_or_else(denied)?;
        if result.is_some() { return Err(denied()); }
        // A changed/retired process is not a valid live debugger witness.
        if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT {
            return Err(denied());
        }
        result = Some(WindowsLoadedModuleWitness {
            pid: *pid, loaded_base, loaded_size, preferred_base,
            preferred_pc, observed_pc: pc, binary_sha256: expected_sha.to_owned(),
            job_member_verified: true, process_exe_path_verified: true,
            os_loaded_pe_image_verified: true,
            os_thread_context_ip_verified: false,
            independent_pdb_line_verified: false,
            installed_app_accepted: false,
        });
    }
    let result = result.ok_or_else(denied)?;
    if prior_members != member_ids(job)? || sha_file(expected_binary)? != expected_sha {
        return Err(denied());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_real_lldb_handle_can_never_attest_event_owner() {
        let fake = WindowsLoadedModuleWitness {
            pid: 1, loaded_base: 0x140000000, loaded_size: 0x2000,
            preferred_base: 0x140000000, preferred_pc: 0x140001000,
            observed_pc: 0x140001000, binary_sha256: "a".repeat(64),
            job_member_verified: true, process_exe_path_verified: true,
            os_loaded_pe_image_verified: true, os_thread_context_ip_verified: false,
            independent_pdb_line_verified: false, installed_app_accepted: false,
        };
        assert!(verify_debug_port_and_owned_parent(
            ptr::null_mut(), ptr::null_mut(), &fake,
        ).is_err());
    }
    #[test]
    fn pe_image_header_and_preferred_pc_are_bounded() {
        let mut b = vec![0_u8; 512];
        b[..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        b[0x98..0x9a].copy_from_slice(&0x20bu16.to_le_bytes());
        b[0xb0..0xb8].copy_from_slice(&0x140000000u64.to_le_bytes());
        b[0xd0..0xd4].copy_from_slice(&0x2000u32.to_le_bytes());
        assert_eq!(pe_preferred_image(&b), Some((0x140000000, 0x2000)));
        b[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        assert_eq!(pe_preferred_image(&b), None);
        assert_eq!(pe_preferred_image(&[0; 64]), None);
    }
    #[test]
    fn thread_handle_without_expected_process_is_denied() {
        assert!(read_thread_rip(0, 0).is_err());
        assert!(read_thread_rip(0, 1).is_err());
        assert!(read_thread_rip(1, 0).is_err());
    }
    #[test]
    fn thread_context_sampling_is_never_a_debug_event_authorization() {
        let mock = WindowsLoadedModuleWitness {
            pid: 0, loaded_base: 0x140000000, loaded_size: 0x2000,
            preferred_base: 0x140000000, preferred_pc: 0x140001000,
            observed_pc: 0x140001000, binary_sha256: "a".repeat(64),
            job_member_verified: true, process_exe_path_verified: true,
            os_loaded_pe_image_verified: true, os_thread_context_ip_verified: false,
            independent_pdb_line_verified: false, installed_app_accepted: false,
        };
        assert!(sample_job_thread_rip(ptr::null_mut(), &mock).is_err());
    }
    #[test]
    fn no_forged_path_or_pid_and_no_dap_supplied_module_base() {
        assert!(normalized_path(r"C:\private\app\debug-target\test.exe").is_ok());
        assert!(normalized_path(r"C:\private\..\other.exe").is_err());
        assert!(normalized_path(r"\\server\share\test.exe").is_err());
        assert!(member_ids(ptr::null_mut()).is_err());
        assert!(observe_job_module(
            ptr::null_mut(), Path::new(r"C:\not-installed.exe"),
            &"a".repeat(64), 0x140001000
        ).is_err());
    }
}
