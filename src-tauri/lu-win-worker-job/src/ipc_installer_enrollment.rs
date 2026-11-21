//! LU-IPC-01D-R3: SYSTEM/Administrators-only installed DLC enrollment record.
//!
//! The source can validate a protected record but CANNOT create an approved
//! publisher or sign a Lu IDE binary. The record contains public exact pins
//! only, NEVER an IPC handshake secret. A separately privileged host must
//! provision an enrolled key from OS credential custody.
//!
//! Open and inspect the fixed ProgramData\Lu\IPC\enrollments\lu.ide.json
//! location. Every Lu-owned path component must have an explicitly protected
//! owner and DACL consisting ONLY of SYSTEM/Administrators full-control ACEs.
//! We reject any reparse point, inherited or unknown ACE, mutable or overly
//! large record, or environment-selected DLC path.
#![cfg(windows)]

use std::{
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    mem,
    os::windows::{fs::{MetadataExt, OpenOptionsExt}, io::AsRawHandle},
    path::{Path, PathBuf},
    ptr,
};

type Handle = *mut c_void;
const OWNER_SECURITY_INFORMATION: u32=1;
const DACL_SECURITY_INFORMATION: u32=4;
const SE_FILE_OBJECT: u32=1;
const SE_DACL_PROTECTED: u16=0x1000;
const ACCESS_ALLOWED_ACE_TYPE: u8=0;
const FILE_ATTRIBUTE_REPARSE_POINT: u32=0x400;
const FILE_SHARE_READ: u32=1;
const FILE_FLAG_BACKUP_SEMANTICS: u32=0x0200_0000;
const MAX_RECORD_BYTES: u64=4096;
const ADMIN_SID: &str="S-1-5-32-544";
const SYSTEM_SID: &str="S-1-5-18";

#[repr(C)]
struct AclHeader {
    revision:u8, unused:u8, size:u16, count:u16, unused2:u16,
}
#[repr(C)]
struct AceHeader {
    kind:u8, flags:u8, size:u16,
}
#[repr(C)]
struct AllowedAce {
    header:AceHeader,
    mask:u32,
    first_sid:u32,
}
#[link(name="advapi32")]
unsafe extern "system" {
    fn GetSecurityInfo(
        handle:Handle,object_type:u32,security_info:u32,owner:*mut Handle,
        group:*mut Handle,dacl:*mut Handle,sacl:*mut Handle,
        security_descriptor:*mut Handle,
    )->u32;
    fn GetSecurityDescriptorControl(
        descriptor:Handle,control:*mut u16,revision:*mut u32,
    )->i32;
    fn GetAce(acl:Handle,index:u32,ace:*mut Handle)->i32;
    fn ConvertSidToStringSidW(sid:Handle,out:*mut *mut u16)->i32;
}
#[link(name="kernel32")]
unsafe extern "system" {
    fn LocalFree(handle:Handle)->Handle;
}

