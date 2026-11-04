//! WRITE-01E.4C.3A — fail-closed Windows security descriptor and file-time
//! custody for an already-validated original and an unpublished stage.
//! No model/UI pathname access. All Win32 FFI stays in the isolated crate.
//!
//! Owner, primary group, and DACL must be byte-for-byte identical in their
//! self-relative security descriptor on source and stage. A custom ACL that
//! cannot be inherited identically is deliberately REFUSED. We do NOT claim
//! preservation of SACLs, object IDs, extended attributes or arbitrary NTFS
//! metadata, and do NOT claim atomic cross-process file-ID compare-and-swap.
//! The file's last-write timestamp intentionally changes on edit/restore.
#![cfg(windows)]
use std::{ffi::c_void,fs::File,io,
    os::windows::io::{AsRawHandle,FromRawHandle},ptr};

const SE_FILE_OBJECT:u32=1;
const OWNER_SECURITY_INFORMATION:u32=0x01;
const GROUP_SECURITY_INFORMATION:u32=0x02;
const DACL_SECURITY_INFORMATION:u32=0x04;
const MAX_SD_BYTES:usize=65536;
const GENERIC_READ:u32=0x80000000;
const FILE_SHARE_READ:u32=0x00000001;
const FILE_SHARE_DELETE:u32=0x00000004;
#[repr(C)]
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq)]
pub struct FileTime { low:u32, high:u32 }

#[link(name="kernel32")]
extern "system" {
    fn GetFileTime(file:*mut c_void,creation:*mut FileTime,
        access:*mut FileTime,write:*mut FileTime)->i32;
    fn SetFileTime(file:*mut c_void,creation:*const FileTime,
        access:*const FileTime,write:*const FileTime)->i32;
    fn LocalFree(mem:*mut c_void)->*mut c_void;
    fn ReOpenFile(original:*mut c_void,access:u32,sharing:u32,flags:u32)->*mut c_void;
}
#[link(name="advapi32")]
extern "system" {
    fn GetSecurityInfo(handle:*mut c_void,object_type:u32,security_info:u32,
        owner:*mut *mut c_void,group:*mut *mut c_void,
        dacl:*mut *mut c_void,sacl:*mut *mut c_void,
        descriptor:*mut *mut c_void)->u32;
    fn GetSecurityDescriptorLength(descriptor:*const c_void)->u32;
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct SecurityMetadataSnapshot {
    pub creation:FileTime,
    pub last_access:FileTime,
    owner_group_dacl:Vec<u8>,
}
fn deny(reason:&str)->io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied,format!(
        "Windows metadata-safe existing-file replacement refused: {reason}"
    ))
}
fn file_times(file:&File)->io::Result<(FileTime,FileTime)>{
    let (mut creation,mut access)=(FileTime::default(),FileTime::default());
    // SAFETY: an owned, live File handle and two aligned, writable FILETIME
    // outputs. Win32 writes only the declared outputs on success.
    let ok=unsafe{GetFileTime(file.as_raw_handle().cast::<c_void>(),
        &mut creation,&mut access,ptr::null_mut())};
    if ok==0 {return Err(io::Error::last_os_error());}
    Ok((creation,access))
}
fn security_descriptor(file:&File)->io::Result<Vec<u8>>{
    let mut descriptor: *mut c_void=ptr::null_mut();
    // SAFETY: live FILE handle, the descriptor output is a pointer variable;
    // GetSecurityInfo allocates the descriptor via LocalAlloc on success.
    let rc=unsafe{GetSecurityInfo(
        file.as_raw_handle().cast::<c_void>(),SE_FILE_OBJECT,
        OWNER_SECURITY_INFORMATION|GROUP_SECURITY_INFORMATION|DACL_SECURITY_INFORMATION,
        ptr::null_mut(),ptr::null_mut(),ptr::null_mut(),ptr::null_mut(),
        &mut descriptor
    )};
    if rc!=0 {
        // API returns Win32 ERROR_* directly, not GetLastError.
        return Err(io::Error::from_raw_os_error(rc as i32));
    }
    if descriptor.is_null(){return Err(deny("native security descriptor missing"));}
    // SAFETY: returned descriptor is valid until LocalFree. Its size is
    // obtained from the OS and capped before any user-space read.
    let size=unsafe{GetSecurityDescriptorLength(descriptor)} as usize;
    let result=if (20..=MAX_SD_BYTES).contains(&size) {
        // SAFETY: GetSecurityDescriptorLength gives the allocation's
        // descriptor length, the source is valid for the duration of copy.
        Ok(unsafe{std::slice::from_raw_parts(
            descriptor.cast::<u8>(),size
        ).to_vec()})
    } else {
        Err(deny("missing, malformed or oversized security descriptor"))
    };
    // SAFETY: only LocalFree frees the pointer from GetSecurityInfo.
    let freed=unsafe{LocalFree(descriptor)};
    if !freed.is_null(){return Err(deny("native descriptor release failed"));}
    result
}
/// Non-owning path-free write-exclusion lease on the already capability-opened
/// target. Fails closed if an existing writable handle conflicts; blocks new
/// GENERIC_WRITE opens until the returned File is dropped. SHARE_DELETE stays
/// set so Lu's own capability-relative rename can publish. A hostile external
/// rename is NOT prevented by this lease; it is not a name-bound CAS.
pub fn no_write_lease(opened_target:&File)->io::Result<File>{
    // SAFETY: ReOpenFile receives a live handle to the previously validated
    // FILE object. Its returned distinct HANDLE is owned by exactly one
    // File::from_raw_handle and closed automatically on Drop.
    let result=unsafe{ReOpenFile(
        opened_target.as_raw_handle().cast::<c_void>(),
        GENERIC_READ,FILE_SHARE_READ|FILE_SHARE_DELETE,0
    )};
    if result.is_null() || result as isize == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful ReOpenFile returns a fresh owning Windows HANDLE.
    Ok(unsafe{File::from_raw_handle(result.cast())})
}

