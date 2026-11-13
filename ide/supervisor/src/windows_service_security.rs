//! D7D Windows recovery service identity and named-pipe security.
//!
//! Service service-name identity is checked against an OS token, not a caller
//! provided env variable. Service DACL is explicit: LocalSystem + service SID
//! may create server instances; only the installer-pinned host account may
//! read/write data (not FILE_CREATE_PIPE_INSTANCE). Remote clients are rejected.
//!
//! A pipe request additionally requires impersonation, client PID from the
//! kernel, matching owner PID/creation time, matching client/owner TokenUser,
//! and an approved *installed* host executable path + fixed SHA-256.
//! Opening a pipe is not executable permission or a recovery receipt.
use crate::SupervisorError;
use sha2::{Digest, Sha256};
use std::{
    ffi::{c_void, OsStr},
    fs::{self, File, OpenOptions},
    io::Read,
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::Path,
    ptr,
    sync::atomic::{AtomicU32, Ordering},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HANDLE},
    Security::{
        AclSizeInformation,
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            ConvertStringSidToSidW, GetNamedSecurityInfoW, GetSecurityInfo, SE_FILE_OBJECT,
            SE_KERNEL_OBJECT,
        },
        CheckTokenMembership, DuplicateToken, EqualSid, GetAce, GetAclInformation,
        GetSecurityDescriptorControl, GetTokenInformation, IsTokenRestricted, LookupAccountNameW,
        RevertToSelf, SecurityImpersonation, TokenGroups, TokenIsAppContainer, TokenRestrictedSids,
        TokenUser, ACCESS_ALLOWED_ACE, ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION,
        SECURITY_ATTRIBUTES, SE_DACL_PROTECTED, SID_AND_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_GROUPS,
        TOKEN_QUERY, TOKEN_USER,
    },
    System::{
        Pipes::{
            GetNamedPipeClientProcessId, GetNamedPipeServerProcessId, ImpersonateNamedPipeClient,
        },
        Threading::{
            GetCurrentProcess, GetCurrentThread, GetProcessTimes, OpenProcess, OpenProcessToken,
            OpenThreadToken, QueryFullProcessImageNameW,
        },
    },
};
const SERVICE_NAME: &str = "LuIdeRecoveryBroker";
pub const SERVICE_ENDPOINT: &str = r"\\.\pipe\LuIdeRecoveryBroker.v1";
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const SYNCHRONIZE: u32 = 0x0010_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_SHARE_READ: u32 = 1;
const MAX_IMAGE_BYTES: u64 = 128 * 1024 * 1024;
fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}
pub fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain([0]).collect()
}
pub fn ws(s: &str) -> Vec<u16> {
    wide(OsStr::new(s))
}
struct Handle(HANDLE);
impl Handle {
    fn new(value: HANDLE) -> Result<Self, SupervisorError> {
        if value.is_null() {
            Err(denied())
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
struct OwnedSid(*mut c_void);
impl Drop for OwnedSid {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
}
pub fn sid_of_service() -> Result<String, SupervisorError> {
    sid_of_named_service(SERVICE_NAME)
}
// Fixed two independently installed service identities, never a request-
 // supplied name. The recovery service retains its original identity.
pub(crate) fn sid_of_named_service(service_name: &str) -> Result<String, SupervisorError> {
    if !matches!(service_name, "LuIdeRecoveryBroker" | "LuIdeDebuggerBroker") {
        return Err(denied());
    }
    let name = ws(&format!("NT SERVICE\\{service_name}"));
    let mut sid_size = 0_u32;
    let mut domain_size = 0_u32;
    let mut use_type = 0_i32;
    unsafe {
        LookupAccountNameW(
            ptr::null(),
            name.as_ptr(),
            ptr::null_mut(),
            &mut sid_size,
            ptr::null_mut(),
            &mut domain_size,
            &mut use_type,
        );
    }
    if sid_size < 8 || sid_size > 1024 || domain_size > 1024 {
        return Err(denied());
    }
    let mut sid = vec![0_u8; sid_size as usize];
    let mut domain = vec![0_u16; domain_size as usize];
    if unsafe {
        LookupAccountNameW(
            ptr::null(),
            name.as_ptr(),
            sid.as_mut_ptr().cast(),
            &mut sid_size,
            domain.as_mut_ptr(),
            &mut domain_size,
            &mut use_type,
        )
    } == 0
    {
        return Err(denied());
    }
    let mut string: *mut u16 = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(sid.as_mut_ptr().cast(), &mut string) } == 0
        || string.is_null()
    {
        return Err(denied());
    }
    let len = (0..256)
        .find(|n| unsafe { *string.add(*n) } == 0)
        .ok_or_else(denied)?;
    let value = String::from_utf16(unsafe { std::slice::from_raw_parts(string, len) })
        .map_err(|_| denied())?;
    unsafe {
        LocalFree(string.cast());
    }
    if !value.starts_with("S-1-5-80-") {
        return Err(denied());
    }
    Ok(value)
}
fn parse_sid(text: &str) -> Result<OwnedSid, SupervisorError> {
    let mut sid = ptr::null_mut();
    if unsafe { ConvertStringSidToSidW(ws(text).as_ptr(), &mut sid) } == 0 {
        return Err(denied());
    }
    Ok(OwnedSid(sid))
}
/// CheckTokenMembership explicitly requires an impersonation token.
/// Never pass the service's OpenProcessToken primary-token handle directly:
/// the Windows SCM native test returned startup stage 103 at that call.
/// Duplicate only for a membership query, preserving restricted SID semantics.
/// Both failures and a missing/disabled service group deny admission.
fn enabled_member_of_primary_token(
    primary: HANDLE,
    sid: *mut c_void,
) -> Result<bool, SupervisorError> {
    let mut duplicate = ptr::null_mut();
    if unsafe { DuplicateToken(primary, SecurityImpersonation, &mut duplicate) } == 0 {
        return Err(denied());
    }
    let token = Handle::new(duplicate)?;
    let mut is_member = 0i32;
    if unsafe { CheckTokenMembership(token.0, sid, &mut is_member) } == 0 {
        return Err(denied());
    }
    Ok(is_member != 0)
}

/// Account identity is TokenUser, not a TokenGroups membership assertion.
/// LocalService is a logon identity, so CheckTokenMembership alone is wrong.
fn token_user_is(token: HANDLE, expected: *mut c_void) -> Result<bool, SupervisorError> {
    let mut len = 0u32;
    unsafe {
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut len);
    }
    if len < size_of::<TOKEN_USER>() as u32 || len > 8192 {
        return Err(denied());
    }
    let mut storage = vec![0usize; (len as usize).div_ceil(size_of::<usize>())];
    if unsafe { GetTokenInformation(token, TokenUser, storage.as_mut_ptr().cast(), len, &mut len) }
        == 0
    {
        return Err(denied());
    }
    let user = unsafe { (*(storage.as_ptr() as *const TOKEN_USER)).User.Sid };
    if user.is_null() {
        return Err(denied());
    }
    Ok(unsafe { EqualSid(user, expected) } != 0)
}

fn token_user_equal(a: HANDLE, b: HANDLE) -> Result<bool, SupervisorError> {
    fn sid(token: HANDLE) -> Result<Vec<u8>, SupervisorError> {
        let mut len = 0_u32;
        unsafe {
            GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut len);
        }
        if len < size_of::<TOKEN_USER>() as u32 || len > 8192 {
            return Err(denied());
        }
        let mut bytes = vec![0_usize; (len as usize).div_ceil(size_of::<usize>())];
        if unsafe {
            GetTokenInformation(token, TokenUser, bytes.as_mut_ptr().cast(), len, &mut len)
        } == 0
        {
            return Err(denied());
        }
        let user_sid = unsafe { (*(bytes.as_ptr() as *const TOKEN_USER)).User.Sid };
        if user_sid.is_null() {
            return Err(denied());
        }
        // Copy the compact textual SID while token allocation remains alive.
        let mut string = ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(user_sid, &mut string) } == 0 || string.is_null() {
            return Err(denied());
        }
        let length = (0..256)
            .find(|n| unsafe { *string.add(*n) } == 0)
            .ok_or_else(denied)?;
        let result = unsafe { std::slice::from_raw_parts(string, length) };
        let bytes = String::from_utf16(result)
            .map_err(|_| denied())?
            .into_bytes();
        unsafe {
            LocalFree(string.cast());
        }
        Ok(bytes)
    }
    Ok(sid(a)? == sid(b)?)
}
// Fixed numeric SCM bootstrap evidence only. Never log token contents,
// account secrets, or allow the diagnostic stage to change admission policy.
static SERVICE_IDENTITY_STAGE: AtomicU32 = AtomicU32::new(0);
pub(crate) fn service_identity_stage() -> u32 {
    SERVICE_IDENTITY_STAGE.load(Ordering::Relaxed)
}
pub fn assert_restricted_local_service() -> Result<(), SupervisorError> {
    SERVICE_IDENTITY_STAGE.store(1, Ordering::Relaxed);
    let mut raw = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE, &mut raw) }
        == 0
    {
        return Err(denied());
    }
    let token = Handle::new(raw)?;
    SERVICE_IDENTITY_STAGE.store(2, Ordering::Relaxed);
    let service_sid = parse_sid(&sid_of_service()?)?;
    let local_service_sid = parse_sid("S-1-5-19")?;
    SERVICE_IDENTITY_STAGE.store(3, Ordering::Relaxed);
    if !enabled_member_of_primary_token(token.0, service_sid.0)? {
        return Err(denied());
    }
    SERVICE_IDENTITY_STAGE.store(4, Ordering::Relaxed);
    // Service must NOT be LocalSystem and must run with restricted SIDs.
    if unsafe { IsTokenRestricted(token.0) } == 0 {
        return Err(denied());
    }
    SERVICE_IDENTITY_STAGE.store(5, Ordering::Relaxed);
    if !token_user_is(token.0, local_service_sid.0)? {
        return Err(denied());
    }
    SERVICE_IDENTITY_STAGE.store(6, Ordering::Relaxed);
    Ok(())
}
pub struct PipeSecurity {
    descriptor: OwnedSid,
    pub attributes: SECURITY_ATTRIBUTES,
}
pub fn validate_approved_host_user_sid(value: &str) -> Result<(), SupervisorError> {
    if value.len() > 160
        || !value.starts_with("S-1-5-21-")
        || value.split('-').count() != 8
        || !value
            .split('-')
            .skip(4)
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(denied());
    }
    let _parsed = parse_sid(value)?;
    Ok(())
}
impl PipeSecurity {
    pub fn new(approved_user_sid: &str) -> Result<Self, SupervisorError> {
        Self::new_for_service(approved_user_sid, &sid_of_service()?)
    }
    pub(crate) fn new_for_service(
        approved_user_sid: &str,
        pinned_service_sid: &str,
    ) -> Result<Self, SupervisorError> {
        validate_approved_host_user_sid(approved_user_sid)?;
        // An arbitrary SID cannot be introduced from a caller's request.
        if !pinned_service_sid.starts_with("S-1-5-80-") {
            return Err(denied());
        }
        let _parsed = parse_sid(pinned_service_sid)?;
        // For this exact installer-pinned host SID, allow mapped FILE
        // read/write data, attributes, EAs, READ_CONTROL and SYNCHRONIZE.
        // Exclude bit 4: FILE_APPEND_DATA is FILE_CREATE_PIPE_INSTANCE on
        // named pipes. No WRITE_DAC, WRITE_OWNER, DELETE or other principal.
        // Native service attestation still verifies the exact kernel ACE.
        // The service must NOT try to assign SYSTEM as an object owner: it
        // runs as restricted LocalService and cannot assume that privilege.
        // Its own kernel-verified service SID receives explicit server rights.
        // No BA grant on the IPC endpoint: administrators are machine TCB
        // but do not need to connect as a desktop worker.
        //
        // SCM services execute at SYSTEM mandatory integrity. Without an
        // explicit pipe label, a medium/high desktop host can have the exact
        // approved TokenUser in this DACL but still be denied by Mandatory
        // Integrity Control (MIC), before DACL evaluation. The explicit
        // MEDIUM mandatory label permits that desktop host to request its
        // narrow client rights, while NO_WRITE_UP still prevents a LOW
        // integrity AppContainer from writing to this trusted IPC endpoint.
        // Authorization still requires kernel PID/token + pinned image hash.
        let sddl = format!(
            "D:P(A;;GA;;;SY)(A;;GA;;;{})(A;;0x0012019b;;;{})S:(ML;;NW;;;ME)",
            pinned_service_sid,
            approved_user_sid
        );
        let mut descriptor = ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                ws(&sddl).as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } == 0
            || descriptor.is_null()
        {
            return Err(denied());
        }
        let mut result = Self {
            descriptor: OwnedSid(descriptor),
            attributes: SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            },
        };
        result.attributes.lpSecurityDescriptor = result.descriptor.0;
        Ok(result)
    }
}
/// Independently verify the *kernel-created* pipe DACL matches the
/// installer-pinned identity. Inspect actual Win32 security rather than
/// relying on a successful ConvertStringSecurityDescriptorTo... call.
/// Fail service startup if any inherited/unknown ACE or pipe-create right
/// appears; no executable action can be admitted from a malformed endpoint.
pub fn verify_created_pipe_security(
    pipe: HANDLE,
    approved_user_sid: &str,
) -> Result<(), SupervisorError> {
    verify_created_pipe_security_for(pipe, approved_user_sid, &sid_of_service()?)
}
pub(crate) fn verify_created_pipe_security_for(
    pipe: HANDLE,
    approved_user_sid: &str,
    pinned_service_sid: &str,
) -> Result<(), SupervisorError> {
    validate_approved_host_user_sid(approved_user_sid)?;
    if !pinned_service_sid.starts_with("S-1-5-80-") {
        return Err(denied());
    }
    let user = parse_sid(approved_user_sid)?;
    let service = parse_sid(pinned_service_sid)?;
    let system = parse_sid("S-1-5-18")?;
    let mut dacl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    if unsafe {
        // Named-pipe handles are FILE objects for the Windows security
        // object-type API, not generic SE_KERNEL_OBJECT handles. Querying
        // the wrong type could falsely reject an otherwise protected pipe.
        GetSecurityInfo(
            pipe,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        )
    } != 0
        || dacl.is_null()
        || descriptor.is_null()
    {
        return Err(denied());
    }
    let _owner = OwnedSid(descriptor);
    let mut control = 0u16;
    let mut revision = 0u32;
    if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0
        || control & SE_DACL_PROTECTED == 0
    {
        return Err(denied());
    }
    let mut info = ACL_SIZE_INFORMATION::default();
    if unsafe {
        GetAclInformation(
            dacl,
            (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
        || info.AceCount != 3
    {
        return Err(denied());
    }
    let (mut system_found, mut service_found, mut user_found) = (false, false, false);
    for index in 0..info.AceCount {
        let mut entry = ptr::null_mut();
        if unsafe { GetAce(dacl, index, &mut entry) } == 0 || entry.is_null() {
            return Err(denied());
        }
        let ace = unsafe { &*(entry as *const ACCESS_ALLOWED_ACE) };
        if ace.Header.AceType != 0
            || ace.Header.AceFlags != 0
            || ace.Header.AceSize < size_of::<ACCESS_ALLOWED_ACE>() as u16
        {
            return Err(denied());
        }
        let sid = (&ace.SidStart as *const u32).cast_mut().cast();
        if unsafe { EqualSid(sid, system.0) } != 0 {
            if system_found {
                return Err(denied());
            }
            system_found = true;
        } else if unsafe { EqualSid(sid, service.0) } != 0 {
            if service_found {
                return Err(denied());
            }
            service_found = true;
        } else if unsafe { EqualSid(sid, user.0) } != 0 {
            // Read + write + synchronize, not FILE_CREATE_PIPE_INSTANCE (4).
            const ALLOWED: u32 = 0x0012_019b;
            // Do not accept a kernel ACE with extra WRITE_DAC, WRITE_OWNER,
            // DELETE or other rights merely because it includes the three
            // requested client rights. The installed exact-user ACL must be
            // *exactly* data read, data write and synchronize, no pipe creation.
            if user_found || ace.Mask != ALLOWED {
                return Err(denied());
            }
            user_found = true;
        } else {
            return Err(denied());
        }
    }
    if !system_found || !service_found || !user_found {
        return Err(denied());
    }
    Ok(())
}

pub struct VerifiedClient {
    pub process: HANDLE, // borrowed native process handle until auth scope ends
    pub pid: u32,
    pub creation_time: u64,
}
pub struct VerifiedClientLease {
    handle: Handle,
    pub pid: u32,
    pub creation_time: u64,
}
impl VerifiedClientLease {
    pub fn process_handle(&self) -> HANDLE {
        self.handle.0
    }
}
fn process_creation(process: HANDLE) -> Result<u64, SupervisorError> {
    use windows_sys::Win32::Foundation::FILETIME;
    let (mut c, mut e, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    if unsafe { GetProcessTimes(process, &mut c, &mut e, &mut k, &mut u) } == 0 {
        return Err(denied());
    }
    Ok(((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64)
}
pub(crate) fn hash_executable(
    expected_path: &Path,
    expected_hash: &str,
) -> Result<(), SupervisorError> {
    if expected_hash.len() != 64
        || !expected_hash.bytes().all(|b| b.is_ascii_hexdigit())
        || !expected_path.is_absolute()
    {
        return Err(denied());
    }
    let md = fs::symlink_metadata(expected_path).map_err(|_| denied())?;
    if !md.is_file() || md.file_attributes() & 0x400 != 0 || md.len() > MAX_IMAGE_BYTES {
        return Err(denied());
    }
    let mut opts = OpenOptions::new();
    opts.read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let mut file = opts.open(expected_path).map_err(|_| denied())?;
    let mut hash = Sha256::new();
    let mut buf = [0_u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|_| denied())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    if format!("{:x}", hash.finalize()) != expected_hash {
        return Err(denied());
    }
    Ok(())
}
struct Impersonation;
impl Drop for Impersonation {
    fn drop(&mut self) {
        unsafe {
            RevertToSelf();
        }
    }
}
/// FIN-04W2: an independent service must never trust a claimed PID or
/// process birth timestamp carried inside its fixed broker packet. Obtain
/// both directly from Windows' named-pipe kernel association and recheck
/// against the same original process handle inside authenticate_client.
pub(crate) fn authenticate_client_from_kernel_pipe(
    pipe: HANDLE,
    approved_host_path: &Path,
    approved_host_sha256: &str,
    approved_host_user_sid: &str,
) -> Result<VerifiedClientLease, SupervisorError> {
    let mut pid = 0u32;
    if unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) } == 0 || pid == 0 {
        return Err(denied());
    }
    // The restricted service must NOT try to OpenProcess on a foreign
    // interactive user's process using its own LocalService token. The
    // original recovery service already impersonates the *kernel-attested*
    // named-pipe client before this exact operation. Repeat that narrow
    // query here, reverting BEFORE original client/receipt authentication.
    // Do not request more token rights or grant the service ambient access.
    let created = {
        if unsafe { ImpersonateNamedPipeClient(pipe) } == 0 {
            return Err(denied());
        }
        let _revert = Impersonation;
        let process = Handle::new(unsafe {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid)
        })?;
        process_creation(process.0)?
    };
    authenticate_client(
        pipe, pid, created, approved_host_path,
        approved_host_sha256, approved_host_user_sid,
    )
}

pub fn authenticate_client(
    pipe: HANDLE,
    asserted_pid: u32,
    asserted_creation: u64,
    approved_host_path: &Path,
    approved_host_sha256: &str,
    approved_host_user_sid: &str,
) -> Result<VerifiedClientLease, SupervisorError> {
    validate_approved_host_user_sid(approved_host_user_sid)?;
    let mut pid = 0_u32;
    if unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) } == 0
        || pid == 0
        || pid != asserted_pid
    {
        return Err(denied());
    }
    // The restricted LocalService principal must not receive a blanket
    // process-open grant against arbitrary interactive applications. A
    // request has already been read from THIS pipe before reaching here.
    // Impersonate the kernel-attested connecting client with its negotiated
    // SECURITY_IMPERSONATION SQOS, and open only that same client's PID.
    // The resulting query/wait handle remains valid after RAII RevertToSelf.
    // All subsequent protected Job and evidence effects execute again as
    // restricted LocalService, never under the borrowed client identity.
    if unsafe { ImpersonateNamedPipeClient(pipe) } == 0 {
        return Err(denied());
    }
    let _impersonation = Impersonation;
    let process = Handle::new(unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid)
    })?;
    let created = process_creation(process.0)?;
    if created != asserted_creation {
        return Err(denied());
    }
    // Kernel image path cannot be asserted through the JSON protocol.
    let mut image = vec![0_u16; 32768];
    let mut size = image.len() as u32;
    if unsafe { QueryFullProcessImageNameW(process.0, 0, image.as_mut_ptr(), &mut size) } == 0
        || size == 0
    {
        return Err(denied());
    }
    let client_image = String::from_utf16(&image[..size as usize]).map_err(|_| denied())?;
    let installed = fs::canonicalize(approved_host_path).map_err(|_| denied())?;
    let client = fs::canonicalize(&client_image).map_err(|_| denied())?;
    if !client
        .to_string_lossy()
        .eq_ignore_ascii_case(&installed.to_string_lossy())
    {
        return Err(denied());
    }
    hash_executable(&installed, approved_host_sha256)?;
    // Read the impersonated caller's token, not a caller-supplied account.
    // Identity and on-disk executable hash are checked before any authority.
    let mut client_token = ptr::null_mut();
    if unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut client_token) } == 0 {
        return Err(denied());
    }
    let client_token = Handle::new(client_token)?;
    let mut owner_token = ptr::null_mut();
    if unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut owner_token) } == 0 {
        return Err(denied());
    }
    let owner_token = Handle::new(owner_token)?;
    let approved = parse_sid(approved_host_user_sid)?;
    if !token_user_equal(client_token.0, owner_token.0)?
        || !token_user_is(client_token.0, approved.0)?
    {
        return Err(denied());
    }
    let mut in_container = 0_u32;
    let mut size = 0_u32;
    if unsafe {
        GetTokenInformation(
            client_token.0,
            TokenIsAppContainer,
            (&mut in_container as *mut u32).cast(),
            size_of::<u32>() as u32,
            &mut size,
        )
    } == 0
        || in_container != 0
    {
        return Err(denied());
    }
    Ok(VerifiedClientLease {
        handle: process,
        pid,
        creation_time: created,
    })
}

