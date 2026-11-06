//! Disposable-native-CI-only NUL compatibility for an exact AppContainer SID.
//! Rust's nested Cargo/rustc Stdio::Null opens \\.\NUL. Never grant broad
//! AC/all-package access, disable AppContainer, or alter production device ACLs.
//! This temporary per-job kernel ACE is still a host-global mutation. A host
//! crash may leave it until reboot, so it is FORBIDDEN in release builds.
use crate::SupervisorError;
use std::{ffi::c_void, mem::size_of, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HANDLE, INVALID_HANDLE_VALUE},
    Security::{
        AclSizeInformation,
        Authorization::{
            GetSecurityInfo, SetEntriesInAclW, SetSecurityInfo, EXPLICIT_ACCESS_W, GRANT_ACCESS,
            REVOKE_ACCESS, SE_KERNEL_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
        },
        EqualSid, GetAce, GetAclInformation, IsValidSid, ACCESS_ALLOWED_ACE, ACL,
        ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION, PSID,
    },
    Storage::FileSystem::{CreateFileW, OPEN_EXISTING},
};
const READ_CONTROL: u32 = 0x0002_0000;
const WRITE_DAC: u32 = 0x0004_0000;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
fn contains_sid(acl: *mut ACL, sid: PSID) -> Result<bool, SupervisorError> {
    if acl.is_null() || unsafe { IsValidSid(sid) } == 0 {
        return Err(denied());
    }
    let mut info = ACL_SIZE_INFORMATION::default();
    if unsafe {
        GetAclInformation(
            acl,
            (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
    {
        return Err(denied());
    }
    for i in 0..info.AceCount {
        let mut ptr_ace = ptr::null_mut();
        if unsafe { GetAce(acl, i, &mut ptr_ace) } == 0 || ptr_ace.is_null() {
            return Err(denied());
        }
        let ace = unsafe { &*(ptr_ace as *const ACCESS_ALLOWED_ACE) };
        // Both ACCESS_ALLOWED and ACCESS_DENIED ACEs carry the SID at the
        // same offset. An existing deny is also a collision: this fixture
        // must never adopt, overwrite or revoke another policy's ACE.
        if (ace.Header.AceType == 0 || ace.Header.AceType == 1)
            && ace.Header.AceSize >= size_of::<ACCESS_ALLOWED_ACE>() as u16
        {
            let ace_sid = (&ace.SidStart as *const u32).cast_mut().cast::<c_void>();
            if unsafe { EqualSid(ace_sid, sid) } != 0 {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn with_dacl<T>(
    handle: HANDLE,
    callback: impl FnOnce(*mut ACL) -> Result<T, SupervisorError>,
) -> Result<T, SupervisorError> {
    let mut acl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    if unsafe {
        GetSecurityInfo(
            handle,
            SE_KERNEL_OBJECT as i32,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    } != 0
    {
        return Err(denied());
    }
    let _allocated = Local(descriptor);
    if acl.is_null() || descriptor.is_null() {
        return Err(denied());
    }
    callback(acl)
}
fn change(handle: HANDLE, sid: PSID, mode: i32) -> Result<(), SupervisorError> {
    with_dacl(handle, |acl| {
        let entry = EXPLICIT_ACCESS_W {
            grfAccessPermissions: GENERIC_READ | GENERIC_WRITE,
            grfAccessMode: mode,
            grfInheritance: 0,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: ptr::null_mut(),
                MultipleTrusteeOperation: 0,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_UNKNOWN,
                ptstrName: sid.cast(),
            },
        };
        let mut updated = ptr::null_mut();
        if unsafe { SetEntriesInAclW(1, &entry, acl, &mut updated) } != 0 {
            return Err(denied());
        }
        let _allocated = Local(updated.cast());
        if updated.is_null() {
            return Err(denied());
        }
        if unsafe {
            SetSecurityInfo(
                handle,
                SE_KERNEL_OBJECT as i32,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                updated,
                ptr::null_mut(),
            )
        } != 0
        {
            return Err(denied());
        }
        Ok(())
    })
}
/// Sole borrowed SID remains valid because the lease is declared AFTER
/// AppProfile and goes out of scope BEFORE it. One job owns one exact SID.
pub(crate) struct CiNullLease {
    handle: Handle,
    sid: PSID,
    applied: bool,
}
impl CiNullLease {
    fn revoke(&mut self) -> Result<(), SupervisorError> {
        if self.applied {
            change(self.handle.0, self.sid, REVOKE_ACCESS)?;
            with_dacl(self.handle.0, |acl| {
                if contains_sid(acl, self.sid)? {
                    Err(denied())
                } else {
                    Ok(())
                }
            })?;
            self.applied = false;
        }
        Ok(())
    }
    pub(crate) fn finish(&mut self) -> Result<(), SupervisorError> {
        self.revoke()
    }
}
impl Drop for CiNullLease {
    fn drop(&mut self) {
        if self.applied && self.revoke().is_err() {
            eprintln!("DENIED: scoped CI NUL kernel ACE cleanup not verified");
        }
    }
}
pub(crate) fn ci_scoped_nul_access(sid: PSID) -> Result<Option<CiNullLease>, SupervisorError> {
    let enabled = cfg!(debug_assertions)
        && std::env::var_os("LU_IDE_D7_CI_ALLOW_EXACT_NUL").as_deref()
            == Some(std::ffi::OsStr::new("1"))
        && std::env::var_os("GITHUB_ACTIONS").as_deref() == Some(std::ffi::OsStr::new("true"))
        && std::env::var_os("CI").as_deref() == Some(std::ffi::OsStr::new("true"));
    if !enabled {
        return Ok(None);
    }
    if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
        return Err(denied());
    }
    let raw = unsafe {
        CreateFileW(
            wide(r"\\.\NUL").as_ptr(),
            READ_CONTROL | WRITE_DAC,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        return Err(denied());
    }
    let handle = Handle(raw);
    // A new profile's SID must not already be present on the device ACL.
    with_dacl(handle.0, |acl| {
        if contains_sid(acl, sid)? {
            Err(denied())
        } else {
            Ok(())
        }
    })?;
    let mut lease = CiNullLease {
        handle,
        sid,
        applied: true,
    };
    // Mark applied before the write, so even ambiguous failures try cleanup.
    change(lease.handle.0, lease.sid, GRANT_ACCESS)?;
    with_dacl(lease.handle.0, |acl| {
        if contains_sid(acl, sid)? {
            Ok(())
        } else {
            Err(denied())
        }
    })?;
    eprintln!("NATIVE_CI_NUL_STAGE=exact_profile_access_verified");
    Ok(Some(lease))
}