/// Snapshot owner, group, DACL, creation and access times from the OPENED
/// target. Any access-denied/unsupported filesystem response is a refusal.
pub fn snapshot_metadata(file:&File)->io::Result<SecurityMetadataSnapshot>{
    let (creation,last_access)=file_times(file)?;
    let owner_group_dacl=security_descriptor(file)?;
    Ok(SecurityMetadataSnapshot{creation,last_access,owner_group_dacl})
}
/// Verify the source is still the reviewed metadata owner. Last-access
/// changes caused by legitimate reads are not treated as source edits.
pub fn check_source_metadata(
    original:&File,snapshot:&SecurityMetadataSnapshot
)->io::Result<()>{
    let (creation,_)=file_times(original)?;
    if creation!=snapshot.creation ||
        security_descriptor(original)?!=snapshot.owner_group_dacl {
        return Err(deny("original creation/security identity changed"));
    }
    Ok(())
}
/// Before a stage can publish, ensure its security descriptor is exactly
/// equivalent to the original; then retain original creation/access times.
/// No WRITE_DAC needed: noninherited custom descriptors are refused.
pub fn prepare_stage_metadata(
    original:&File,stage:&File,snapshot:&SecurityMetadataSnapshot
)->io::Result<()>{
    check_source_metadata(original,snapshot)?;
    if security_descriptor(stage)?!=snapshot.owner_group_dacl {
        return Err(deny("stage would change owner, group or DACL"));
    }
    // SAFETY: stage must be opened with FILE_WRITE_ATTRIBUTES; valid owned
    // stage handle and two borrowed FILETIME inputs. We deliberately do
    // not copy last-write time, so the edit remains visibly modified.
    let ok=unsafe{SetFileTime(stage.as_raw_handle().cast::<c_void>(),
        &snapshot.creation,&snapshot.last_access,ptr::null())};
    if ok==0 {return Err(io::Error::last_os_error());}
    let (now_creation,now_access)=file_times(stage)?;
    if now_creation!=snapshot.creation || now_access!=snapshot.last_access {
        return Err(deny("filesystem could not round-trip creation/access time"));
    }
    stage.sync_all()?;
    if security_descriptor(stage)?!=snapshot.owner_group_dacl {
        return Err(deny("stage security changed during timestamp preparation"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs,sync::atomic::{AtomicU64,Ordering}};
    static NEXT:AtomicU64=AtomicU64::new(0);
    fn temp()->std::path::PathBuf {
        let p=std::env::temp_dir().join(format!(
            "lu-win-metadata-{}-{}",std::process::id(),
            NEXT.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir(&p).unwrap();p
    }
    #[test]
    fn stage_inherits_exact_security_and_preserves_original_creation() {
        let p=temp();let a=p.join("existing.txt"),b=p.join("stage.txt");
        fs::write(&a,b"before").unwrap();
        fs::write(&b,b"after").unwrap();
        let source=File::open(&a).unwrap();
        let snapshot=snapshot_metadata(&source).unwrap();
        let staged=fs::OpenOptions::new().read(true).write(true).open(&b).unwrap();
        prepare_stage_metadata(&source,&staged,&snapshot).unwrap();
        assert_eq!(snapshot_metadata(&staged).unwrap().creation,snapshot.creation);
        assert_eq!(snapshot_metadata(&staged).unwrap().owner_group_dacl,
            snapshot.owner_group_dacl);
        drop(staged);drop(source);fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn no_write_lease_blocks_new_external_writer_until_release() {
        let p=temp();let path=p.join("source.txt");
        fs::write(&path,b"original").unwrap();
        let opened=File::open(&path).unwrap();
        let lease=no_write_lease(&opened).unwrap();
        assert!(fs::OpenOptions::new().write(true).open(&path).is_err(),
            "new writer should not be able to race Lu's final validation");
        drop(lease);
        assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
        drop(opened);fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn no_write_lease_refuses_an_existing_external_writer() {
        let p=temp();let path=p.join("source.txt");
        fs::write(&path,b"original").unwrap();
        let writer=fs::OpenOptions::new().write(true).open(&path).unwrap();
        let opened=File::open(&path).unwrap();
        assert!(no_write_lease(&opened).is_err(),
            "an extant writer prevents Lu from claiming a no-write lease");
        drop(opened);drop(writer);
        let opened=File::open(&path).unwrap();
        assert!(no_write_lease(&opened).is_ok());
        drop(opened);fs::remove_dir_all(p).unwrap();
    }

    #[test]
    fn refuse_stage_security_mismatch_not_publish() {
        let p=temp();let a=p.join("existing.txt"),b=p.join("stage.txt");
        fs::write(&a,b"before").unwrap();fs::write(&b,b"after").unwrap();
        let source=File::open(&a).unwrap();
        let mut snap=snapshot_metadata(&source).unwrap();
        snap.owner_group_dacl.push(0);
        let staged=fs::OpenOptions::new().read(true).write(true).open(&b).unwrap();
        assert!(prepare_stage_metadata(&source,&staged,&snap).is_err());
        assert_eq!(fs::read(&a).unwrap(),b"before");
        drop(staged);drop(source);fs::remove_dir_all(p).unwrap();
    }
}