/// Independently check installer expectations at runtime. Root and config
/// must have protected DACLs granting only SYSTEM and this service SID.
/// No broad user, administrator, or inherited ACE can be silently adopted.

/// Debug/CI-only identity correlation for the approved installer account.
pub(crate) fn ci_diagnostic_user_sid_matches(value: &str) -> bool {
    validate_approved_host_user_sid(value).is_ok()
        && current_token_user_sid()
            .map(|actual| actual == value)
            .unwrap_or(false)
}
fn current_token_user_sid() -> Result<String, SupervisorError> {
    let mut raw = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
        return Err(denied());
    }
    let token = Handle::new(raw)?;
    let mut len = 0u32;
    unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut len);
    }
    if len < size_of::<TOKEN_USER>() as u32 || len > 8192 {
        return Err(denied());
    }
    let mut storage = vec![0usize; (len as usize).div_ceil(size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            storage.as_mut_ptr().cast(),
            len,
            &mut len,
        )
    } == 0
    {
        return Err(denied());
    }
    let sid = unsafe { (*(storage.as_ptr() as *const TOKEN_USER)).User.Sid };
    if sid.is_null() {
        return Err(denied());
    }
    let mut text = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 || text.is_null() {
        return Err(denied());
    }
    let owned = OwnedSid(text.cast());
    let length = (0..256)
        .find(|i| unsafe { *text.add(*i) } == 0)
        .ok_or_else(denied)?;
    String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) }).map_err(|_| denied())
}
/// Job ACL explicitly allows the creating native host, SYSTEM and restricted
/// service SID (query + terminate only). It must never use a default user ACL.
pub struct JobSecurity {
    descriptor: OwnedSid,
    pub attributes: SECURITY_ATTRIBUTES,
}
impl JobSecurity {
    /// FIN-04W2: a NEW named Job is owned by the distinct restricted
    /// debugger service, never by the desktop host user. Only SYSTEM and
    /// this original kernel-resolved service SID get any Job rights.
    /// The actual kernel descriptor must still be independently read back
    /// before an OS worker is accepted.
    pub(crate) fn new_debugger_service_owned() -> Result<Self, SupervisorError> {
        let sid = sid_of_named_service("LuIdeDebuggerBroker")?;
        let sddl = format!("D:P(A;;GA;;;SY)(A;;GA;;;{})", sid);
        let mut descriptor = ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                ws(&sddl).as_ptr(), 1, &mut descriptor, ptr::null_mut(),
            )
        } == 0 || descriptor.is_null() {
            return Err(denied());
        }
        let owned = OwnedSid(descriptor);
        Ok(Self {
            attributes: SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: owned.0,
                bInheritHandle: 0,
            },
            descriptor: owned,
        })
    }
    pub fn new() -> Result<Self, SupervisorError> {
        let sddl = format!(
            "D:P(A;;GA;;;SY)(A;;GA;;;{})(A;;0x0010000C;;;{})",
            current_token_user_sid()?,
            sid_of_service()?,
        );
        let mut descriptor = ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                ws(&sddl).as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } == 0
            || descriptor.is_null()
        {
            return Err(denied());
        }
        let owned = OwnedSid(descriptor);
        Ok(Self {
            attributes: SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: owned.0,
                bInheritHandle: 0,
            },
            descriptor: owned,
        })
    }
}

