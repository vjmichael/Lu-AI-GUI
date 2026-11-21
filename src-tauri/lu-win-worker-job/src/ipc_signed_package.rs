//! LU-IPC-01D Windows installed-package verification boundary.
//! WinVerifyTrust verifies an *embedded* Authenticode signature against the
//! OS trust policy, using the SAME held read-only image handle whose bytes Lu
//! Core hashes. A trusted host must independently pin BOTH that exact file
//! SHA-256 and the leaf signer certificate's DER SHA-256. Neither the process
//! pathname nor the claimed Protobuf package digest is a trust anchor.
//! No network retrieval, prompt, shell, model call, or dynamic trust fallback.
#![cfg(windows)]

use crate::ipc_pipe_peer::{observe_connected_peer, observe_message_token_peer, WindowsPipePeer};
use std::{
    ffi::c_void,
    fs::{File, OpenOptions},
    io, mem,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
    ptr,
};
use tokio::net::windows::named_pipe::NamedPipeServer;

type Handle = *mut c_void;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const SYNCHRONIZE: u32 = 0x0010_0000;
const WAIT_TIMEOUT: u32 = 0x0000_0102;
const FILE_SHARE_READ: u32 = 0x0000_0001;
const MAX_CERT_DER_BYTES: usize = 128 * 1024;
const WTD_UI_NONE: u32 = 2;
const WTD_REVOKE_NONE: u32 = 0;
const WTD_CHOICE_FILE: u32 = 1;
const WTD_STATEACTION_VERIFY: u32 = 1;
const WTD_STATEACTION_CLOSE: u32 = 2;
const WTD_REVOCATION_CHECK_NONE: u32 = 0x10;
const WTD_CACHE_ONLY_URL_RETRIEVAL: u32 = 0x1000;
const WTD_DISABLE_MD2_MD4: u32 = 0x2000;

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}
const GENERIC_VERIFY_V2: Guid = Guid {
    data1: 0x00aa_c56b, data2: 0xcd44, data3: 0x11d0,
    data4: [0x8c, 0xc2, 0x00, 0xc0, 0x4f, 0xc2, 0x95, 0xee],
};

#[repr(C)]
struct WinTrustFileInfo {
    cb_struct: u32,
    file_path: *const u16,
    file: Handle,
    known_subject: *const Guid,
}
#[repr(C)]
struct WinTrustData {
    cb_struct: u32,
    policy_callback: *mut c_void,
    sip_callback: *mut c_void,
    ui_choice: u32,
    revocation_checks: u32,
    union_choice: u32,
    file_info: *mut WinTrustFileInfo,
    state_action: u32,
    state_data: Handle,
    url_reference: *mut u16,
    provider_flags: u32,
    ui_context: u32,
    signature_settings: *mut c_void,
}
#[repr(C)]
struct CertContext {
    encoding: u32,
    encoded: *const u8,
    encoded_size: u32,
    cert_info: *const c_void,
    cert_store: Handle,
}
#[repr(C)]
struct CryptProviderCert {
    cb_struct: u32,
    cert: *const CertContext,
    // Remaining CRYPT_PROVIDER_CERT fields are intentionally opaque. The
    // documented cbStruct comes BEFORE pCert (WinTrust ABI alignment).
}
#[repr(C)]
#[derive(Default)]
struct FileTime { low: u32, high: u32 }

#[link(name = "wintrust")]
unsafe extern "system" {
    fn WinVerifyTrust(window: Handle, action: *const Guid, data: *mut WinTrustData) -> i32;
    fn WTHelperProvDataFromStateData(state: Handle) -> *mut c_void;
    fn WTHelperGetProvSignerFromChain(
        provider: *mut c_void, signer_index: u32, countersigner: i32,
        countersigner_index: u32,
    ) -> *mut c_void;
    fn WTHelperGetProvCertFromChain(signer: *mut c_void, cert_index: u32)
        -> *const CryptProviderCert;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn GetProcessId(process: Handle) -> u32;
    fn GetProcessTimes(process: Handle, created: *mut FileTime, exited: *mut FileTime,
        kernel: *mut FileTime, user: *mut FileTime) -> i32;
    fn WaitForSingleObject(handle: Handle, timeout_ms: u32) -> u32;
    fn CloseHandle(handle: Handle) -> i32;
}

