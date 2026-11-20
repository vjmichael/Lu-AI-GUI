//! CP-12B-R7 Windows evaluation. Explicitly feature-gated v1 count-only,
//! v2 reviewed-count and v3 stage-review exchanges share the same kernel
//! endpoint protections, per-account SID checks, single-use MAC and deadlines.
//! This is not enabled for normal Windows release packages.
use super::{protocol::*, *};
use std::time::Duration;
use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::{
        LocalFree, ERROR_NO_DATA, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING,
        GENERIC_ALL, HANDLE, INVALID_HANDLE_VALUE,
    },
    Security::{
        AclSizeInformation,
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            GetSecurityInfo, SE_KERNEL_OBJECT,
        },
        GetAce, GetAclInformation, GetLengthSid, GetSecurityDescriptorControl, GetTokenInformation,
        IsValidAcl, IsValidSid, RevertToSelf, TokenUser, ACCESS_ALLOWED_ACE, ACE_HEADER,
        ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSID,
        SECURITY_ATTRIBUTES, SE_DACL_PRESENT, SE_DACL_PROTECTED, TOKEN_QUERY, TOKEN_USER,
    },
    Storage::FileSystem::{
        CreateFileW, ReadFile, WriteFile, FILE_ALL_ACCESS, FILE_FLAG_FIRST_PIPE_INSTANCE,
        FILE_READ_DATA, FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
        READ_CONTROL, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT, SYNCHRONIZE,
    },
    System::{
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, ImpersonateNamedPipeClient,
            SetNamedPipeHandleState, PIPE_NOWAIT, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_TYPE_BYTE,
        },
        Threading::{GetCurrentProcess, GetCurrentThread, OpenProcessToken, OpenThreadToken},
    },
};
fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}
fn invalid() -> io::Error {
    io::ErrorKind::InvalidData.into()
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
fn handle(h: HANDLE) -> io::Result<OwnedHandle> {
    if h.is_null() || h == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedHandle::from_raw_handle(h) })
    }
}
// A SID returned by the kernel is copied while its owning buffer is live.
#[derive(Debug, PartialEq, Eq)]
struct Sid(Vec<u32>);
impl Sid {
    unsafe fn copy(p: PSID) -> io::Result<Self> {
        if p.is_null() || unsafe { IsValidSid(p) } == 0 {
            return Err(denied());
        }
        let n = unsafe { GetLengthSid(p) } as usize;
        if !(8..=68).contains(&n) || n % 4 != 0 {
            return Err(denied());
        }
        let mut x = vec![0u32; n / 4];
        unsafe {
            std::ptr::copy_nonoverlapping(p.cast::<u8>(), x.as_mut_ptr().cast(), n);
        }
        Ok(Self(x))
    }
    fn text(&self) -> io::Result<String> {
        let mut p = null_mut();
        if unsafe { ConvertSidToStringSidW(self.0.as_ptr().cast_mut().cast(), &mut p) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let _mem = Local(p.cast());
        let mut n = 0;
        while n < 256 && unsafe { *p.add(n) } != 0 {
            n += 1;
        }
        if n == 256 {
            return Err(invalid());
        }
        String::from_utf16(unsafe { std::slice::from_raw_parts(p, n) }).map_err(|_| invalid())
    }
}
struct Local(*mut std::ffi::c_void);
impl Drop for Local {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
}
fn token_sid(token: &OwnedHandle) -> io::Result<Sid> {
    let mut n = 0;
    unsafe {
        GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut n);
    }
    if n < std::mem::size_of::<TOKEN_USER>() as u32 || n > 4096 {
        return Err(denied());
    }
    let mut buf = vec![0usize; (n as usize).div_ceil(std::mem::size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buf.as_mut_ptr().cast(),
            n,
            &mut n,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    unsafe { Sid::copy((*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid) }
}
fn account() -> io::Result<Sid> {
    let mut t = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut t) } == 0 {
        return Err(io::Error::last_os_error());
    }
    token_sid(&handle(t)?)
}
/// Query the actual kernel object's owner and effective DACL, not the name
/// or the security descriptor we intended to install. Reject open/null ACLs,
/// inherited entries, extra accounts and owner substitution on both ends.
fn verify_endpoint(pipe: &OwnedHandle, expected: &Sid) -> io::Result<()> {
    let mut sid = null_mut();
    let mut dacl = null_mut();
    let mut sd = null_mut();
    let err = unsafe {
        GetSecurityInfo(
            pipe.as_raw_handle(),
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut sid,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut sd,
        )
    };
    let _mem = Local(sd);
    if err != 0 {
        return Err(io::Error::from_raw_os_error(err as i32));
    }
    if &unsafe { Sid::copy(sid)? } != expected || sd.is_null() || dacl.is_null() {
        return Err(denied());
    }
    let mut control = 0;
    let mut revision = 0;
    if unsafe { GetSecurityDescriptorControl(sd, &mut control, &mut revision) } == 0
        || control & (SE_DACL_PRESENT | SE_DACL_PROTECTED) != (SE_DACL_PRESENT | SE_DACL_PROTECTED)
        || unsafe { IsValidAcl(dacl) } == 0
    {
        return Err(denied());
    }
    let mut size = ACL_SIZE_INFORMATION::default();
    if unsafe {
        GetAclInformation(
            dacl,
            (&mut size as *mut ACL_SIZE_INFORMATION).cast(),
            std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
        || size.AceCount != 1
    {
        return Err(denied());
    }
    let mut ptr = null_mut();
    if unsafe { GetAce(dacl, 0, &mut ptr) } == 0 || ptr.is_null() {
        return Err(denied());
    }
    let header = unsafe { &*ptr.cast::<ACE_HEADER>() };
    // ACCESS_ALLOWED_ACE_TYPE is 0; no inheritance or alternate ACE types.
    // A regular allowed ACE has an 8-byte prefix followed by its SID.
    if header.AceType != 0
        || header.AceFlags != 0
        || header.AceSize as usize != 8 + expected.0.len() * 4
    {
        return Err(denied());
    }
    let ace = unsafe { &*ptr.cast::<ACCESS_ALLOWED_ACE>() };
    if !matches!(ace.Mask, GENERIC_ALL | FILE_ALL_ACCESS) {
        return Err(denied());
    }
    let ace_sid = unsafe { Sid::copy((&ace.SidStart as *const u32).cast_mut().cast())? };
    if &ace_sid != expected {
        return Err(denied());
    }
    Ok(())
}
// Verification uses the token associated with the last message actually read,
// not an attacker-supplied process ID or an account name. Never act as the peer.
fn peer(pipe: &OwnedHandle, expected: &Sid) -> bool {
    if unsafe { ImpersonateNamedPipeClient(pipe.as_raw_handle()) } == 0 {
        return false;
    }
    struct Revert;
    impl Drop for Revert {
        fn drop(&mut self) {
            if unsafe { RevertToSelf() } == 0 {
                std::process::abort();
            }
        }
    }
    let revert = Revert;
    let mut t = null_mut();
    let result = if unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut t) } == 0 {
        false
    } else {
        handle(t)
            .and_then(|t| token_sid(&t))
            .is_ok_and(|sid| &sid == expected)
    };
    drop(revert);
    result
}
fn path(id: &[u8; 16]) -> Vec<u16> {
    wide(&format!(r"\\.\pipe\lu-dx-{}", hex(id)))
}
fn create(id: &[u8; 16], expected: &Sid) -> io::Result<OwnedHandle> {
    let sid = expected.text()?;
    // Protected DACL, exact process user (not a group or Administrators).
    let sddl = wide(&format!("O:{sid}D:P(A;;GA;;;{sid})"));
    let mut sd = null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), 1, &mut sd, null_mut())
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let _mem = Local(sd);
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd,
        bInheritHandle: 0,
    };
    let h = handle(unsafe {
        CreateNamedPipeW(
            path(id).as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            8192,
            8192,
            0,
            &sa,
        )
    })?;
    verify_endpoint(&h, expected)?;
    Ok(h)
}
fn expired(stop: &AtomicBool, end: Instant) -> io::Result<()> {
    if stop.load(Ordering::Acquire) || Instant::now() >= end {
        Err(io::ErrorKind::TimedOut.into())
    } else {
        Ok(())
    }
}
fn transfer(
    h: &OwnedHandle,
    b: &mut [u8],
    write: bool,
    stop: &AtomicBool,
    end: Instant,
) -> io::Result<()> {
    let mut pos = 0;
    while pos < b.len() {
        expired(stop, end)?;
        let mut n = 0;
        let count = (b.len() - pos) as u32;
        let rc = unsafe {
            if write {
                WriteFile(
                    h.as_raw_handle(),
                    b[pos..].as_ptr(),
                    count,
                    &mut n,
                    null_mut(),
                )
            } else {
                ReadFile(
                    h.as_raw_handle(),
                    b[pos..].as_mut_ptr(),
                    count,
                    &mut n,
                    null_mut(),
                )
            }
        };
        if rc != 0 {
            if n > count {
                return Err(invalid());
            }
            if n > 0 {
                pos += n as usize;
                continue;
            }
        } else {
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(ERROR_NO_DATA as i32) {
                return Err(e);
            }
        }
        thread::sleep(TICK);
    }
    Ok(())
}
fn read(h: &OwnedHandle, stop: &AtomicBool, end: Instant) -> io::Result<Vec<u8>> {
    let mut size = [0; 4];
    transfer(h, &mut size, false, stop, end)?;
    let n = u32::from_be_bytes(size) as usize;
    if n == 0 || n > MAX {
        return Err(invalid());
    }
    let mut data = vec![0; n];
    transfer(h, &mut data, false, stop, end)?;
    Ok(data)
}
fn write(h: &OwnedHandle, mut data: Vec<u8>, stop: &AtomicBool, end: Instant) -> io::Result<()> {
    if data.is_empty() || data.len() > MAX {
        return Err(invalid());
    }
    transfer(h, &mut (data.len() as u32).to_be_bytes(), true, stop, end)?;
    transfer(h, &mut data, true, stop, end)
}
fn accept(h: &OwnedHandle, stop: &AtomicBool, end: Instant) -> io::Result<()> {
    loop {
        expired(stop, end)?;
        if unsafe { ConnectNamedPipe(h.as_raw_handle(), null_mut()) } != 0 {
            // PIPE_NOWAIT success may mean listening, not connected.
            // Only the connected state admits a request.
            thread::sleep(TICK);
            continue;
        }
        let e = io::Error::last_os_error();
        match e.raw_os_error().map(|n| n as u32) {
            Some(ERROR_PIPE_CONNECTED) => return Ok(()),
            Some(ERROR_PIPE_LISTENING) => thread::sleep(TICK),
            Some(ERROR_NO_DATA) => unsafe {
                DisconnectNamedPipe(h.as_raw_handle());
            },
            _ => return Err(e),
        }
    }
}
fn connect(id: &[u8; 16], expected: &Sid, stop: &AtomicBool, end: Instant) -> io::Result<OwnedHandle> {
    let name = path(id);
    let h = loop {
        expired(stop, end)?;
        // No GENERIC_WRITE (which includes FILE_CREATE_PIPE_INSTANCE).
        match handle(unsafe {
            CreateFileW(
                name.as_ptr(),
                FILE_READ_DATA
                    | FILE_WRITE_DATA
                    | FILE_WRITE_ATTRIBUTES
                    | READ_CONTROL
                    | SYNCHRONIZE,
                0,
                null(),
                OPEN_EXISTING,
                SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                null_mut(),
            )
        }) {
            Ok(h) => break h,
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => thread::sleep(TICK),
            Err(e) => return Err(e),
        }
    };
    verify_endpoint(&h, expected)?;
    let mode = PIPE_READMODE_BYTE | PIPE_NOWAIT;
    if unsafe { SetNamedPipeHandleState(h.as_raw_handle(), &mode, null(), null()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(h)
}
struct Session {
    stop: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}
impl Session {
    fn stop(self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.worker.join();
    }
}
#[derive(Default)]
pub(crate) struct Controller {
    session: Mutex<Option<Session>>,
    closed: AtomicBool,
}
impl Controller {
    pub fn active(&self) -> bool {
        self.session
            .lock()
            .is_ok_and(|s| s.as_ref().is_some_and(|s| !s.worker.is_finished()))
    }
    pub fn stop(&self) -> bool {
        if let Ok(mut lock) = self.session.lock() {
            if let Some(s) = lock.take() {
                s.stop();
                return true;
            }
        }
        false
    }
    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        self.stop();
    }
    pub fn start(&self, summary: Summary) -> Result<String, String> {
        self.start_inner(summary, TTL)
            .map_err(|_| "Local sharing unavailable".into())
    }
    pub(crate) fn start_review(&self, review: ReviewedSummary) -> Result<String, String> {
        self.start_inner_version(review.summary, REVIEW_VERSION, review.finding as u32, None, TTL)
            .map_err(|_| "Local reviewed sharing unavailable".into())
    }
    pub(crate) fn start_stage_review(&self, review: ReviewedStages) -> Result<String, String> {
        self.start_inner_version(review.summary, STAGE_VERSION, review.verdict, Some(review.evidence), TTL)
            .map_err(|_| "Local stage review unavailable".into())
    }
    fn start_inner(&self, summary: Summary, ttl: Duration) -> io::Result<String> {
        self.start_inner_version(summary, VERSION, 0, None, ttl)
    }
    fn start_inner_version(&self, summary: Summary, version:u32, finding:u32, stages:Option<StageEvidence>, ttl: Duration) -> io::Result<String> {
        // Fail before creating a pipe or invitation. Every v2/v3 verdict must
        // already be checked against its original finite native evidence.
        if !summary.valid() || !match version {
            VERSION => finding == 0 && stages.is_none(),
            REVIEW_VERSION => stages.is_none() && Finding::from_wire(finding) == Finding::expected(&summary),
            STAGE_VERSION => stages.as_ref().is_some_and(|e|e.expected(&summary)==Some(finding)),
            _ => false,
        } { return Err(invalid()); }
        let mut lock = self.session.lock().map_err(|_| denied())?;
        if self.closed.load(Ordering::Acquire) {
            return Err(denied());
        }
        if lock.as_ref().is_some_and(|s| !s.worker.is_finished()) {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        if let Some(s) = lock.take() {
            s.stop();
        }
        let id = random::<16>()?;
        let key = random::<32>()?;
        let expected = account()?;
        let h = create(&id, &expected)?;
        let stop = Arc::new(AtomicBool::new(false));
        let w = stop.clone();
        let expires = Instant::now() + ttl;
        let worker = thread::Builder::new()
            .name("lu-diagnostic-share".into())
            .spawn(move || {
                while accept(&h, &w, expires).is_ok() {
                    let end = (Instant::now() + FRAME).min(expires);
                    let authenticated = read(&h, &w, end)
                        .ok()
                        .filter(|_| peer(&h, &expected))
                        .and_then(|b| auth_request_for(&key, &b, version));
                    if let Some(r) = authenticated {
                        // An authenticated request consumes the invitation even if delivery fails.
                        if write(
                            &h,
                            response_full(&key, r.nonce, summary.clone(), version, finding, stages.clone()).encode_to_vec(),
                            &w,
                            end,
                        )
                        .is_ok()
                        {
                            // Do not discard unread response bytes by disconnecting prematurely.
                            // Receiver closes after validation. Whole-frame deadline bounds the wait.
                            let _ = transfer(&h, &mut [0u8; 1], false, &w, end);
                        }
                        break;
                    }
                    if unsafe { DisconnectNamedPipe(h.as_raw_handle()) } == 0 {
                        break;
                    }
                }
                // Drop the only server handle; no persistent path or reusable token.
            })?;
        *lock = Some(Session { stop, worker });
        Ok(format!("lu{version}-{}-{}", hex(&id), hex(&key)))
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.shutdown();
    }
}
pub(crate) fn receive(code: &str) -> Result<Summary, String> {
    receive_version(code, VERSION, &AtomicBool::new(false))
        .map(|(summary, _, _)| summary)
        .map_err(|_| "The invitation is invalid, expired, incompatible or unavailable.".into())
}
pub(crate) fn receive_review_cancellable(code: &str, stop: &AtomicBool) -> Result<ReviewedSummary, String> {
    let (summary, finding, _) = receive_version(code, REVIEW_VERSION, stop).map_err(|_| {
        if stop.load(Ordering::Acquire) { "Review stopped; an already consumed invitation cannot be recalled." }
        else { "The review invitation is invalid, expired, incompatible or unavailable." }
    })?;
    Ok(ReviewedSummary { summary, finding: Finding::from_wire(finding).ok_or("Unverified peer finding")? })
}
pub(crate) fn receive_stage_review_cancellable(code: &str, stop: &AtomicBool) -> Result<ReviewedStages, String> {
    let (summary, verdict, evidence) = receive_version(code, STAGE_VERSION, stop).map_err(|_| {
        if stop.load(Ordering::Acquire) { "Review stopped; an already consumed invitation cannot be recalled." }
        else { "The stage review invitation is invalid, expired, incompatible or unavailable." }
    })?;
    Ok(ReviewedStages { summary, verdict, evidence: evidence.ok_or("Unverified stage evidence")? })
}
#[cfg(test)]
fn receive_inner(code: &str) -> io::Result<Summary> {
    receive_version(code, VERSION, &AtomicBool::new(false)).map(|(summary, _, _)|summary)
}
fn receive_version(code:&str, version:u32, stop:&AtomicBool) -> io::Result<(Summary,u32,Option<StageEvidence>)> {
    if code.len() != 101 { return Err(invalid()); }
    let mut parts = code.split('-');
    if parts.next() != Some(match version {
        VERSION => "lu1", REVIEW_VERSION => "lu2", STAGE_VERSION => "lu3", _ => return Err(invalid()),
    }) { return Err(invalid()); }
    let id = unhex::<16>(parts.next().ok_or_else(invalid)?).ok_or_else(invalid)?;
    let key = unhex::<32>(parts.next().ok_or_else(invalid)?).ok_or_else(invalid)?;
    if parts.next().is_some() { return Err(invalid()); }
    let end = Instant::now() + FRAME;
    expired(stop, end)?;
    let h = connect(&id, &account()?, stop, end)?;
    expired(stop, end)?;
    let nonce = random::<32>()?.to_vec();
    write(&h, request_for(&key, nonce.clone(), version).encode_to_vec(), stop, end)?;
    auth_response_full(&key, &nonce, &read(&h, stop, end)?, version).ok_or_else(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, Write},
        process::{Command, Stdio},
    };
    fn sample() -> Summary {
        Summary {
            complete: 1,
            observed_failure: 2,
            bounded: 3,
            incomplete: 4,
            unavailable: 5,
        }
    }
    fn decode(code: &str) -> ([u8; 16], [u8; 32]) {
        let mut p = code.split('-').skip(1);
        (
            unhex(p.next().unwrap()).unwrap(),
            unhex(p.next().unwrap()).unwrap(),
        )
    }
    fn child(name: &str) -> Command {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args(["--exact", name, "--ignored", "--nocapture"])
            .env("LU_DX_TEST_CHILD", "1");
        c
    }
    #[test]
    fn native_roundtrip_and_owned_endpoint() {
        let c = Controller::default();
        assert!(!c.active());
        let code = c.start(sample()).unwrap();
        let (id, _) = decode(&code);
        assert!(
            create(&id, &account().unwrap()).is_err(),
            "second server must not claim name"
        );
        assert_eq!(receive(&code).unwrap(), sample());
        assert!(receive(&code).is_err());
        c.stop();
        assert!(!c.active());
    }
    #[test]
    fn receiver_rejects_endpoint_with_extra_account() {
        // Deliberately weak disposable test endpoint; no production code or
        // diagnostic payload runs behind it. Reader must reject before sending.
        let expected = account().unwrap();
        let sid = expected.text().unwrap();
        let id = random::<16>().unwrap();
        let key = random::<32>().unwrap();
        let sddl = wide(&format!("O:{sid}D:P(A;;GA;;;{sid})(A;;GR;;;WD)"));
        let mut sd = null_mut();
        assert_ne!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    1,
                    &mut sd,
                    null_mut(),
                )
            },
            0
        );
        let _mem = Local(sd);
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        let h = handle(unsafe {
            CreateNamedPipeW(
                path(&id).as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                8192,
                8192,
                0,
                &sa,
            )
        })
        .unwrap();
        assert_eq!(
            verify_endpoint(&h, &expected).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        let code = format!("lu1-{}-{}", hex(&id), hex(&key));
        assert_eq!(
            receive_inner(&code).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        drop(h);
    }
    #[test]
    fn invalid_auth_incompatible_and_oversize_do_not_consume() {
        let c = Controller::default();
        let code = c.start(sample()).unwrap();
        let (id, key) = decode(&code);
        let wrong = format!("{}{}", &code[..37], "0".repeat(64));
        assert!(receive(&wrong).is_err());
        let stop = AtomicBool::new(false);
        for (version, dx_version) in [(0, DX_VERSION), (VERSION, 1), (VERSION, DX_VERSION + 1)] {
            let mut r = request(&key, vec![9; 32]);
            r.version = version;
            r.dx_version = dx_version;
            r.mac.clear();
            r.mac = mac(&key, b"lu.dx.request.v1\0", &r.encode_to_vec()).to_vec();
            let end = Instant::now() + FRAME;
            let h = connect(&id, &account().unwrap(), &AtomicBool::new(false), end).unwrap();
            write(&h, r.encode_to_vec(), &stop, end).unwrap();
            assert!(read(&h, &stop, end).is_err());
        }
        {
            let end = Instant::now() + FRAME;
            let h = connect(&id, &account().unwrap(), &AtomicBool::new(false), end).unwrap();
            transfer(&h, &mut ((MAX + 1) as u32).to_be_bytes(), true, &stop, end).unwrap();
            assert!(read(&h, &stop, end).is_err());
        }
        assert_eq!(receive(&code).unwrap(), sample());
        c.stop();
    }
    #[test]
    fn revoke_active_frame_expire_rotate_and_shutdown() {
        let c = Controller::default();
        let code = c.start(sample()).unwrap();
        let (id, _) = decode(&code);
        let end = Instant::now() + FRAME;
        let h = connect(&id, &account().unwrap(), &AtomicBool::new(false), end).unwrap();
        transfer(&h, &mut [0], true, &AtomicBool::new(false), end).unwrap();
        let t = Instant::now();
        assert!(c.stop());
        assert!(t.elapsed() < Duration::from_secs(2));
        drop(h);
        assert!(receive(&code).is_err());
        let fresh = c.start_inner(sample(), Duration::from_millis(200)).unwrap();
        assert_ne!(fresh, code);
        thread::sleep(Duration::from_millis(300));
        assert!(receive(&fresh).is_err());
        c.shutdown();
        assert!(c.start(sample()).is_err());
    }
    #[test]
    fn separate_process_exchange() {
        let c = Controller::default();
        let code = c.start(sample()).unwrap();
        let r = child("dx_ipc::windows_pipe::tests::child_receiver")
            .env("LU_DX_TEST_CODE", &code)
            .output()
            .unwrap();
        assert!(
            r.status.success(),
            "child receive failed: {}",
            String::from_utf8_lossy(&r.stdout)
        );
        assert!(receive(&code).is_err());
        c.stop();
    }
    #[test]
    #[ignore = "parent subprocess entry"]
    fn child_receiver() {
        assert_eq!(std::env::var("LU_DX_TEST_CHILD").as_deref(), Ok("1"));
        assert_eq!(
            receive(&std::env::var("LU_DX_TEST_CODE").unwrap()).unwrap(),
            sample()
        );
    }
    #[test]
    fn abrupt_sender_death_then_new_invitation() {
        let mut p = child("dx_ipc::windows_pipe::tests::child_sender")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = p.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let reader = thread::spawn(move || {
            for l in io::BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(code) = l.strip_prefix("INVITE:") {
                    let _ = tx.send(code.to_owned());
                    break;
                }
            }
        });
        let code = rx.recv_timeout(Duration::from_secs(15));
        let _ = p.kill();
        let status = p.wait().unwrap();
        let _ = reader.join();
        assert!(!status.success());
        let code = code.expect("child sender unavailable");
        assert!(receive(&code).is_err());
        let c = Controller::default();
        let fresh = c.start(sample()).unwrap();
        assert_ne!(fresh, code);
        assert!(receive(&code).is_err());
        assert_eq!(receive(&fresh).unwrap(), sample());
        c.stop();
    }
    #[test]
    #[ignore = "parent subprocess entry"]
    fn child_sender() {
        let c = Controller::default();
        let code = c.start(sample()).unwrap();
        println!("INVITE:{code}");
        io::stdout().flush().unwrap();
        thread::sleep(Duration::from_secs(60));
    }
    #[test]
    fn response_nonce_mac_and_privacy_contract() {
        let key = [42; 32];
        let nonce = vec![9; 32];
        let mut r = response(&key, nonce.clone(), sample());
        assert_eq!(
            auth_response(&key, &nonce, &r.encode_to_vec()),
            Some(sample())
        );
        assert!(auth_response(&key, &[8; 32], &r.encode_to_vec()).is_none());
        r.summary.as_mut().unwrap().complete += 1;
        assert!(auth_response(&key, &nonce, &r.encode_to_vec()).is_none());
        let mut k = [0; 32];
        k[..20].fill(0x0b);
        assert_eq!(
            hex(&mac(&k, b"", b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
    // CP-12B-R7: version-separated Windows review and stage-review transport.
    // These execute the same kernel pipe, SID, one-shot and HMAC path as v1.
    fn stage_sample() -> ReviewedStages {
        let s = Summary { complete: 1, observed_failure: 1, bounded: 0, incomplete: 0, unavailable: 0 };
        let e = StageEvidence { stages: vec![stage::StageCount { stage: 20, observed: 1, bounded: 0 }] };
        ReviewedStages::checked(s, e, "REFUSED").unwrap()
    }
    fn assert_stage(received: ReviewedStages) {
        let expected = stage_sample();
        assert_eq!(received.summary, expected.summary);
        assert_eq!(received.evidence, expected.evidence);
        assert_eq!(received.verdict, expected.verdict);
    }
    #[test]
    fn cp12br7_versioned_review_native_roundtrip_and_replay() {
        let c = Controller::default();
        let summary = sample();
        let finding = Finding::expected(&summary).unwrap();
        let code = c.start_review(ReviewedSummary { summary: summary.clone(), finding }).unwrap();
        assert!(code.starts_with("lu2-"));
        assert!(receive(&code).is_err());
        assert!(receive_stage_review_cancellable(&code, &AtomicBool::new(false)).is_err());
        let received = receive_review_cancellable(&code, &AtomicBool::new(false)).unwrap();
        assert_eq!(received.summary, summary);
        assert_eq!(received.finding, finding);
        assert!(receive_review_cancellable(&code, &AtomicBool::new(false)).is_err());
        assert!(c.stop());
        assert!(!c.active());
    }
    #[test]
    fn cp12br7_stage_native_roundtrip_revocation_rotation_and_replay() {
        let c = Controller::default();
        let old = c.start_stage_review(stage_sample()).unwrap();
        assert!(old.starts_with("lu3-"));
        assert!(receive(&old).is_err());
        assert!(receive_review_cancellable(&old, &AtomicBool::new(false)).is_err());
        assert_stage(receive_stage_review_cancellable(&old, &AtomicBool::new(false)).unwrap());
        assert!(receive_stage_review_cancellable(&old, &AtomicBool::new(false)).is_err());
        c.stop();
        let revoked = c.start_stage_review(stage_sample()).unwrap();
        assert!(c.stop());
        assert!(receive_stage_review_cancellable(&revoked, &AtomicBool::new(false)).is_err());
        let fresh = c.start_stage_review(stage_sample()).unwrap();
        assert_ne!(old, fresh); assert_ne!(revoked, fresh);
        assert!(receive_stage_review_cancellable(&revoked, &AtomicBool::new(false)).is_err());
        assert_stage(receive_stage_review_cancellable(&fresh, &AtomicBool::new(false)).unwrap());
        c.shutdown();
        assert!(c.start_stage_review(stage_sample()).is_err());
    }
    #[test]
    fn cp12br7_stage_malformed_and_tampered_evidence_denied() {
        let c = Controller::default();
        let mut bad = stage_sample(); bad.verdict = 9;
        assert!(c.start_stage_review(bad).is_err());
        let mut bad = stage_sample(); bad.evidence.stages[0].observed = 63;
        assert!(c.start_stage_review(bad).is_err());
        let k = [42u8; 32]; let nonce = vec![9u8; 32];
        let sample = stage_sample();
        let r = response_full(&k, nonce.clone(), sample.summary.clone(), STAGE_VERSION, sample.verdict, Some(sample.evidence.clone()));
        assert!(auth_response_full(&k, &nonce, &r.encode_to_vec(), STAGE_VERSION).is_some());
        assert!(auth_response_full(&k, &nonce, &r.encode_to_vec(), REVIEW_VERSION).is_none());
        assert!(auth_response_full(&k, &[0;32], &r.encode_to_vec(), STAGE_VERSION).is_none());
        let mut tampered = r.clone(); tampered.stages.as_mut().unwrap().stages[0].observed = 2;
        assert!(auth_response_full(&k, &nonce, &tampered.encode_to_vec(), STAGE_VERSION).is_none());
        let mut tampered = r.clone(); tampered.stages.as_mut().unwrap().stages[0].stage = 19;
        assert!(auth_response_full(&k, &nonce, &tampered.encode_to_vec(), STAGE_VERSION).is_none());
        let mut tampered = r.clone(); tampered.version = REVIEW_VERSION;
        assert!(auth_response_full(&k, &nonce, &tampered.encode_to_vec(), STAGE_VERSION).is_none());
    }
    #[test]
    fn cp12br7_stage_cancel_mid_frame_keeps_invitation_unconsumed() {
        let c = Controller::default();
        let code = c.start_stage_review(stage_sample()).unwrap();
        let (id, _) = decode(&code);
        let end = Instant::now() + FRAME;
        let h = connect(&id, &account().unwrap(), &AtomicBool::new(false), end).unwrap();
        transfer(&h, &mut [0u8], true, &AtomicBool::new(false), end).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let test_stop = Arc::clone(&stop);
        let start = Instant::now();
        let code_for_thread = code.clone();
        let task = thread::spawn(move || receive_stage_review_cancellable(&code_for_thread, &test_stop));
        thread::sleep(Duration::from_millis(100));
        stop.store(true, Ordering::Release);
        assert!(task.join().unwrap().is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        // Revocation cancels the server's incomplete frame immediately.
        assert!(c.stop()); drop(h);
        assert!(receive_stage_review_cancellable(&code, &AtomicBool::new(false)).is_err());
    }
    #[test]
    fn cp12br7_stage_separate_process_authenticated_exchange() {
        let c = Controller::default();
        let code = c.start_stage_review(stage_sample()).unwrap();
        let child = child("dx_ipc::windows_pipe::tests::cp12br7_stage_child_receiver")
            .env("LU_DX_TEST_CODE", &code).output().unwrap();
        assert!(child.status.success(), "child exchange failed: {}", String::from_utf8_lossy(&child.stdout));
        assert!(receive_stage_review_cancellable(&code, &AtomicBool::new(false)).is_err());
        c.stop();
    }
    #[test]
    #[ignore = "parent subprocess entry"]
    fn cp12br7_stage_child_receiver() {
        assert_eq!(std::env::var("LU_DX_TEST_CHILD").as_deref(), Ok("1"));
        assert_stage(receive_stage_review_cancellable(&std::env::var("LU_DX_TEST_CODE").unwrap(), &AtomicBool::new(false)).unwrap());
    }
    #[test]
    fn cp12br7_stage_sender_crash_invalidates_invitation() {
        let mut p = child("dx_ipc::windows_pipe::tests::cp12br7_stage_child_sender")
            .stdout(Stdio::piped()).spawn().unwrap();
        let stdout = p.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let reader = thread::spawn(move || {
            for line in io::BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(code) = line.strip_prefix("INVITE:") { let _ = tx.send(code.to_owned()); break; }
            }
        });
        let code = rx.recv_timeout(Duration::from_secs(15));
        let _ = p.kill(); let status = p.wait().unwrap(); let _ = reader.join();
        assert!(!status.success()); let old = code.expect("child sender not ready");
        assert!(receive_stage_review_cancellable(&old, &AtomicBool::new(false)).is_err());
        let c = Controller::default(); let fresh = c.start_stage_review(stage_sample()).unwrap();
        assert_ne!(old, fresh);
        assert!(receive_stage_review_cancellable(&old, &AtomicBool::new(false)).is_err());
        assert_stage(receive_stage_review_cancellable(&fresh, &AtomicBool::new(false)).unwrap());
        c.stop();
    }
    #[test]
    #[ignore = "parent subprocess entry"]
    fn cp12br7_stage_child_sender() {
        let c = Controller::default();
        let code = c.start_stage_review(stage_sample()).unwrap();
        println!("INVITE:{code}"); io::stdout().flush().unwrap();
        thread::sleep(Duration::from_secs(60));
    }
    #[test]
    #[ignore = "requires separately provisioned disposable Windows standard account"]
    fn cp12br7_stage_different_account_kernel_denial() {
        let user = wide(&std::env::var("LU_DX_LAB_USER").expect("disposable account required"));
        let pass = wide(&std::env::var("LU_DX_LAB_PASSWORD").expect("disposable account required"));
        use windows_sys::Win32::{Foundation::ERROR_ACCESS_DENIED,Security::{ImpersonateLoggedOnUser,LogonUserW, LOGON32_LOGON_INTERACTIVE,LOGON32_PROVIDER_DEFAULT}};
        let c = Controller::default(); let code = c.start_stage_review(stage_sample()).unwrap();
        let (id, _) = decode(&code); let mut t = null_mut();
        assert_ne!(unsafe { LogonUserW(user.as_ptr(), wide(".").as_ptr(), pass.as_ptr(), LOGON32_LOGON_INTERACTIVE, LOGON32_PROVIDER_DEFAULT, &mut t) }, 0);
        let t = handle(t).unwrap(); assert_ne!(token_sid(&t).unwrap(), account().unwrap());
        assert_ne!(unsafe { ImpersonateLoggedOnUser(t.as_raw_handle()) }, 0);
        struct Revert; impl Drop for Revert { fn drop(&mut self) { if unsafe { RevertToSelf() } == 0 { std::process::abort() } } }
        let guard = Revert;
        let foreign = handle(unsafe { CreateFileW(path(&id).as_ptr(), FILE_READ_DATA|FILE_WRITE_DATA|FILE_WRITE_ATTRIBUTES|READ_CONTROL|SYNCHRONIZE, 0, null(), OPEN_EXISTING, SECURITY_SQOS_PRESENT|SECURITY_IDENTIFICATION, null_mut()) });
        drop(guard);
        assert_eq!(foreign.unwrap_err().raw_os_error(),Some(ERROR_ACCESS_DENIED as i32));
        assert_stage(receive_stage_review_cancellable(&code,&AtomicBool::new(false)).unwrap()); c.stop();
    }
    #[test]
    #[ignore = "isolated disposable Windows lab only; creates and deletes one standard account"]
    fn disposable_native_account_lifecycle() {
        use windows_sys::Win32::NetworkManagement::NetManagement::{
            NetUserAdd, NetUserDel, UF_NORMAL_ACCOUNT, UF_SCRIPT, USER_INFO_1, USER_PRIV_USER,
        };
        assert_eq!(std::env::var("LU_DX_DISPOSABLE_LAB").as_deref(), Ok("1"));
        let mut name = wide(&format!("LuDx{}", hex(&random::<6>().unwrap())));
        let mut pass = wide(&format!("Dx!9a{}", hex(&random::<16>().unwrap())));
        let info = USER_INFO_1 {
            usri1_name: name.as_mut_ptr(),
            usri1_password: pass.as_mut_ptr(),
            usri1_priv: USER_PRIV_USER,
            usri1_flags: UF_SCRIPT | UF_NORMAL_ACCOUNT,
            ..Default::default()
        };
        let mut err = 0;
        let rc = unsafe { NetUserAdd(null(), 1, (&info as *const USER_INFO_1).cast(), &mut err) };
        assert_eq!(
            rc, 0,
            "native account provisioning failed (status {rc}, field {err})"
        );
        struct Delete(Vec<u16>);
        impl Drop for Delete {
            fn drop(&mut self) {
                unsafe {
                    if !self.0.is_empty() {
                        NetUserDel(null(), self.0.as_ptr());
                    }
                }
            }
        }
        let mut guard = Delete(name.clone());
        let result = std::panic::catch_unwind(|| check_foreign_account(&name, &pass));
        let rc = unsafe { NetUserDel(null(), name.as_ptr()) };
        if rc == 0 {
            guard.0.clear();
        }
        assert_eq!(rc, 0, "disposable account cleanup failed");
        result.unwrap();
    }
    #[test]
    #[ignore = "requires separately provisioned disposable Windows standard account"]
    fn different_account_kernel_access_denied() {
        let user = wide(&std::env::var("LU_DX_LAB_USER").expect("disposable account required"));
        let pass = wide(&std::env::var("LU_DX_LAB_PASSWORD").expect("disposable account required"));
        check_foreign_account(&user, &pass);
    }
    fn check_foreign_account(user: &[u16], pass: &[u16]) {
        use windows_sys::Win32::{
            Foundation::ERROR_ACCESS_DENIED,
            Security::{
                ImpersonateLoggedOnUser, LogonUserW, LOGON32_LOGON_INTERACTIVE,
                LOGON32_PROVIDER_DEFAULT,
            },
        };
        let c = Controller::default();
        let code = c.start(sample()).unwrap();
        let (id, _) = decode(&code);
        let mut t = null_mut();
        assert_ne!(
            unsafe {
                LogonUserW(
                    user.as_ptr(),
                    wide(".").as_ptr(),
                    pass.as_ptr(),
                    LOGON32_LOGON_INTERACTIVE,
                    LOGON32_PROVIDER_DEFAULT,
                    &mut t,
                )
            },
            0,
            "disposable account logon failed"
        );
        let t = handle(t).unwrap();
        assert_ne!(token_sid(&t).unwrap(), account().unwrap());
        assert_ne!(unsafe { ImpersonateLoggedOnUser(t.as_raw_handle()) }, 0);
        struct Revert;
        impl Drop for Revert {
            fn drop(&mut self) {
                if unsafe { RevertToSelf() } == 0 {
                    std::process::abort()
                }
            }
        }
        let g = Revert;
        // A direct kernel open under the alternate standard-account token must fail even with a known name and key.
        let opened = handle(unsafe {
            CreateFileW(
                path(&id).as_ptr(),
                FILE_READ_DATA
                    | FILE_WRITE_DATA
                    | FILE_WRITE_ATTRIBUTES
                    | READ_CONTROL
                    | SYNCHRONIZE,
                0,
                null(),
                OPEN_EXISTING,
                SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                null_mut(),
            )
        });
        drop(g);
        assert_eq!(
            opened.unwrap_err().raw_os_error(),
            Some(ERROR_ACCESS_DENIED as i32)
        );
        assert_eq!(receive(&code).unwrap(), sample());
        c.stop();
    }
}