/// Inspect the actual kernel Job object's DACL, not merely the SDDL
/// that the independent service asked CreateJobObjectW to apply. Only
/// SYSTEM and the separate debugger service SID are allowed; no desktop
/// host, administrators, Users or Everyone ACE is permitted.
pub(crate) fn verify_debugger_job_kernel_acl(job: HANDLE) -> Result<(), SupervisorError> {
    let system = parse_sid("S-1-5-18")?;
    let service = parse_sid(&sid_of_named_service("LuIdeDebuggerBroker")?)?;
    let mut dacl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    if unsafe {
        GetSecurityInfo(
            job, SE_KERNEL_OBJECT, DACL_SECURITY_INFORMATION,
            ptr::null_mut(), ptr::null_mut(), &mut dacl,
            ptr::null_mut(), &mut descriptor,
        )
    } != 0 || dacl.is_null() || descriptor.is_null() {
        return Err(denied());
    }
    let _owned = OwnedSid(descriptor);
    let mut control = 0u16;
    let mut revision = 0u32;
    if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0
        || control & SE_DACL_PROTECTED == 0 {
        return Err(denied());
    }
    let mut info = ACL_SIZE_INFORMATION::default();
    if unsafe {
        GetAclInformation(
            dacl, (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32, AclSizeInformation,
        )
    } == 0 || info.AceCount != 2 {
        return Err(denied());
    }
    let (mut system_seen, mut service_seen) = (false, false);
    for index in 0..info.AceCount {
        let mut raw = ptr::null_mut();
        if unsafe { GetAce(dacl, index, &mut raw) } == 0 || raw.is_null() {
            return Err(denied());
        }
        let ace = unsafe { &*(raw as *const ACCESS_ALLOWED_ACE) };
        if ace.Header.AceType != 0 || ace.Header.AceFlags != 0
            || ace.Header.AceSize < size_of::<ACCESS_ALLOWED_ACE>() as u16
            || ace.Mask == 0 {
            return Err(denied());
        }
        let sid = (&ace.SidStart as *const u32).cast_mut().cast();
        if unsafe { EqualSid(sid, system.0) } != 0 {
            if system_seen { return Err(denied()); }
            system_seen = true;
        } else if unsafe { EqualSid(sid, service.0) } != 0 {
            if service_seen { return Err(denied()); }
            service_seen = true;
        } else {
            return Err(denied());
        }
    }
    if !system_seen || !service_seen { return Err(denied()); }
    Ok(())
}

/// Client-side server attestation. A spoofed, same-user pipe instance cannot
/// claim to be the recovery service merely by echoing a nonce. Kernel PID,
/// fixed installed image and restricted Windows service token all must match.
// A query-only token handle is sufficient for OS-backed service SID
// admission. A foreign service token must never be duplicated merely to
// run CheckTokenMembership: token duplication is not an authentication
// prerequisite and may be denied by the service token's own ACL.
// Check the enabled normal group AND the restricting SID list. This
// preserves the two-list restricted-token access-check semantics.
fn query_token_sid_list(
    token: HANDLE,
    class: i32,
    wanted_sid: *mut c_void,
    require_enabled: bool,
) -> Result<bool, SupervisorError> {
    let mut len = 0u32;
    unsafe {
        GetTokenInformation(token, class, ptr::null_mut(), 0, &mut len);
    }
    let offset = std::mem::offset_of!(TOKEN_GROUPS, Groups);
    if len as usize > 32768 || (len as usize) < offset + size_of::<SID_AND_ATTRIBUTES>() {
        return Err(denied());
    }
    let capacity = len as usize;
    let mut aligned = vec![0usize; capacity.div_ceil(size_of::<usize>())];
    if unsafe { GetTokenInformation(token, class, aligned.as_mut_ptr().cast(), len, &mut len) } == 0
        || len as usize > capacity
        || (len as usize) < offset
    {
        return Err(denied());
    }
    let groups = unsafe { &*(aligned.as_ptr() as *const TOKEN_GROUPS) };
    let count = groups.GroupCount as usize;
    if count == 0 || count > ((len as usize - offset) / size_of::<SID_AND_ATTRIBUTES>()) {
        return Err(denied());
    }
    let items = unsafe { std::slice::from_raw_parts(groups.Groups.as_ptr(), count) };
    let mut found = false;
    for item in items {
        if item.Sid.is_null() {
            return Err(denied());
        }
        if unsafe { EqualSid(item.Sid, wanted_sid) } != 0 {
            if found {
                return Err(denied()); // unexpected duplicate service SID
            }
            if require_enabled && (item.Attributes & 0x0000_0004) == 0 {
                return Err(denied()); // SE_GROUP_ENABLED required
            }
            found = true;
        }
    }
    Ok(found)
}

// Native CI diagnostics classify the *first* failed kernel attestation
// operation. They neither authorize IPC nor expose process paths, SIDs,
// certificates, hashes or token fields.
fn ci_service_attestation_stage(stage: &'static str) {
    if cfg!(debug_assertions)
        && std::env::var_os("CI").as_deref() == Some(OsStr::new("true"))
        && std::env::var_os("GITHUB_ACTIONS").as_deref() == Some(OsStr::new("true"))
        && std::env::var_os("LU_IDE_NATIVE_ACCEPTANCE_TRACE").as_deref() == Some(OsStr::new("1"))
    {
        eprintln!("PROTECTED_SERVICE_ADMISSION_STAGE={stage}");
    }
}
pub fn authenticate_service_server(
    pipe: HANDLE,
    approved_service_path: &Path,
    service_sha256: &str,
) -> Result<(), SupervisorError> {
    authenticate_fixed_service_server(
        pipe,approved_service_path,service_sha256,"LuIdeRecoveryBroker",
    )
}
/// W2-C native host verifies the *new* debugger service's actual SCM PID,
/// exact image hash and restricting SID before consuming its original
/// one-use grant. A spoofed same-user named pipe can never mint approval.
#[cfg(feature="fin04w2-approved-fixed-exec")]
pub(crate) fn authenticate_debugger_service_server(
    pipe: HANDLE,
    approved_service_path: &Path,
    service_sha256: &str,
) -> Result<(), SupervisorError> {
    authenticate_fixed_service_server(
        pipe,approved_service_path,service_sha256,"LuIdeDebuggerBroker",
    )
}
fn authenticate_fixed_service_server(
    pipe: HANDLE,
    approved_service_path: &Path,
    service_sha256: &str,
    service_name: &str,
) -> Result<(), SupervisorError> {
    let mut pid = 0u32;
    if unsafe { GetNamedPipeServerProcessId(pipe, &mut pid) } == 0 || pid == 0 {
        ci_service_attestation_stage("service_kernel_pid_denied");
        return Err(denied());
    }
    let server = Handle::new(unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid)
    })
    .map_err(|_| {
        ci_service_attestation_stage("service_process_open_denied");
        denied()
    })?;
    let mut text = vec![0u16; 32768];
    let mut length = text.len() as u32;
    if unsafe { QueryFullProcessImageNameW(server.0, 0, text.as_mut_ptr(), &mut length) } == 0
        || length == 0
    {
        ci_service_attestation_stage("service_kernel_image_query_denied");
        return Err(denied());
    }
    let observed = Path::new(&String::from_utf16(&text[..length as usize]).map_err(|_| {
        ci_service_attestation_stage("service_kernel_image_encoding_denied");
        denied()
    })?)
    .to_path_buf();
    let expected = fs::canonicalize(approved_service_path).map_err(|_| {
        ci_service_attestation_stage("service_expected_image_path_denied");
        denied()
    })?;
    let observed = fs::canonicalize(observed).map_err(|_| {
        ci_service_attestation_stage("service_observed_image_path_denied");
        denied()
    })?;
    if !observed
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected.to_string_lossy())
    {
        ci_service_attestation_stage("service_kernel_image_mismatch");
        return Err(denied());
    }
    hash_executable(&expected, service_sha256).map_err(|_| {
        ci_service_attestation_stage("service_image_hash_denied");
        denied()
    })?;
    let mut raw = ptr::null_mut();
    if unsafe { OpenProcessToken(server.0, TOKEN_QUERY, &mut raw) } == 0 {
        ci_service_attestation_stage("service_process_token_query_denied");
        return Err(denied());
    }
    let token = Handle::new(raw)?;
    let service = parse_sid(&sid_of_named_service(service_name)?).map_err(|_| {
        ci_service_attestation_stage("service_sid_lookup_denied");
        denied()
    })?;
    let local = parse_sid("S-1-5-19")?;
    if !query_token_sid_list(token.0, TokenGroups, service.0, true)? {
        ci_service_attestation_stage("service_enabled_group_denied");
        return Err(denied());
    }
    if !query_token_sid_list(token.0, TokenRestrictedSids, service.0, false)? {
        ci_service_attestation_stage("service_restricting_sid_denied");
        return Err(denied());
    }
    if !token_user_is(token.0, local.0)? {
        ci_service_attestation_stage("service_localservice_user_denied");
        return Err(denied());
    }
    if unsafe { IsTokenRestricted(token.0) } == 0 {
        ci_service_attestation_stage("service_not_restricted");
        return Err(denied());
    }
    ci_service_attestation_stage("service_identity_authenticated");
    Ok(())
}