fn refused(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

struct OwnedProcess(Handle);
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if !self.0.is_null() { unsafe { CloseHandle(self.0) }; }
    }
}
// Win32 process HANDLE is a kernel reference with thread-independent CloseHandle.
// No raw HANDLE is exposed or duplicated by this module.
unsafe impl Send for OwnedProcess {}

fn creation_filetime(handle: Handle) -> io::Result<u64> {
    let (mut c, mut e, mut k, mut u) = (
        FileTime::default(), FileTime::default(),
        FileTime::default(), FileTime::default(),
    );
    if unsafe { GetProcessTimes(handle, &mut c, &mut e, &mut k, &mut u) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let created = (u64::from(c.high) << 32) | u64::from(c.low);
    if created == 0 { return Err(refused("process creation identity missing")); }
    Ok(created)
}

/// WinVerifyTrust must be called with the held image handle; it cannot use
/// only a pathname which may be substituted between verification and hashing.
/// This verifies the embedded PE signature, not a catalog-only signature.
/// Strict offline verification uses cached trust only. Revocation status is
/// NOT claimed here; the host's exact signer+file pins are still mandatory.
pub fn embedded_signer_der(file: &File, absolute_path: &str) -> io::Result<Vec<u8>> {
    if !Path::new(absolute_path).is_absolute() || absolute_path.contains('\0') {
        return Err(refused("invalid signed executable pathname"));
    }
    let wide: Vec<u16> = absolute_path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = WinTrustFileInfo {
        cb_struct: mem::size_of::<WinTrustFileInfo>() as u32,
        file_path: wide.as_ptr(),
        file: file.as_raw_handle().cast(),
        known_subject: ptr::null(),
    };
    let mut data = WinTrustData {
        cb_struct: mem::size_of::<WinTrustData>() as u32,
        policy_callback: ptr::null_mut(), sip_callback: ptr::null_mut(),
        ui_choice: WTD_UI_NONE, revocation_checks: WTD_REVOKE_NONE,
        union_choice: WTD_CHOICE_FILE, file_info: &mut info,
        state_action: WTD_STATEACTION_VERIFY, state_data: ptr::null_mut(),
        url_reference: ptr::null_mut(),
        provider_flags: WTD_CACHE_ONLY_URL_RETRIEVAL
            | WTD_REVOCATION_CHECK_NONE | WTD_DISABLE_MD2_MD4,
        ui_context: 0, signature_settings: ptr::null_mut(),
    };
    // SAFETY: file, UTF-16 pathname and repr(C) structs are held through the
    // synchronous WinTrust call; only documented provider helper accessors
    // are used while the verification state is alive.
    let status = unsafe { WinVerifyTrust(ptr::null_mut(), &GENERIC_VERIFY_V2, &mut data) };
    let result = (|| -> io::Result<Vec<u8>> {
        if status != 0 || data.state_data.is_null() {
            return Err(refused("Authenticode embedded signature not trusted"));
        }
        let provider = unsafe { WTHelperProvDataFromStateData(data.state_data) };
        if provider.is_null() { return Err(refused("missing trust provider evidence")); }
        let signer = unsafe { WTHelperGetProvSignerFromChain(provider, 0, 0, 0) };
        if signer.is_null() { return Err(refused("missing verified primary signer")); }
        let certificate = unsafe { WTHelperGetProvCertFromChain(signer, 0) };
        if certificate.is_null() { return Err(refused("missing trusted signer certificate")); }
        if unsafe { (*certificate).cb_struct } < (mem::size_of::<CryptProviderCert>() as u32) {
            return Err(refused("invalid WinTrust certificate record"));
        }
        let ctx = unsafe { (*certificate).cert };
        if ctx.is_null() { return Err(refused("missing signer certificate context")); }
        let length = unsafe { (*ctx).encoded_size } as usize;
        let raw = unsafe { (*ctx).encoded };
        if raw.is_null() || !(128..=MAX_CERT_DER_BYTES).contains(&length) {
            return Err(refused("invalid signer DER size"));
        }
        // Copy before WTD_STATEACTION_CLOSE frees provider-owned evidence.
        Ok(unsafe { std::slice::from_raw_parts(raw, length) }.to_vec())
    })();
    data.state_action = WTD_STATEACTION_CLOSE;
    // WinTrust allocates state on VERIFY; even a failed signer extraction
    // MUST perform matching CLOSE. A failed CLOSE refuses admission.
    let closed = unsafe { WinVerifyTrust(ptr::null_mut(), &GENERIC_VERIFY_V2, &mut data) };
    if closed != 0 { return Err(refused("WinTrust state cleanup failed")); }
    result
}

/// Open exactly one installed image for both code-signature verification and
/// SHA-256 hashing. Deny new write/delete opens while its file handle lives.
pub fn open_signed_image(path: &str) -> io::Result<(File, Vec<u8>)> {
    let target = Path::new(path);
    if !target.is_absolute() || target.as_os_str().is_empty()
        || std::fs::symlink_metadata(target)?.file_type().is_symlink() {
        return Err(refused("untrusted signed-image path"));
    }
    let file = OpenOptions::new().read(true)
        .share_mode(FILE_SHARE_READ)
        .open(target)?;
    let cert = embedded_signer_der(&file, path)?;
    Ok((file, cert))
}

/// Retained kernel process and opened image file: dropping this also drops
/// admission custody. A caller must retain it for the whole IPC session and
/// check_alive() before accepting further messages. No public constructor.
pub struct SignedClientLease {
    witness: WindowsPipePeer,
    process: OwnedProcess,
    image_file: File,
    signer_der: Vec<u8>,
}
impl SignedClientLease {
    pub fn witness(&self) -> &WindowsPipePeer { &self.witness }
    pub fn image_file(&self) -> &File { &self.image_file }
    pub fn signer_certificate_der(&self) -> &[u8] { &self.signer_der }
    pub fn check_alive(&self) -> io::Result<()> {
        if unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT ||
            unsafe { GetProcessId(self.process.0) } != self.witness.pid ||
            creation_filetime(self.process.0)? != self.witness.creation_filetime {
            return Err(refused("enrolled DLC process exited or identity changed"));
        }
        Ok(())
    }
}

/// This MUST run synchronously after a complete first pipe ReadFile and before
/// any other pipe read. Caller MUST have already checked Hello's frame bounds.
/// No IPC claimed ID is trusted. Process/primary/impersonated SID and generation
/// are observed twice around the held process/file opening and signature check.
pub fn inspect_signed_client(
    pipe: &NamedPipeServer, expected_absolute_path: &str,
) -> io::Result<SignedClientLease> {
    let before = observe_message_token_peer(pipe)?;
    if before.image_path != expected_absolute_path {
        return Err(refused("DLC executable path differs from installer pin"));
    }
    let raw = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, before.pid)
    };
    if raw.is_null() { return Err(io::Error::last_os_error()); }
    let process = OwnedProcess(raw);
    if unsafe { GetProcessId(process.0) } != before.pid ||
        creation_filetime(process.0)? != before.creation_filetime {
        return Err(refused("DLC process PID was reused"));
    }
    let (image_file, signer_der) = open_signed_image(expected_absolute_path)?;
    let after = observe_connected_peer(pipe)?;
    if before != after { return Err(refused("DLC client generation changed during signing")); }
    let lease = SignedClientLease { witness: after, process, image_file, signer_der };
    lease.check_alive()?;
    Ok(lease)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_and_unsigned_files_never_supply_signer_der() {
        let root = std::env::temp_dir().join(format!("lu-ipc-unsigned-{}",
            std::process::id()));
        std::fs::write(&root, b"not a portable executable").unwrap();
        assert!(open_signed_image(root.to_str().unwrap()).is_err());
        std::fs::remove_file(root).unwrap();
        assert!(open_signed_image("relative-or-claimed.exe").is_err());
    }
    #[test]
    fn os_signed_pwsh_has_embedded_cert_evidence() {
        let program_files = std::env::var_os("ProgramFiles").expect("Windows program files");
        let candidate = Path::new(&program_files).join(r"PowerShell\7\pwsh.exe");
        let name = candidate.to_str().expect("Windows PowerShell pathname");
        let (file, certificate) = open_signed_image(name)
            .expect("installed signed pwsh.exe must verify offline");
        assert!(file.metadata().unwrap().len() > 4096);
        assert!(certificate.len() >= 128);
        assert!(certificate.len() <= MAX_CERT_DER_BYTES);
    }
}
