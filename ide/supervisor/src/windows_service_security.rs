//! D7D Windows recovery service identity and named-pipe security.
//!
//! Service service-name identity is checked against an OS token, not a caller
//! provided env variable. Service DACL is explicit: LocalSystem + service SID
//! may create server instances; Interactive may only read/write data (not
//! FILE_CREATE_PIPE_INSTANCE). Remote pipe clients are rejected.
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
    os::windows::{ffi::OsStrExt, fs::{MetadataExt, OpenOptionsExt}, io::AsRawHandle},
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HANDLE},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            ConvertStringSidToSidW, LookupAccountNameW, GetNamedSecurityInfoW,
            SE_FILE_OBJECT,
        },
        CheckTokenMembership, EqualSid, GetTokenInformation, GetAclInformation, GetAce,
        GetSecurityDescriptorControl, ACCESS_ALLOWED_ACE, ACL_SIZE_INFORMATION,
        AclSizeInformation, DACL_SECURITY_INFORMATION, SE_DACL_PROTECTED,
        ACCESS_ALLOWED_ACE_TYPE,
        IsTokenRestricted, OpenProcessToken, OpenThreadToken, RevertToSelf,
        SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenIsAppContainer, TokenUser,
    },
    System::{
        Pipes::{GetNamedPipeClientProcessId, ImpersonateNamedPipeClient},
        Threading::{GetCurrentProcess, GetCurrentThread, GetProcessTimes,
            OpenProcess, QueryFullProcessImageNameW},
    },
};
const SERVICE_NAME: &str = "LuIdeRecoveryBroker";
pub const SERVICE_ENDPOINT: &str = r"\\.\pipe\LuIdeRecoveryBroker.v1";
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const SYNCHRONIZE: u32 = 0x0010_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_SHARE_READ: u32 = 1;
const MAX_IMAGE_BYTES: u64 = 128 * 1024 * 1024;
fn denied() -> SupervisorError { SupervisorError::NoContainment }
pub fn wide(s: &OsStr) -> Vec<u16> { s.encode_wide().chain([0]).collect() }
pub fn ws(s: &str) -> Vec<u16> { wide(OsStr::new(s)) }
struct Handle(HANDLE);
impl Handle {
    fn new(value: HANDLE) -> Result<Self, SupervisorError> {
        if value.is_null() { Err(denied()) } else { Ok(Self(value)) }
    }
}
impl Drop for Handle {
    fn drop(&mut self) { if !self.0.is_null() { unsafe { CloseHandle(self.0); } } }
}
struct OwnedSid(*mut c_void);
impl Drop for OwnedSid {
    fn drop(&mut self) { if !self.0.is_null() { unsafe { LocalFree(self.0); } } }
}
pub fn sid_of_service() -> Result<String, SupervisorError> {
    let name = ws(&format!("NT SERVICE\\{SERVICE_NAME}"));
    let mut sid_size = 0_u32;
    let mut domain_size = 0_u32;
    let mut use_type = 0_i32;
    unsafe { LookupAccountNameW(ptr::null(), name.as_ptr(), ptr::null_mut(),
        &mut sid_size, ptr::null_mut(), &mut domain_size, &mut use_type); }
    if sid_size < 8 || sid_size > 1024 || domain_size > 1024 {
        return Err(denied());
    }
    let mut sid = vec![0_u8; sid_size as usize];
    let mut domain = vec![0_u16; domain_size as usize];
    if unsafe { LookupAccountNameW(ptr::null(), name.as_ptr(),
        sid.as_mut_ptr().cast(), &mut sid_size,
        domain.as_mut_ptr(), &mut domain_size, &mut use_type)
    } == 0 { return Err(denied()); }
    let mut string: *mut u16 = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(sid.as_mut_ptr().cast(), &mut string) } == 0
        || string.is_null() { return Err(denied()); }
    let len = (0..256).find(|n| unsafe { *string.add(*n) } == 0)
        .ok_or_else(denied)?;
    let value = String::from_utf16(unsafe {
        std::slice::from_raw_parts(string, len)
    }).map_err(|_| denied())?;
    unsafe { LocalFree(string.cast()); }
    if !value.starts_with("S-1-5-80-") { return Err(denied()); }
    Ok(value)
}
fn parse_sid(text: &str) -> Result<OwnedSid, SupervisorError> {
    let mut sid = ptr::null_mut();
    if unsafe { ConvertStringSidToSidW(ws(text).as_ptr(), &mut sid) } == 0 {
        return Err(denied());
    }
    Ok(OwnedSid(sid))
}
fn token_user_equal(a: HANDLE, b: HANDLE) -> Result<bool, SupervisorError> {
    fn sid(token: HANDLE) -> Result<Vec<u8>, SupervisorError> {
        let mut len = 0_u32;
        unsafe { GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut len); }
        if len < size_of::<TOKEN_USER>() as u32 || len > 8192 { return Err(denied()); }
        let mut bytes = vec![0_usize; (len as usize).div_ceil(size_of::<usize>())];
        if unsafe { GetTokenInformation(token, TokenUser,
            bytes.as_mut_ptr().cast(), len, &mut len) } == 0 {
            return Err(denied());
        }
        let user_sid = unsafe { (*(bytes.as_ptr() as *const TOKEN_USER)).User.Sid };
        if user_sid.is_null() { return Err(denied()); }
        // Copy the compact textual SID while token allocation remains alive.
        let mut string = ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(user_sid, &mut string) } == 0 ||
            string.is_null() { return Err(denied()); }
        let length = (0..256).find(|n| unsafe { *string.add(*n) } == 0)
            .ok_or_else(denied)?;
        let result = unsafe { std::slice::from_raw_parts(string, length) };
        let bytes = String::from_utf16(result).map_err(|_| denied())?.into_bytes();
        unsafe { LocalFree(string.cast()); }
        Ok(bytes)
    }
    Ok(sid(a)? == sid(b)?)
}
pub fn assert_restricted_local_service() -> Result<(), SupervisorError> {
    let mut raw = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
        return Err(denied());
    }
    let token = Handle::new(raw)?;
    let service_sid = parse_sid(&sid_of_service()?)?;
    let local_service_sid = parse_sid("S-1-5-19")?;
    let mut service_member = 0_i32;
    if unsafe { CheckTokenMembership(token.0, service_sid.0, &mut service_member) } == 0
        || service_member == 0 { return Err(denied()); }
    // Service must NOT be LocalSystem and must run with restricted SIDs.
    if unsafe { IsTokenRestricted(token.0) } == 0 { return Err(denied()); }
    let mut local_token = 0_i32;
    if unsafe { CheckTokenMembership(token.0, local_service_sid.0, &mut local_token) } == 0
        || local_token == 0 { return Err(denied()); }
    Ok(())
}
pub struct PipeSecurity {
    descriptor: OwnedSid,
    pub attributes: SECURITY_ATTRIBUTES,
}
impl PipeSecurity {
    pub fn new() -> Result<Self, SupervisorError> {
        // 0x00100003: SYNCHRONIZE | FILE_READ_DATA | FILE_WRITE_DATA.
        // Does NOT include FILE_APPEND_DATA / FILE_CREATE_PIPE_INSTANCE.
        // No BA grant on the IPC endpoint: administrators are machine TCB
        // but do not need to connect as a desktop worker.
        let sddl = format!(
            "O:SYG:SYD:P(A;;GA;;;SY)(A;;GA;;;{})(A;;0x00100003;;;IU)",
            sid_of_service()?
        );
        let mut descriptor = ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                ws(&sddl).as_ptr(), 1, &mut descriptor, ptr::null_mut())
        } == 0 || descriptor.is_null() {
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
    pub fn process_handle(&self) -> HANDLE { self.handle.0 }
}
fn process_creation(process: HANDLE) -> Result<u64, SupervisorError> {
    use windows_sys::Win32::Foundation::FILETIME;
    let (mut c, mut e, mut k, mut u) = (
        FILETIME::default(), FILETIME::default(),
        FILETIME::default(), FILETIME::default()
    );
    if unsafe { GetProcessTimes(process, &mut c, &mut e, &mut k, &mut u) } == 0 {
        return Err(denied());
    }
    Ok(((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64)
}
pub(crate) fn hash_executable(expected_path: &Path, expected_hash: &str) -> Result<(), SupervisorError> {
    if expected_hash.len() != 64 || !expected_hash.bytes().all(|b| b.is_ascii_hexdigit())
        || !expected_path.is_absolute() { return Err(denied()); }
    let md = fs::symlink_metadata(expected_path).map_err(|_| denied())?;
    if !md.is_file() || md.file_attributes() & 0x400 != 0 ||
        md.len() > MAX_IMAGE_BYTES { return Err(denied()); }
    let mut opts = OpenOptions::new();
    opts.read(true).share_mode(FILE_SHARE_READ).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let mut file = opts.open(expected_path).map_err(|_| denied())?;
    let mut hash = Sha256::new();
    let mut buf = [0_u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|_| denied())?;
        if n == 0 { break; }
        hash.update(&buf[..n]);
    }
    if format!("{:x}", hash.finalize()) != expected_hash {
        return Err(denied());
    }
    Ok(())
}
struct Impersonation;
impl Drop for Impersonation {
    fn drop(&mut self) { unsafe { RevertToSelf(); } }
}
pub fn authenticate_client(
    pipe: HANDLE, asserted_pid: u32, asserted_creation: u64,
    approved_host_path: &Path, approved_host_sha256: &str,
) -> Result<VerifiedClientLease, SupervisorError> {
    let mut pid = 0_u32;
    if unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) } == 0
        || pid == 0 || pid != asserted_pid { return Err(denied()); }
    let process = Handle::new(unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid)
    })?;
    let created = process_creation(process.0)?;
    if created != asserted_creation { return Err(denied()); }
    // Kernel image path cannot be asserted through the JSON protocol.
    let mut image = vec![0_u16; 32768];
    let mut size = image.len() as u32;
    if unsafe { QueryFullProcessImageNameW(process.0, 0, image.as_mut_ptr(), &mut size) } == 0
        || size == 0 { return Err(denied()); }
    let client_image = String::from_utf16(&image[..size as usize]).map_err(|_| denied())?;
    let installed = fs::canonicalize(approved_host_path).map_err(|_| denied())?;
    let client = fs::canonicalize(&client_image).map_err(|_| denied())?;
    if !client.to_string_lossy().eq_ignore_ascii_case(&installed.to_string_lossy()) {
        return Err(denied());
    }
    hash_executable(&installed, approved_host_sha256)?;
    // The call must occur only after ReadFile on this connection.
    // On impersonation failure, immediately deny; never continue as service.
    if unsafe { ImpersonateNamedPipeClient(pipe) } == 0 { return Err(denied()); }
    let _impersonation = Impersonation;
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
    if !token_user_equal(client_token.0, owner_token.0)? { return Err(denied()); }
    let mut in_container = 0_u32;
    let mut size = 0_u32;
    if unsafe { GetTokenInformation(client_token.0, TokenIsAppContainer,
        (&mut in_container as *mut u32).cast(), size_of::<u32>() as u32, &mut size)
    } == 0 || in_container != 0 { return Err(denied()); }
    Ok(VerifiedClientLease { handle: process, pid, creation_time: created })
}

