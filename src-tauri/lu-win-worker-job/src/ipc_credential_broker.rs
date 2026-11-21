//! LU-IPC-01D-R4: native OS-held installed Lu IDE handshake key reader.
//!
//! This reader does NOT provision or export a credential. The approved
//! SYSTEM-hosted broker must separately install a version-bound 32-byte
//! nonzero key in Windows Credential Manager under the LocalSystem identity
//! and securely enroll the corresponding signed IDE through its protected
//! channel. A desktop user, arbitrary caller, Hello, model or manifest must
//! NEVER supply this material.
//!
//! CRED_TYPE_GENERIC target is exactly:
//!     LuCore/IPC/IDE/v1/<lowercase 64-byte package SHA-256>
//! Credential must be the locally-persisted SYSTEM record; no credential
//! from the current interactive user, enterprise roaming, test fixture or
//! fallback target is accepted.
#![cfg(windows)]

use std::{ffi::c_void,io,ptr};

type Handle=*mut c_void;
const CRED_TYPE_GENERIC:u32=1;
const CRED_PERSIST_LOCAL_MACHINE:u32=2;
const LOCAL_SYSTEM_SID:&str="S-1-5-18";
const TARGET_PREFIX:&str="LuCore/IPC/IDE/v1/";

#[repr(C)]
struct FileTime{low:u32,high:u32}
#[repr(C)]
struct CredentialW{
    flags:u32,
    kind:u32,
    target_name:*const u16,
    comment:*const u16,
    last_written:FileTime,
    secret_size:u32,
    secret:*const u8,
    persistence:u32,
    attribute_count:u32,
    attributes:Handle,
    target_alias:*const u16,
    user_name:*const u16,
}
#[link(name="advapi32")]
unsafe extern "system"{
    fn CredReadW(target_name:*const u16,kind:u32,flags:u32,
        credential:*mut *mut CredentialW)->i32;
    fn CredFree(credential:Handle);
}
struct OwnedCredential(*mut CredentialW);
impl Drop for OwnedCredential{
    fn drop(&mut self){
        if !self.0.is_null(){unsafe{CredFree(self.0.cast())};}
    }
}
fn denied(reason:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::PermissionDenied,reason)
}
fn target_for_package(package:[u8;32])->io::Result<String>{
    if package.iter().all(|b|*b==0){
        return Err(denied("missing approved installed DLC package digest"));
    }
    let mut target=String::with_capacity(TARGET_PREFIX.len()+64);
    target.push_str(TARGET_PREFIX);
    for b in package{
        use std::fmt::Write as _;
        write!(&mut target,"{b:02x}").map_err(|_|denied("invalid credential target"))?;
    }
    Ok(target)
}
fn validate_broker_context()->io::Result<()>{
    let process=crate::ipc_pipe_peer::observe_host_process(std::process::id())
        .map_err(|_|denied("cannot inspect credential-broker host identity"))?;
    if process.user_sid!=LOCAL_SYSTEM_SID{
        return Err(denied("installed DLC credential requires SYSTEM broker identity"));
    }
    Ok(())
}
fn copy_checked_secret(record:&CredentialW)->io::Result<[u8;32]>{
    if record.flags!=0||
       record.kind!=CRED_TYPE_GENERIC||
       record.persistence!=CRED_PERSIST_LOCAL_MACHINE||
       record.secret_size!=32||
       record.secret.is_null(){
        return Err(denied("invalid or non-local installed DLC credential"));
    }
    // Only read the opaque, OS-owned credential blob after strict length checks.
    let mut key=[0u8;32];
    unsafe{ptr::copy_nonoverlapping(record.secret,key.as_mut_ptr(),32);}
    if key.iter().all(|b|*b==0){
        return Err(denied("empty installed DLC HMAC credential"));
    }
    Ok(key)
}
/// Read a package-bound local credential from the SYSTEM account's Windows
/// Credential Manager vault. 32-byte key never appears in logs/JSON/Hello.
/// No interactive-user fallback and no mutable target name parameter.
pub fn read_installed_ide_key(package:[u8;32])->io::Result<[u8;32]>{
    let target=target_for_package(package)?;
    validate_broker_context()?;
    let wide:Vec<u16>=target.encode_utf16().chain(std::iter::once(0)).collect();
    let mut raw:*mut CredentialW=ptr::null_mut();
    if unsafe{CredReadW(wide.as_ptr(),CRED_TYPE_GENERIC,0,&mut raw)}==0||
       raw.is_null(){
        return Err(denied("approved installed IDE broker credential unavailable"));
    }
    let _owned=OwnedCredential(raw);
    let secret=unsafe{&*raw};
    copy_checked_secret(secret)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn credential_target_binds_exact_package_and_never_falls_back(){
        assert!(target_for_package([0;32]).is_err());
        let a=target_for_package([0x12;32]).unwrap();
        let b=target_for_package([0x13;32]).unwrap();
        assert_ne!(a,b);
        assert_eq!(a,format!("{}{}",TARGET_PREFIX,"12".repeat(32)));
        assert_eq!(a.len(),TARGET_PREFIX.len()+64);
    }
    #[test]
    fn credential_record_rejects_unapproved_flags_persistence_or_secret(){
        let secret=[0x42u8;32];
        let make=|flags:u32,kind:u32,persistence:u32,size:u32,ptr:*const u8|
            CredentialW{flags,kind,target_name:std::ptr::null(),
                comment:std::ptr::null(),last_written:FileTime{low:0,high:0},
                secret_size:size,secret:ptr,persistence,
                attribute_count:0,attributes:std::ptr::null_mut(),
                target_alias:std::ptr::null(),user_name:std::ptr::null()};
        assert_eq!(copy_checked_secret(&make(0,1,2,32,secret.as_ptr())).unwrap(),secret);
        assert!(copy_checked_secret(&make(1,1,2,32,secret.as_ptr())).is_err());
        assert!(copy_checked_secret(&make(0,2,2,32,secret.as_ptr())).is_err());
        assert!(copy_checked_secret(&make(0,1,3,32,secret.as_ptr())).is_err());
        assert!(copy_checked_secret(&make(0,1,2,64,secret.as_ptr())).is_err());
        assert!(copy_checked_secret(&make(0,1,2,32,ptr::null())).is_err());
        assert!(copy_checked_secret(&make(0,1,2,32,[0u8;32].as_ptr())).is_err());
    }
    #[test]
    fn unsigned_normal_host_cannot_access_system_ide_broker(){
        // The Windows CI runner is an ordinary runneradmin process, not
        // LocalSystem; release must fail closed instead of requesting any
        // credential under this user or auto-creating a fake secret.
        assert!(read_installed_ide_key([0x42;32]).is_err());
    }
}