fn denied(message:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::PermissionDenied,message)
}
struct OwnedLocal(Handle);
impl Drop for OwnedLocal {
    fn drop(&mut self) {
        if !self.0.is_null(){unsafe{LocalFree(self.0)};}
    }
}
fn sid_to_string(sid:Handle)->io::Result<String>{
    if sid.is_null(){return Err(denied("absent security principal"));}
    let mut ptr:*mut u16=ptr::null_mut();
    if unsafe{ConvertSidToStringSidW(sid,&mut ptr)}==0||ptr.is_null(){
        return Err(io::Error::last_os_error());
    }
    let _owner=OwnedLocal(ptr.cast());
    let mut count=0usize;
    while count<256&&unsafe{*ptr.add(count)}!=0{count+=1;}
    if count==0||count==256{return Err(denied("security principal too long"));}
    String::from_utf16(unsafe{std::slice::from_raw_parts(ptr,count)})
        .map_err(|_|denied("invalid security principal"))
}
fn strong_dacl(file:&File)->io::Result<()> {
    let mut owner:Handle=ptr::null_mut();
    let mut dacl:Handle=ptr::null_mut();
    let mut descriptor:Handle=ptr::null_mut();
    let status=unsafe{GetSecurityInfo(
        file.as_raw_handle().cast(),SE_FILE_OBJECT,
        OWNER_SECURITY_INFORMATION|DACL_SECURITY_INFORMATION,
        &mut owner,ptr::null_mut(),&mut dacl,ptr::null_mut(),&mut descriptor,
    )};
    if status!=0||descriptor.is_null() {
        return Err(denied("cannot inspect installed record owner/DACL"));
    }
    let _descriptor=OwnedLocal(descriptor);
    if sid_to_string(owner)?!=SYSTEM_SID&&sid_to_string(owner)?!=ADMIN_SID {
        return Err(denied("record path not owned by SYSTEM or Administrators"));
    }
    if dacl.is_null(){return Err(denied("record DACL absent"));}
    let mut control=0u16;
    let mut revision=0u32;
    if unsafe{GetSecurityDescriptorControl(
        descriptor,&mut control,&mut revision)}==0||
        control&SE_DACL_PROTECTED==0 {
        return Err(denied("record DACL allows inherited permissions"));
    }
    let header=unsafe{&*(dacl as *const AclHeader)};
    if header.size<mem::size_of::<AclHeader>() as u16||
        header.count<2||header.count>16 {
        return Err(denied("unexpected installed enrollment ACL"));
    }
    let (mut admin,mut system)=(false,false);
    for i in 0..u32::from(header.count) {
        let mut ace:Handle=ptr::null_mut();
        if unsafe{GetAce(dacl,i,&mut ace)}==0||ace.is_null(){
            return Err(denied("cannot inspect enrollment ACL entry"));
        }
        let header=unsafe{&*(ace as *const AceHeader)};
        // No deny/conditional/object/audit/unknown or inherited entries:
        // this is a deliberately strict installer-owned private DACL.
        if header.kind!=ACCESS_ALLOWED_ACE_TYPE||
           header.flags!=0||
           usize::from(header.size)<mem::size_of::<AllowedAce>() {
            return Err(denied("unsupported enrollment ACL entry"));
        }
        let value=unsafe{&*(ace as *const AllowedAce)};
        let sid_addr:Handle=ptr::addr_of!(value.first_sid) as Handle;
        let subject=sid_to_string(sid_addr)?;
        // GenericAll or standard FILE_ALL_ACCESS from .NET ACL methods.
        if value.mask&0x1000_0000==0&&
           value.mask&0x001f_01ff!=0x001f_01ff {
            return Err(denied("enrollment custodians lack full control"));
        }
        match subject.as_str() {
            ADMIN_SID if !admin =>admin=true,
            SYSTEM_SID if !system =>system=true,
            _=>return Err(denied("record accessible to non-installer principal")),
        }
    }
    if !admin||!system {return Err(denied("missing native installer custodians"));}
    Ok(())
}
fn no_reparse(path:&Path)->io::Result<()>{
    if fs::symlink_metadata(path)?.file_attributes()&FILE_ATTRIBUTE_REPARSE_POINT!=0 {
        return Err(denied("installer record reparse path"));
    }
    Ok(())
}
fn open_custodian(path:&Path,is_directory:bool)->io::Result<File>{
    no_reparse(path)?;
    let mut option=OpenOptions::new();
    option.read(true).share_mode(FILE_SHARE_READ);
    if is_directory {option.custom_flags(FILE_FLAG_BACKUP_SEMANTICS);}
    let file=option.open(path)?;
    strong_dacl(&file)?;
    no_reparse(path)?;
    Ok(file)
}
fn os_program_data()->io::Result<PathBuf>{
    // The host service, not a model/Hello/frame, owns its process environment.
    // If run under a user-controlled process, ACL owner and DACL checks still
    // reject an attacker-created Lu folder under a forged ProgramData.
    let raw=std::env::var_os("ProgramData")
        .ok_or_else(||denied("ProgramData unavailable"))?;
    let path=PathBuf::from(raw);
    if !path.is_absolute(){return Err(denied("relative ProgramData path"));}
    no_reparse(&path)?;
    Ok(path)
}
fn load_fixed_record(root:&Path)->io::Result<Vec<u8>>{
    let lu=root.join("Lu");
    let ipc=lu.join("IPC");
    let enrollments=ipc.join("enrollments");
    let record=enrollments.join("lu.ide.json");
    for folder in [&lu,&ipc,&enrollments]{
        let owned=open_custodian(folder,true)?;
        if !owned.metadata()?.is_dir(){return Err(denied("record parent not a directory"));}
    }
    let mut file=open_custodian(&record,false)?;
    let len=file.metadata()?.len();
    if len==0||len>MAX_RECORD_BYTES{return Err(denied("record size exceeds limit"));}
    let mut bytes=Vec::with_capacity(len as usize);
    file.read_to_end(&mut bytes)?;
    if bytes.len()!=len as usize||bytes.iter().any(|b|*b==0){
        return Err(denied("changed or malformed enrollment record"));
    }
    Ok(bytes)
}
/// Read ONLY the deterministic, protected, installed Lu IDE enrollment.
/// The OS credential key is deliberately NOT present in these record bytes.
pub fn read_installed_lu_ide_pins()->io::Result<Vec<u8>>{
    load_fixed_record(&os_program_data()?)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn refuses_user_owned_or_missing_installer_record(){
        let scratch=std::env::temp_dir().join(format!(
            "lu-untrusted-enroll-{}",std::process::id()));
        fs::create_dir_all(scratch.join(r"Lu\IPC\enrollments")).unwrap();
        fs::write(scratch.join(r"Lu\IPC\enrollments\lu.ide.json"),
            b"{\"schema\":\"pretend\"}").unwrap();
        assert!(load_fixed_record(&scratch).is_err(),
            "ordinary user-writable test files cannot become installer custody");
        fs::remove_dir_all(scratch).unwrap();
    }
}
