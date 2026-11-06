//! WRITE-01E.4C: fail-closed Win32 *opened-handle* metadata checks.
//! No pathname-based metadata lookup and no model-supplied Win32 handles.
//! This DOES NOT provide atomic CAS or preserve arbitrary ACL metadata.
//! Win32 ABI: FILE_ID_INFO, FILE_STANDARD_INFO, FILE_STREAM_INFO.
#![cfg(windows)]
use std::{
    ffi::c_void, fs::File, io,
    os::windows::{fs::MetadataExt, io::{AsRawHandle,FromRawHandle}},
};
const FILE_STANDARD_INFO:u32=1;
const FILE_STREAM_INFO:u32=7;
const FILE_ID_INFO:u32=18;
const UNSUPPORTED_ATTRIBUTES:u32=
    0x0000_0001 | // READONLY
    0x0000_0002 | // HIDDEN
    0x0000_0004 | // SYSTEM
    0x0000_0200 | // SPARSE_FILE
    0x0000_0400 | // REPARSE_POINT
    0x0000_0800 | // COMPRESSED
    0x0000_1000 | // OFFLINE
    0x0000_4000 | // ENCRYPTED
    0x0000_8000 | // INTEGRITY_STREAM
    0x0004_0000 | // RECALL_ON_OPEN
    0x0040_0000;  // RECALL_ON_DATA_ACCESS