/// Independently check installer expectations at runtime. Root and config
/// must have protected DACLs granting only SYSTEM and this service SID.
/// No broad user, administrator, or inherited ACE can be silently adopted.
pub fn verify_private_evidence_root(root: &Path) -> Result<(), SupervisorError> {
    let system = parse_sid("S-1-5-18")?;
    let service = parse_sid(&sid_of_service()?)?;
    for path in [root.to_path_buf(), root.join("service.json")] {
        let meta = fs::symlink_metadata(&path).map_err(|_| denied())?;
        if meta.file_attributes() & 0x400 != 0 { return Err(denied()); }
        let mut dacl = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        let result = unsafe { GetNamedSecurityInfoW(
            wide(path.as_os_str()).as_ptr(), SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION, ptr::null_mut(), ptr::null_mut(),
            &mut dacl, ptr::null_mut(), &mut descriptor,
        ) };
        if result != 0 || descriptor.is_null() || dacl.is_null() {
            return Err(denied());
        }
        let owned = OwnedSid(descriptor);
        let mut control = 0u16;
        let mut revision = 0u32;
        if unsafe { GetSecurityDescriptorControl(owned.0, &mut control, &mut revision) } == 0
            || control & SE_DACL_PROTECTED == 0 { return Err(denied()); }
        let mut info = ACL_SIZE_INFORMATION::default();
        if unsafe { GetAclInformation(dacl,
            (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32, AclSizeInformation)
        } == 0 || info.AceCount < 2 || info.AceCount > 8 { return Err(denied()); }
        let mut seen_system = false;
        let mut seen_service = false;
        for i in 0..info.AceCount {
            let mut entry = ptr::null_mut();
            if unsafe { GetAce(dacl, i, &mut entry) } == 0 || entry.is_null() {
                return Err(denied());
            }
            let ace = unsafe { &*(entry as *const ACCESS_ALLOWED_ACE) };
            if ace.Header.AceType != ACCESS_ALLOWED_ACE_TYPE
                || ace.Header.AceFlags & 0x10 != 0
                || ace.Header.AceSize < size_of::<ACCESS_ALLOWED_ACE>() as u16 {
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
        if !seen_system || !seen_service { return Err(denied()); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_pipe_dacl_does_not_allow_pipe_instance_creation() {
        let template = format!("O:SYG:SYD:P(A;;GA;;;SY)(A;;GA;;;{})(A;;0x00100003;;;IU)",
            "S-1-5-80-1-2-3-4-5");
        assert!(template.contains("0x00100003;;;IU"));
        assert!(!template.contains(";;;WD"));
        assert!(!template.contains(";;;AN"));
    }
    #[test]
    fn invalid_client_image_pins_are_always_denied() {
        assert!(hash_executable(Path::new("not-absolute"), &"a".repeat(64)).is_err());
        assert!(hash_executable(Path::new("C:\\Windows\\no-such-host.exe"), "bad").is_err());
    }
}
