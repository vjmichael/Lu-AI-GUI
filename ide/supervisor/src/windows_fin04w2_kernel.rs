//! FIN-04W2-B Windows kernel identity *preflight* only.
//! This cannot launch LLDB, create a service, open IPC or authorize a Job.
//! The independent debugger SCM broker, Job lease, ACL/WFP denial and
//! original one-use approval have separate mandatory acceptance gates.
//! Even a successful preflight is NEVER an execution or source-line receipt.
#![cfg(all(windows, debug_assertions, feature = "fin04w2-scm"))]

use crate::SupervisorError;
use std::{ffi::OsStr, mem::size_of, os::windows::ffi::OsStrExt, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, WAIT_OBJECT_0},
    Security::{
        CheckTokenMembership, CreateWellKnownSid, DuplicateToken, EqualSid, GetTokenInformation,
        IsTokenRestricted, LookupAccountNameW, TokenGroups, TokenRestrictedSids,
        TokenIsAppContainer, TokenUser, WinLocalServiceSid, SecurityImpersonation,
        SID_AND_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_GROUPS, TOKEN_QUERY, TOKEN_USER,
    },
    System::{
        JobObjects::{
            CreateJobObjectW, IsProcessInJob, QueryInformationJobObject,
            SetInformationJobObject, JobObjectBasicAccountingInformation,
            JobObjectCpuRateControlInformation, JobObjectExtendedLimitInformation, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
            JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
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
fn service_sid(service_name:&str) -> Result<Vec<usize>, SupervisorError> {
    if service_name!="LuIdeDebuggerBroker"
        &&service_name!="LuIdeDebugTargetBroker" {return Err(denied());}
    let account=format!("NT SERVICE\\\\{}",service_name);
    let name: Vec<u16> = OsStr::new(&account)
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

// Both membership and the *restricted* SID allowlist are mandatory.
// A service SID that appears only in enabled groups is not a protected
// service-SID execution boundary. Bounded token query, read-only.
fn contains_sid(
    token: HANDLE, class: windows_sys::Win32::Security::TOKEN_INFORMATION_CLASS,
    expected: windows_sys::Win32::Security::PSID,
) -> Result<bool, SupervisorError> {
    let mut needed = 0u32;
    unsafe { GetTokenInformation(token, class, ptr::null_mut(), 0, &mut needed); }
    if needed < size_of::<TOKEN_GROUPS>() as u32 || needed > 4096 {
        return Err(denied());
    }
    let mut storage = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    let mut received = 0u32;
    if unsafe {
        GetTokenInformation(
            token, class, storage.as_mut_ptr().cast(), needed, &mut received,
        )
    } == 0 || received != needed {
        return Err(denied());
    }
    let groups = unsafe { &*(storage.as_ptr() as *const TOKEN_GROUPS) };
    if groups.GroupCount == 0 || groups.GroupCount > 64 {
        return Err(denied());
    }
    let total = size_of::<TOKEN_GROUPS>()
        .checked_add(
            (groups.GroupCount as usize - 1)
                .checked_mul(size_of::<SID_AND_ATTRIBUTES>())
                .ok_or_else(denied)?,
        )
        .ok_or_else(denied)?;
    if total > storage.len() * size_of::<usize>() {
        return Err(denied());
    }
    let entries = unsafe {
        std::slice::from_raw_parts(groups.Groups.as_ptr(), groups.GroupCount as usize)
    };
    Ok(entries.iter().any(|entry| {
        !entry.Sid.is_null() && unsafe { EqualSid(entry.Sid, expected) } != 0
    }))
}

/// The service process is intentionally *outside* the per-debuggee Job:
/// the independent SCM guardian must survive and retain the Job lease.
/// Only its OS token, not any string or caller-supplied PID, identifies it.
/// This is insufficient for service request admission, WFP or debugger launch.
pub(crate) fn inspect_current_service_principal() -> Result<(), SupervisorError> {
    inspect_current_split_service_principal("LuIdeDebuggerBroker")
}
#[cfg(feature="fin04w2-split-native")]
pub(crate) fn inspect_current_target_service_principal() -> Result<(), SupervisorError> {
    inspect_current_split_service_principal("LuIdeDebugTargetBroker")
}
fn inspect_current_split_service_principal(service_name:&str) -> Result<(), SupervisorError> {
    if service_name!="LuIdeDebuggerBroker"
        && service_name!="LuIdeDebugTargetBroker" {return Err(denied());}
    let mut raw = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE, &mut raw) } == 0 {
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
    let mut service = service_sid(service_name)?;
    // CheckTokenMembership requires an impersonation token. The kernel
    // primary token is duplicated for this READ-ONLY query; neither
    // impersonation nor its resulting handle is used for execution.
    let mut duplicate = ptr::null_mut();
    if unsafe { DuplicateToken(token.0, SecurityImpersonation, &mut duplicate) } == 0 {
        return Err(denied());
    }
    let duplicate = Token(duplicate);
    let mut enabled = 0i32;
    if unsafe {
        CheckTokenMembership(duplicate.0, service.as_mut_ptr().cast(), &mut enabled)
    } == 0 || enabled == 0
        || !contains_sid(token.0, TokenGroups, service.as_mut_ptr().cast())?
        || !contains_sid(token.0, TokenRestrictedSids, service.as_mut_ptr().cast())?
    {
        return Err(denied());
    }
    Ok(())
}

/// Separate read-only preflight for an OS worker that the independent
/// service has already placed into the correct named Job. Checking *some*
/// job membership is not a substitute for the separate exact Job+receipt
/// witness, which is mandatory before any debugger execution is admitted.
pub(crate) fn inspect_current_worker_prerequisites() -> Result<(), SupervisorError> {
    inspect_current_service_principal()?;
    let mut job_member = 0i32;
    if unsafe { IsProcessInJob(GetCurrentProcess(), ptr::null_mut(), &mut job_member) } == 0
        || job_member == 0 {
        return Err(denied());
    }
    Ok(())
}

/// A genuinely kernel-created, service-owned and EMPTY Job Object.
/// It has no method to enroll or launch a child. A WFP restriction,
/// verified debuggee descendant capture, guardian-loss and real OS RIP
/// are separate positive gates before FIN-04 can enable LLDB.
pub(crate) struct EmptyAuditJob {
    _owned: Token,
}
// The owner-scoped handle is NEVER serialized, passed over IPC, or used
// to transfer Job custody to the other SCM service. This repairs the
// recovered split launcher's missing typed Job-handle interface.
#[cfg(feature="fin04w2-split-native")]
impl EmptyAuditJob {
    pub(crate) fn held_split_job_handle(&self)->HANDLE {self._owned.0}
}
pub(crate) fn create_bounded_empty_debugger_job() -> Result<EmptyAuditJob, SupervisorError> {
    create_bounded_empty_split_service_job("LuIdeDebuggerBroker")
}
#[cfg(feature="fin04w2-split-native")]
pub(crate) fn create_bounded_empty_target_job() -> Result<EmptyAuditJob, SupervisorError> {
    create_bounded_empty_split_service_job("LuIdeDebugTargetBroker")
}
fn create_bounded_empty_split_service_job(service_name:&str) -> Result<EmptyAuditJob, SupervisorError> {
    inspect_current_split_service_principal(service_name)?;
    let acl = crate::windows_service_security::JobSecurity::new_split_service_owned(service_name)?;
    // The name is derived only inside the authenticated service process,
    // not from the original untrusted client packet.
    let unique = format!("Local\\LuIdeFin04W2Audit_{:016x}", rand::random::<u64>());
    let name: Vec<u16> = unique.encode_utf16().chain([0]).collect();
    let raw = unsafe { CreateJobObjectW(&acl.attributes, name.as_ptr()) };
    if raw.is_null() { return Err(denied()); }
    let creation_error = unsafe { GetLastError() };
    let job = Token(raw);
    if creation_error == ERROR_ALREADY_EXISTS { return Err(denied()); }
    const CAP: u32 = 16;
    const MEMORY: usize = 512 * 1024 * 1024;
    const REQUIRED: u32 = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS | JOB_OBJECT_LIMIT_JOB_MEMORY;
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = REQUIRED;
    limits.BasicLimitInformation.ActiveProcessLimit = CAP;
    limits.JobMemoryLimit = MEMORY;
    if unsafe {
        SetInformationJobObject(
            job.0, JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0 { return Err(denied()); }
    let mut observed = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    if unsafe {
        QueryInformationJobObject(
            job.0, JobObjectExtendedLimitInformation,
            (&mut observed as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ptr::null_mut(),
        )
    } == 0
        || observed.BasicLimitInformation.LimitFlags != REQUIRED
        || observed.BasicLimitInformation.ActiveProcessLimit != CAP
        || observed.JobMemoryLimit != MEMORY
    { return Err(denied()); }
    // CPU cap uses the same 50% hard cap as the original protected
    // Windows worker, but is configured by the separate SCM guardian.
    // This does not change production debugger or user process limits.
    #[repr(C)]
    struct CpuRate {
        flags: u32,
        rate: u32,
    }
    const HARD_CAP: u32 = 0x1 | 0x4;
    const HALF: u32 = 5000;
    let cpu = CpuRate { flags: HARD_CAP, rate: HALF };
    if unsafe {
        SetInformationJobObject(
            job.0, JobObjectCpuRateControlInformation,
            (&raw const cpu).cast(), size_of::<CpuRate>() as u32,
        )
    } == 0 {
        return Err(denied());
    }
    let mut observed_cpu = CpuRate { flags: 0, rate: 0 };
    if unsafe {
        QueryInformationJobObject(
            job.0, JobObjectCpuRateControlInformation,
            (&mut observed_cpu as *mut CpuRate).cast(),
            size_of::<CpuRate>() as u32, ptr::null_mut(),
        )
    } == 0
        || observed_cpu.flags & HARD_CAP != HARD_CAP
        || observed_cpu.rate != HALF
    {
        return Err(denied());
    }
    let mut usage = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    if unsafe {
        QueryInformationJobObject(
            job.0, JobObjectBasicAccountingInformation,
            (&mut usage as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
            size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            ptr::null_mut(),
        )
    } == 0 || usage.ActiveProcesses != 0 {
        return Err(denied());
    }
    crate::windows_service_security::verify_split_job_kernel_acl(job.0,service_name)?;
    Ok(EmptyAuditJob { _owned: job })
}

// The only admitted executable in W2-C remains the service-verified,
// hash-pinned CI fixture. The split admission deliberately supports an
// ORIGINAL fsynced Running receipt BEFORE a suspended child is resumed.
// This is NOT a generic debugger launcher or native host command.
#[cfg(feature="fin04w2-contained-probe")]
pub(crate) struct PendingFixedWorker<'a> {
    job: &'a EmptyAuditJob,
    child: Token,
    thread: Token,
    active: bool,
}
#[cfg(feature="fin04w2-contained-probe")]
impl Drop for PendingFixedWorker<'_> {
    fn drop(&mut self) {
        if self.active {
            use windows_sys::Win32::System::Threading::{TerminateProcess,WaitForSingleObject};
            // Any failure, timeout or host disconnect kills the actual OS
            // child handle. The independently held Job subsequently reaps
            // all descendants; never leave an unassigned suspended process.
            unsafe { TerminateProcess(self.child.0,77); }
            let _=unsafe { WaitForSingleObject(self.child.0,5000) };
        }
    }
}
#[cfg(feature="fin04w2-contained-probe")]
impl PendingFixedWorker<'_> {
    pub(crate) fn resume_and_verify(mut self)->Result<(),SupervisorError>{
        use windows_sys::Win32::System::Threading::{
            ResumeThread,GetExitCodeProcess,WaitForSingleObject,
        };
        use std::time::{Duration,Instant};
        if unsafe {ResumeThread(self.thread.0)}==u32::MAX {
            return Err(denied());
        }
        // The exact OS Job must show both parent and descendant, then
        // zero live members. A child-reported exit code alone is not enough.
        let until=Instant::now()+Duration::from_secs(15);
        let mut saw_two=false;
        let mut empty=false;
        while Instant::now()<until{
            let mut info=JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            if unsafe {
                QueryInformationJobObject(
                    self.job._owned.0,JobObjectBasicAccountingInformation,
                    (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    ptr::null_mut(),
                )
            }==0{return Err(denied());}
            if info.TotalProcesses>=2 && info.ActiveProcesses>=2 {saw_two=true;}
            if saw_two && info.ActiveProcesses==0 {empty=true;break;}
            std::thread::sleep(Duration::from_millis(100));
        }
        if !saw_two || !empty
            || unsafe {WaitForSingleObject(self.child.0,0)}!=WAIT_OBJECT_0 {
            return Err(denied());
        }
        let mut exit=u32::MAX;
        if unsafe {GetExitCodeProcess(self.child.0,&mut exit)}==0 || exit!=0 {
            return Err(denied());
        }
        self.active=false;
        Ok(())
    }
}
#[cfg(feature="fin04w2-contained-probe")]
impl EmptyAuditJob {
    pub(crate) fn prepare_fixed_probe(
        &self,
        reviewed_exe:&std::path::Path,
        reviewed_sha256:&str,
        owner_private_file:&std::path::Path,
    )->Result<PendingFixedWorker<'_>,SupervisorError>{
        use windows_sys::Win32::System::{
            JobObjects::AssignProcessToJobObject,
            Threading::{
                CreateProcessW,TerminateProcess,WaitForSingleObject,
                CREATE_NO_WINDOW,CREATE_SUSPENDED,PROCESS_INFORMATION,
                STARTUPINFOW,
            },
        };
        if !reviewed_exe.is_absolute() || !owner_private_file.is_absolute()
            || reviewed_exe.to_string_lossy().contains('"')
            || owner_private_file.to_string_lossy().contains('"')
            || owner_private_file.to_string_lossy().contains('\0')
        {return Err(denied());}
        crate::windows_service_security::hash_executable(
            reviewed_exe,reviewed_sha256,
        )?;
        let exe:Vec<u16>=reviewed_exe.as_os_str().encode_wide().chain([0]).collect();
        // Never interpolate client-controlled text. Both absolute paths
        // originate solely in admin-pinned, private service configuration.
        let command=format!(
            "\"{}\" --worker \"{}\"",reviewed_exe.display(),
            owner_private_file.display(),
        );
        let mut wide:Vec<u16>=command.encode_utf16().chain([0]).collect();
        let mut startup=STARTUPINFOW::default();
        startup.cb=size_of::<STARTUPINFOW>() as u32;
        let mut process=PROCESS_INFORMATION::default();
        if unsafe {
            CreateProcessW(
                exe.as_ptr(),wide.as_mut_ptr(),ptr::null(),ptr::null(),0,
                CREATE_SUSPENDED|CREATE_NO_WINDOW,ptr::null(),ptr::null(),
                &startup,&mut process,
            )
        }==0 {return Err(denied());}
        let child=Token(process.hProcess);
        let thread=Token(process.hThread);
        if unsafe {AssignProcessToJobObject(self._owned.0,child.0)}==0 {
            unsafe {TerminateProcess(child.0,77);}
            let _=unsafe {WaitForSingleObject(child.0,5000)};
            return Err(denied());
        }
        let mut included=0i32;
        if unsafe{IsProcessInJob(child.0,self._owned.0,&mut included)}==0
            || included==0 {
            unsafe {TerminateProcess(child.0,77);}
            let _=unsafe {WaitForSingleObject(child.0,5000)};
            return Err(denied());
        }
        Ok(PendingFixedWorker {
            job:self,child,thread,active:true,
        })
    }
    /// W2-B's original exact native admission still uses the same two
    /// phases locally; it needs no host journal because it is explicitly
    /// a disposable OS containment fixture, not an approved operation.
    pub(crate) fn verify_fixed_probe(
        &self,reviewed_exe:&std::path::Path,reviewed_sha256:&str,
        owner_private_file:&std::path::Path,
    )->Result<(),SupervisorError>{
        self.prepare_fixed_probe(
            reviewed_exe,reviewed_sha256,owner_private_file,
        )?.resume_and_verify()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interactive_ci_runner_is_not_the_independent_restricted_service_worker() {
        // The GitHub runner is intentionally NOT the installed independently
        // restricted LuIdeDebuggerBroker identity. A deny is required, even
        // when a parent CI runner happens to be inside some Job Object.
        assert!(inspect_current_service_principal().is_err());
        assert!(inspect_current_worker_prerequisites().is_err());
    }
}