#[repr(C)]
#[derive(Clone,Copy,Default)]
struct FileIdInfo {
    volume_serial_number:u64,
    file_id:[u8;16],
}
#[repr(C)]
#[derive(Clone,Copy,Default)]
struct FileStandardInfo {
    allocation_size:i64,
    end_of_file:i64,
    number_of_links:u32,
    delete_pending:u8,
    directory:u8,
    _padding:[u8;2],
}
#[link(name="kernel32")]
extern "system" {
    fn GetFileInformationByHandleEx(
        file:*mut c_void,
        file_info_class:u32,
        file_information:*mut c_void,
        buffer_size:u32
    )->i32;
    fn ReOpenFile(
        original:*mut c_void,desired_access:u32,share_mode:u32,flags:u32
    )->*mut c_void;
    fn SetFileInformationByHandle(
        file:*mut c_void,info_class:u32,info:*const c_void,bytes:u32
    )->i32;
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct WindowsFileIdentity {
    volume_serial:u64,
    file_id:[u8;16],
}
impl WindowsFileIdentity {
    /// Stable, host-captured token: native volume serial (16 hex) and 128-bit
    /// file ID (32 hex). Not a user authorization grant or a filesystem path.
    pub fn token(self)->String {
        let mut result=format!("{:016x}",self.volume_serial);
        for b in self.file_id {result.push_str(&format!("{b:02x}"));}
        result
    }
}
fn unsafe_target(reason:&str)->io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied,format!(
        "existing-file replacement refused: {reason}"
    ))
}
fn query<T>(file:&File,cls:u32,out:&mut T)->io::Result<()> {
    // SAFETY: file handle is live for the entire OS call. The output is a
    // live, properly aligned writable repr(C) struct with exact size.
    let ok=unsafe {GetFileInformationByHandleEx(
        file.as_raw_handle().cast::<c_void>(),cls,
        (out as *mut T).cast::<c_void>(),std::mem::size_of::<T>() as u32
    )};
    if ok==0 {Err(io::Error::last_os_error())} else {Ok(())}
}
/// Only a single unnamed default data stream is accepted. Unsupported
/// stream enumeration, unrecognized layouts and oversized tables fail closed.
fn only_default_data_stream(file:&File)->io::Result<()> {
    // Win32 FILE_STREAM_INFO entries must be 8-byte aligned.
    let mut buffer=[0u64;2048]; // strict 16-KiB bound
    // SAFETY: aligned writable buffer, live file handle, correct length.
    let ok=unsafe {GetFileInformationByHandleEx(
        file.as_raw_handle().cast::<c_void>(),FILE_STREAM_INFO,
        buffer.as_mut_ptr().cast::<c_void>(),
        std::mem::size_of_val(&buffer) as u32
    )};
    if ok==0 {return Err(io::Error::last_os_error());}
    // SAFETY: u64 array is contiguous initialized storage; bytes cover
    // exactly the object representation (not beyond the buffer).
    let bytes=unsafe {std::slice::from_raw_parts(
        buffer.as_ptr().cast::<u8>(),std::mem::size_of_val(&buffer)
    )};
    let mut offset=0usize;
    let mut count=0usize;
    loop {
        // next (u32), name byte length (u32), size (i64),
        // allocation (i64), UTF-16 name at byte offset 24.
        let head=bytes.get(offset..offset+24)
            .ok_or_else(||unsafe_target("truncated native stream header"))?;
        let next=u32::from_le_bytes(head[0..4].try_into().unwrap()) as usize;
        let len=u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
        if len%2!=0 || len>2048 {
            return Err(unsafe_target("invalid or oversized stream name"));
        }
        let end=offset.checked_add(24).and_then(|p|p.checked_add(len))
            .ok_or_else(||unsafe_target("stream table overflow"))?;
        let name=bytes.get(offset+24..end)
            .ok_or_else(||unsafe_target("truncated stream table"))?;
        let decoded:Vec<u16>=name.chunks_exact(2)
            .map(|v|u16::from_le_bytes([v[0],v[1]])).collect();
        let default="::$DATA".encode_utf16().collect::<Vec<_>>();
        if !decoded.is_empty() && decoded!=default {
            return Err(unsafe_target("alternate data stream present"));
        }
        count+=1;
        if count>1 {return Err(unsafe_target("multiple streams")); }
        if next==0 {break;}
        if next<24+len || next%8!=0 {
            return Err(unsafe_target("invalid stream-table next offset"));
        }
        offset=offset.checked_add(next)
            .ok_or_else(||unsafe_target("stream-table offset overflow"))?;
        if offset>=bytes.len() {return Err(unsafe_target("stream table exceeds buffer"));}
    }
    if count!=1 {return Err(unsafe_target("missing default stream"));}
    Ok(())
}
/// Opened-file identity and strict admission: rejects multiple hardlinks,
/// reparse points, ADS, sparse/compressed/encrypted/special files and invalid
/// filesystem metadata. Never fallback to path-based checks on errors.
pub fn inspect_open_file(file:&File)->io::Result<WindowsFileIdentity> {
    let meta=file.metadata()?;
    if !meta.is_file() || meta.file_attributes() & UNSUPPORTED_ATTRIBUTES!=0 {
        return Err(unsafe_target("unsupported file type or attributes"));
    }
    let mut standard=FileStandardInfo::default();
    query(file,FILE_STANDARD_INFO,&mut standard)?;
    if standard.directory!=0 || standard.delete_pending!=0 ||
        standard.number_of_links!=1 {
        return Err(unsafe_target("directory, pending deletion, or hardlink"));
    }
    let mut native_id=FileIdInfo::default();
    query(file,FILE_ID_INFO,&mut native_id)?;
    if native_id.file_id==[0u8;16] {
        return Err(unsafe_target("no stable native volume file ID"));
    }
    only_default_data_stream(file)?;
    Ok(WindowsFileIdentity{
        volume_serial:native_id.volume_serial_number,
        file_id:native_id.file_id,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs,sync::atomic::{AtomicU64,Ordering}};
    static NEXT:AtomicU64=AtomicU64::new(0);
    fn scratch()->std::path::PathBuf {
        let p=std::env::temp_dir().join(format!("lu-win-file-guard-{}-{}",
            std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir(&p).unwrap();p
    }
    #[test]
    fn ordinary_file_identity_stable_but_hardlink_refused() {
        let root=scratch();let a=root.join("original.txt");
        fs::write(&a,b"hello").unwrap();
        let handle=File::open(&a).unwrap();
        assert_eq!(inspect_open_file(&handle).unwrap(),
            inspect_open_file(&handle).unwrap());
        drop(handle);
        fs::hard_link(&a,root.join("alias.txt")).unwrap();
        let handle=File::open(&a).unwrap();
        assert!(inspect_open_file(&handle).is_err());
        drop(handle);
        assert_eq!(fs::read(&a).unwrap(),b"hello");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn alternate_data_stream_is_never_silently_discarded() {
        let root=scratch();let a=root.join("has-stream.txt");
        fs::write(&a,b"main").unwrap();
        let ads=std::path::PathBuf::from(format!("{}:LuSecondary",a.display()));
        match fs::write(&ads,b"metadata") {
            Ok(())=>{
                let file=File::open(&a).unwrap();
                assert!(inspect_open_file(&file).is_err());
                drop(file);
                assert_eq!(fs::read(&a).unwrap(),b"main");
            },
            Err(e)=>eprintln!("ADS unsupported on runner: {e}; no ADS acceptance asserted"),
        }
        fs::remove_dir_all(root).unwrap();
    }
}


/// A directory identity witness from a live Win32 directory HANDLE, not a
/// pathname. Reparse-point or missing native file ID is rejected.
pub fn opened_directory_identity(file:&File)->io::Result<String>{
    let metadata=file.metadata()?;
    if !metadata.is_dir()||metadata.file_attributes()&0x0000_0400!=0{
        return Err(unsafe_target("unsafe directory or reparse point"))
    }
    let mut id=FileIdInfo::default();
    query(file,FILE_ID_INFO,&mut id)?;
    if id.file_id==[0;16]{
        return Err(unsafe_target("directory has no stable native file ID"))
    }
    let mut value=format!("{:016x}",id.volume_serial_number);
    for byte in id.file_id{value.push_str(&format!("{byte:02x}"))}
    Ok(value)
}


/// Stable 128-bit file identity, allowed after a hard link has increased the
/// link count. This is an observation, not an authorization or deletion API.
pub fn opened_file_identity(file:&File)->io::Result<String>{
    let metadata=file.metadata()?;
    if !metadata.is_file()||metadata.file_attributes()&0x0000_0400!=0{
        return Err(unsafe_target("file is not an ordinary opened file"))
    }
    let mut id=FileIdInfo::default();
    query(file,FILE_ID_INFO,&mut id)?;
    if id.file_id==[0;16]{
        return Err(unsafe_target("file has no stable native file ID"))
    }
    let mut value=format!("{:016x}",id.volume_serial_number);
    for byte in id.file_id{value.push_str(&format!("{byte:02x}"))}
    Ok(value)
}


/// Atomic handle-relative, no-overwrite rename of the already opened file.
/// A substituted SOURCE pathname cannot redirect this to a different file.
/// All Win32 ABI and raw handle ownership stay inside this audited boundary.
pub fn rename_opened_file_noreplace(
    source:&File,destination_dir:&File,target:&str
)->io::Result<()>{
    const DELETE:u32=0x0001_0000;
    const FILE_SHARE_ALL:u32=0x0000_0007;
    const FILE_RENAME_INFO:u32=3;
    if target.is_empty()||target.len()>255||target=="."||target==".."||
        target.ends_with(' ')||target.ends_with('.')||
        target.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':')){
        return Err(unsafe_target("invalid no-clobber move destination"))
    }
    let original_id=opened_file_identity(source)?;
    opened_directory_identity(destination_dir)?;
    // ReOpenFile retains the original FILE OBJECT, not a second pathname
    // lookup. Request DELETE access only for this exact opened file.
    let raw=unsafe{ReOpenFile(
        source.as_raw_handle().cast::<c_void>(),DELETE,
        FILE_SHARE_ALL,0,
    )};
    if raw.is_null()||raw as isize==-1{
        return Err(io::Error::last_os_error())
    }
    // SAFETY: ReOpenFile returned a new, exclusively owned valid HANDLE.
    let reopened=unsafe{File::from_raw_handle(raw.cast())};
    if opened_file_identity(&reopened)?!=original_id{
        return Err(unsafe_target("source file object changed"))
    }
    let utf16=target.encode_utf16().collect::<Vec<_>>();
    if utf16.is_empty()||utf16.len()>256{
        return Err(unsafe_target("destination name too long"))
    }
    #[repr(C)]
    struct RenameInfo{
        replace_if_exists:u32,
        reserved:u32,
        root_directory:*mut c_void,
        filename_bytes:u32,
        filename:[u16;256],
    }
    let mut info=RenameInfo{
        replace_if_exists:0,reserved:0,
        root_directory:destination_dir.as_raw_handle().cast::<c_void>(),
        filename_bytes:(utf16.len()*2) as u32,
        filename:[0;256],
    };
    info.filename[..utf16.len()].copy_from_slice(&utf16);
    // SAFETY: valid opened DELETE handle and initialized repr(C) buffer;
    // FileRenameInfo, ReplaceIfExists=false, uses the pinned root directory.
    let result=unsafe{SetFileInformationByHandle(
        reopened.as_raw_handle().cast::<c_void>(),FILE_RENAME_INFO,
        (&info as *const RenameInfo).cast::<c_void>(),
        std::mem::size_of::<RenameInfo>() as u32
    )};
    if result==0{Err(io::Error::last_os_error())}else{Ok(())}
}
