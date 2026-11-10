//! IDE-05.6: strict *observation-only* Windows package-stage DACL auditor.
//!
//! Does not create ACLs, enable a uv worker, consume grants, or convert a
//! structural DACL check into an AppContainer security attestation. The
//! production Windows uv_sync_offline worker still returns NoContainment
//! before creating an AppContainer profile. A positive hostile kernel child
//! read/write/rename + MIC and SCM custody gate is separately required.
use crate::SupervisorError;
use std::{
    ffi::{c_void, OsStr},
    fs,
    mem::size_of,
    os::windows::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HANDLE},
    Security::{
        AclSizeInformation,
        Authorization::{ConvertStringSidToSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT},
        EqualSid, GetAce, GetAclInformation, GetSecurityDescriptorControl, GetTokenInformation,
        TokenUser, ACCESS_ALLOWED_ACE, ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION,
        LABEL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, SE_DACL_PROTECTED, TOKEN_QUERY,
        TOKEN_USER,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

const FILE_ALL_ACCESS_MASK: u32 = 0x001F_01FF;
const FILE_READ_EXECUTE_MASK: u32 = 0x0012_00A9;
const REPARSE_ATTRIBUTE: u32 = 0x400;
const OBJECT_AND_CONTAINER_INHERIT: u8 = 0x03;
const INHERITED_ACE: u8 = 0x10;
const MAX_NODES: usize = 60_000;
const MANDATORY_LABEL_ACE_TYPE: u8 = 0x11;
const MANDATORY_NO_WRITE_UP: u32 = 0x01;

fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}
fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain([0]).collect()
}
struct OwnedLocal(*mut c_void);
impl Drop for OwnedLocal {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
}
struct OwnedToken(HANDLE);
impl Drop for OwnedToken {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
fn parse_system_sid() -> Result<OwnedLocal, SupervisorError> {
    let mut sid = ptr::null_mut();
    if unsafe { ConvertStringSidToSidW(wide(OsStr::new("S-1-5-18")).as_ptr(), &mut sid) } == 0
        || sid.is_null()
    {
        return Err(denied());
    }
    Ok(OwnedLocal(sid))
}
/// Inspect an explicit native MEDIUM no-write-up mandatory-integrity SACL.
 /// An implicit default-medium label is not accepted for sealed untrusted
 /// source; the observer never grants token privileges or changes the label.
fn require_explicit_medium_integrity_label(path: &Path) -> Result<(), SupervisorError> {
    let mut sacl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    let status = unsafe {
        GetNamedSecurityInfoW(
            wide(path.as_os_str()).as_ptr(),
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut sacl,
            &mut descriptor,
        )
    };
    if status != 0 || descriptor.is_null() || sacl.is_null() {
        return Err(denied());
    }
    let _descriptor = OwnedLocal(descriptor);
    let mut info = ACL_SIZE_INFORMATION::default();
    if unsafe {
        GetAclInformation(
            sacl,
            (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
        || info.AceCount != 1
    {
        return Err(denied());
    }
    let mut raw = ptr::null_mut();
    if unsafe { GetAce(sacl, 0, &mut raw) } == 0 || raw.is_null() {
        return Err(denied());
    }
    // Mandatory label ACE has the same header/mask/SidStart layout as an
    // access-allowed ACE. The type and mask are independently required.
    let label = unsafe { &*(raw as *const ACCESS_ALLOWED_ACE) };
    if label.Header.AceType != MANDATORY_LABEL_ACE_TYPE
        || label.Header.AceFlags & INHERITED_ACE != 0
        || label.Mask != MANDATORY_NO_WRITE_UP
        || label.Header.AceSize < size_of::<ACCESS_ALLOWED_ACE>() as u16
    {
        return Err(denied());
    }
    let mut medium = ptr::null_mut();
    if unsafe {
        ConvertStringSidToSidW(wide(OsStr::new("S-1-16-8192")).as_ptr(), &mut medium)
    } == 0
        || medium.is_null()
    {
        return Err(denied());
    }
    let _medium = OwnedLocal(medium);
    let sid = (&label.SidStart as *const u32).cast_mut().cast();
    if unsafe { EqualSid(sid, medium) } == 0 {
        return Err(denied());
    }
    Ok(())
}

fn owner_token_sid() -> Result<(OwnedToken, Vec<usize>), SupervisorError> {
    let mut raw = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 || raw.is_null()
    {
        return Err(denied());
    }
    let handle = OwnedToken(raw);
    let mut length = 0u32;
    unsafe {
        GetTokenInformation(handle.0, TokenUser, ptr::null_mut(), 0, &mut length);
    }
    if length < size_of::<TOKEN_USER>() as u32 || length > 4096 {
        return Err(denied());
    }
    let mut aligned = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            handle.0,
            TokenUser,
            aligned.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err(denied());
    }
    Ok((handle, aligned))
}

fn inspect_one(
    path: &Path,
    package_sid: *mut c_void,
    host_user_sid: *mut c_void,
    system_sid: *mut c_void,
) -> Result<(), SupervisorError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| denied())?;
    if metadata.file_attributes() & REPARSE_ATTRIBUTE != 0
        || (!metadata.is_file() && !metadata.is_dir())
    {
        return Err(denied());
    }
    let mut owner = ptr::null_mut();
    let mut dacl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    if unsafe {
        GetNamedSecurityInfoW(
            wide(path.as_os_str()).as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        )
    } != 0
        || descriptor.is_null()
        || owner.is_null()
        || dacl.is_null()
    {
        return Err(denied());
    }
    let _descriptor = OwnedLocal(descriptor);
    if unsafe { EqualSid(owner, host_user_sid) } == 0 {
        return Err(denied());
    }
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
    let mut package = false;
    let mut host = false;
    let mut system = false;
    for index in 0..info.AceCount {
        let mut raw = ptr::null_mut();
        if unsafe { GetAce(dacl, index, &mut raw) } == 0 || raw.is_null() {
            return Err(denied());
        }
        let ace = unsafe { &*(raw as *const ACCESS_ALLOWED_ACE) };
        if ace.Header.AceType != 0
            || ace.Header.AceFlags & INHERITED_ACE != 0
            || ace.Header.AceSize < size_of::<ACCESS_ALLOWED_ACE>() as u16
        {
            return Err(denied());
        }
        if metadata.is_dir()
            && ace.Header.AceFlags & OBJECT_AND_CONTAINER_INHERIT != OBJECT_AND_CONTAINER_INHERIT
        {
            return Err(denied());
        }
        let sid = (&ace.SidStart as *const u32).cast_mut().cast();
        if unsafe { EqualSid(sid, package_sid) } != 0 {
            if package || ace.Mask != FILE_READ_EXECUTE_MASK {
                return Err(denied());
            }
            package = true;
        } else if unsafe { EqualSid(sid, host_user_sid) } != 0 {
            if host || ace.Mask != FILE_ALL_ACCESS_MASK {
                return Err(denied());
            }
            host = true;
        } else if unsafe { EqualSid(sid, system_sid) } != 0 {
            if system || ace.Mask != FILE_ALL_ACCESS_MASK {
                return Err(denied());
            }
            system = true;
        } else {
            // Deny unknown principal, broad AllApplicationPackages grants,
            // inherited generic Everyone grants or capabilities.
            return Err(denied());
        }
    }
    if package && host && system {
        require_explicit_medium_integrity_label(path)?;
        Ok(())
    } else {
        Err(denied())
    }
}

/// Inspect ONLY a host-created, bounded, non-reparse staged source/tool tree.
/// The proof does not cover MIC, AppContainer token effective access, owner
/// rights or process escape; it cannot grant a Windows package operation.
pub fn inspect_staged_readonly_tree(
    root: &Path,
    package_sid: *mut c_void,
) -> Result<usize, SupervisorError> {
    if package_sid.is_null() {
        return Err(denied());
    }
    let system = parse_system_sid()?;
    let (_token, user) = owner_token_sid()?;
    let user_ptr = unsafe { (*(user.as_ptr() as *const TOKEN_USER)).User.Sid };
    if user_ptr.is_null() {
        return Err(denied());
    }
    let mut pending = vec![PathBuf::from(root)];
    let mut count = 0usize;
    while let Some(path) = pending.pop() {
        count = count.checked_add(1).ok_or_else(denied)?;
        if count > MAX_NODES {
            return Err(denied());
        }
        inspect_one(&path, package_sid, user_ptr, system.0)?;
        if fs::symlink_metadata(&path).map_err(|_| denied())?.is_dir() {
            for child in fs::read_dir(&path).map_err(|_| denied())? {
                pending.push(child.map_err(|_| denied())?.path());
                if pending.len() + count > MAX_NODES {
                    return Err(denied());
                }
            }
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsealed_host_staging_cannot_be_accepted_as_readonly_appcontainer() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ide056-acl-negative-{}-{}",
            std::process::id(),
            stamp
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("source.rs"), b"immutable source fixture").unwrap();
        let sid = parse_system_sid().unwrap();
        // The wrong package SID and the inherited user-write ACL MUST fail.
        assert_eq!(
            inspect_staged_readonly_tree(&path, sid.0).unwrap_err(),
            SupervisorError::NoContainment
        );
        fs::remove_dir_all(&path).unwrap();
    }
    #[test]
    fn exact_read_execute_mask_does_not_grant_mutation_or_dacl_control() {
        const WRITE_DELETE_OWNER_DAC: u32 = 0x0001_0116 | 0x0004_0000 | 0x0008_0000 | 0x0001_0000;
        assert_eq!(FILE_READ_EXECUTE_MASK & WRITE_DELETE_OWNER_DAC, 0);
        assert_eq!(OBJECT_AND_CONTAINER_INHERIT, 3);
    }
}