/// FIN-04W2-C independently attests the ORIGINAL host supervisor journal
/// before the service reads its approval chain. The journal path is pinned
/// in protected service configuration, never supplied by the client frame.
/// This rejects broad principals, inherited DACLs, write access for the
/// debugger service and any reparse point. Host owner/SYSTEM may write
/// original native receipts; the debugger service can only READ them.
/// The OS token and exact pinned host binary are checked separately.
#[cfg(feature = "fin04w2-approved-fixed-exec")]
pub(crate) fn verify_original_approval_journal_acl(
    path: &Path, host_user_sid: &str,
) -> Result<(), SupervisorError> {
    validate_approved_host_user_sid(host_user_sid)?;
    let meta=fs::symlink_metadata(path).map_err(|_|denied())?;
    if !path.is_absolute() || !meta.is_file()
        || meta.file_attributes() & 0x400 != 0
        || meta.len() == 0 || meta.len() > 2 * 1024 * 1024 {
        return Err(denied());
    }
    let system=parse_sid("S-1-5-18")?;
    let host=parse_sid(host_user_sid)?;
    let service=parse_sid(&sid_of_named_service("LuIdeDebuggerBroker")?)?;
    let mut dacl=ptr::null_mut();
    let mut sd=ptr::null_mut();
    if unsafe {
        GetNamedSecurityInfoW(
            wide(path.as_os_str()).as_ptr() as *mut u16,
            SE_FILE_OBJECT, DACL_SECURITY_INFORMATION,
            ptr::null_mut(),ptr::null_mut(),&mut dacl,
            ptr::null_mut(),&mut sd,
        )
    } != 0 || dacl.is_null() || sd.is_null() {
        return Err(denied());
    }
    let _descriptor=OwnedSid(sd);
    let mut control=0u16;
    let mut revision=0u32;
    if unsafe {GetSecurityDescriptorControl(sd,&mut control,&mut revision)}==0
        || control & SE_DACL_PROTECTED == 0 {
        return Err(denied());
    }
    let mut count=ACL_SIZE_INFORMATION::default();
    if unsafe {
        GetAclInformation(
            dacl,(&mut count as *mut ACL_SIZE_INFORMATION).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,AclSizeInformation
        )
    }==0 || count.AceCount!=3 { return Err(denied()); }
    let(mut seen_host,mut seen_system,mut seen_service)=(false,false,false);
    // FILE_WRITE_DATA, APPEND, WRITE_EA, WRITE_ATTRIBUTES, DELETE,
    // WRITE_DAC, WRITE_OWNER, GENERIC_WRITE and GENERIC_ALL are prohibited.
    const FORBIDDEN_SERVICE: u32 =
        0x0000_0002 | 0x0000_0004 | 0x0000_0010 | 0x0000_0100
        | 0x0001_0000 | 0x0004_0000 | 0x0008_0000
        | 0x4000_0000 | 0x1000_0000;
    const SERVICE_READ: u32=0x0012_0089;
    for i in 0..count.AceCount {
        let mut entry=ptr::null_mut();
        if unsafe {GetAce(dacl,i,&mut entry)}==0 || entry.is_null() {
            return Err(denied());
        }
        let ace=unsafe { &*(entry as *const ACCESS_ALLOWED_ACE) };
        if ace.Header.AceType!=0 || ace.Header.AceFlags!=0
            || ace.Header.AceSize < size_of::<ACCESS_ALLOWED_ACE>() as u16 {
            return Err(denied());
        }
        let sid=(&ace.SidStart as *const u32).cast_mut().cast();
        if unsafe {EqualSid(sid,service.0)}!=0 {
            if seen_service || ace.Mask != SERVICE_READ
                || ace.Mask & FORBIDDEN_SERVICE != 0 { return Err(denied()); }
            seen_service=true;
        } else if unsafe {EqualSid(sid,host.0)}!=0 {
            if seen_host || ace.Mask==0 {return Err(denied());}
            seen_host=true;
        } else if unsafe {EqualSid(sid,system.0)}!=0 {
            if seen_system || ace.Mask==0 {return Err(denied());}
            seen_system=true;
        } else {return Err(denied());}
    }
    if !seen_host || !seen_system || !seen_service {return Err(denied());}
    Ok(())
}

pub fn verify_private_evidence_root(root: &Path) -> Result<(), SupervisorError> {
    verify_private_evidence_root_for(root, &sid_of_service()?)
}
pub(crate) fn verify_private_evidence_root_for(
    root: &Path,
    service_sid: &str,
) -> Result<(), SupervisorError> {
    if !service_sid.starts_with("S-1-5-80-") {
        return Err(denied());
    }
    let system = parse_sid("S-1-5-18")?;
    let service = parse_sid(service_sid)?;
    for path in [root.to_path_buf(), root.join("service.json")] {
        let meta = fs::symlink_metadata(&path).map_err(|_| denied())?;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(denied());
        }
        let mut dacl = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        let result = unsafe {
            GetNamedSecurityInfoW(
                wide(path.as_os_str()).as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut dacl,
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        if result != 0 || descriptor.is_null() || dacl.is_null() {
            return Err(denied());
        }
        let owned = OwnedSid(descriptor);
        let mut control = 0u16;
        let mut revision = 0u32;
        if unsafe { GetSecurityDescriptorControl(owned.0, &mut control, &mut revision) } == 0
            || control & SE_DACL_PROTECTED == 0
        {
            return Err(denied());
        }
        let mut info = ACL_SIZE_INFORMATION::default();
        if unsafe {
            GetAclInformation(
                dacl,
                (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } == 0
            || info.AceCount < 2
            || info.AceCount > 8
        {
            return Err(denied());
        }
        let mut seen_system = false;
        let mut seen_service = false;
        for i in 0..info.AceCount {
            let mut entry = ptr::null_mut();
            if unsafe { GetAce(dacl, i, &mut entry) } == 0 || entry.is_null() {
                return Err(denied());
            }
            let ace = unsafe { &*(entry as *const ACCESS_ALLOWED_ACE) };
            if ace.Header.AceType != 0
                || ace.Header.AceFlags & 0x10 != 0
                || ace.Header.AceSize < size_of::<ACCESS_ALLOWED_ACE>() as u16
            {
                return Err(denied());
            }
            let sid = (&ace.SidStart as *const u32).cast_mut().cast();
            if unsafe { EqualSid(sid, system.0) } != 0 {
                seen_system = true;
            } else if unsafe { EqualSid(sid, service.0) } != 0 {
                seen_service = true;
            } else {
                return Err(denied());
            }
        }
        if !seen_system || !seen_service {
            return Err(denied());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_pipe_dacl_does_not_allow_pipe_instance_creation() {
        let sid = "S-1-5-21-11-22-33-1001";
        validate_approved_host_user_sid(sid).unwrap();
        let template = format!(
            "D:P(A;;GA;;;SY)(A;;GA;;;{})(A;;0x0012019b;;;{})S:(ML;;NW;;;ME)",
            "S-1-5-80-1-2-3-4-5", sid
        );
        // Medium/no-write-up is an explicit MIC policy, not another
        // discretionary ACE that would authorize extra user accounts.
        assert!(template.ends_with("S:(ML;;NW;;;ME)"));
        assert!(!template.contains("S:(ML;;NW;;;LW)"));
        assert!(!template.contains(";;;AC)"));
        assert!(template.contains("0x0012019b;;;S-1-5-21-11-22-33-1001"));
        assert!(!template.contains(";;;IU"));
        assert!(!template.contains(";;;AU"));
        assert!(validate_approved_host_user_sid("S-1-5-18").is_err());
        assert!(validate_approved_host_user_sid("S-1-5-32-544").is_err());
        // Exact host ACE still cannot create another pipe server instance.
        let host_mask = 0x0012_019b_u32;
        assert_eq!(host_mask & 0x0000_0004, 0);
        assert_eq!(host_mask & 0x0004_0000, 0);
        assert_eq!(host_mask & 0x0008_0000, 0);
        assert!(!template.contains(";;;WD"));
        assert!(!template.contains(";;;AN"));
    }
    #[test]
    fn primary_process_token_is_duplicated_before_membership_check() {
        // The documented CheckTokenMembership contract requires an
        // impersonation token, even when verifying a process primary token.
        // This probes only the known built-in Everyone SID, not service
        // installation, privileged rights, or a fictional recovery receipt.
        let mut raw = ptr::null_mut();
        assert_ne!(
            unsafe {
                OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE, &mut raw)
            },
            0
        );
        let primary = Handle::new(raw).unwrap();
        let everyone = parse_sid("S-1-1-0").unwrap();
        assert!(enabled_member_of_primary_token(primary.0, everyone.0).unwrap());
        let random_service = parse_sid("S-1-5-80-1-2-3-4-5").unwrap();
        assert!(!enabled_member_of_primary_token(primary.0, random_service.0).unwrap());
    }
    #[test]
    fn invalid_client_image_pins_are_always_denied() {
        assert!(hash_executable(Path::new("not-absolute"), &"a".repeat(64)).is_err());
        assert!(hash_executable(Path::new("C:\\Windows\\no-such-host.exe"), "bad").is_err());
    }
}
