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
fn load_fixed_named_record(root:&Path,filename:&str,allow_binary:bool)->io::Result<Vec<u8>>{
    let lu=root.join("Lu");
    let ipc=lu.join("IPC");
    let enrollments=ipc.join("enrollments");
    let record=enrollments.join(filename);
    for folder in [&lu,&ipc,&enrollments]{
        let owned=open_custodian(folder,true)?;
        if !owned.metadata()?.is_dir(){return Err(denied("record parent not a directory"));}
    }
    let mut file=open_custodian(&record,false)?;
    let len=file.metadata()?.len();
    if len==0||len>MAX_RECORD_BYTES{return Err(denied("record size exceeds limit"));}
    let mut bytes=Vec::with_capacity(len as usize);
    file.read_to_end(&mut bytes)?;
    if bytes.len()!=len as usize||(!allow_binary && bytes.iter().any(|b|*b==0)){
        return Err(denied("changed or malformed enrollment record"));
    }
    Ok(bytes)
}
fn load_fixed_record(root:&Path)->io::Result<Vec<u8>>{
    load_fixed_named_record(root,"lu.ide.json",false)
}
/// Verify all Lu-owned ancestors of the fixed executor receipt directory,
/// including owner/DACL and reparse checks; never create it on demand.
pub fn protected_executor_receipts_root()->io::Result<PathBuf>{
    let root=os_program_data()?;
    let lu=root.join("Lu");
    let ipc=lu.join("IPC");
    let receipts=ipc.join("scm-executor-receipts-v1");
    for dir in [&lu,&ipc,&receipts] {
        let opened=open_custodian(dir,true)?;
        if !opened.metadata()?.is_dir(){
            return Err(denied("SCM receipt ancestor is not a directory"));
        }
    }
    Ok(receipts)
}
/// The installer-sealed ECDSA P-256 PUBLIC blob, not a private key.
pub fn read_installed_scm_receipt_public_key()->io::Result<[u8;72]>{
    let bytes=load_fixed_named_record(
        &os_program_data()?,"scm-receipt-pub-v1.bin",true)?;
    bytes.try_into().map_err(|_|denied("SCM public-key pin must be exactly 72 bytes"))
}
/// Installer-protected, fixed Core identity for the private SCM control pipe.
/// This is NOT a workspace/DLC capability or a bearer credential.
#[derive(Debug,Clone)]
pub struct InstalledScmCorePins{
    pub image_path:String,
    pub image_sha256:[u8;32],
    pub signer_sha256:[u8;32],
    pub user_sid:String,
}
fn decode_hex32(value:&str)->io::Result<[u8;32]>{
    if value.len()!=64 || !value.bytes().all(|b|
        b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){
        return Err(denied("SCM installed digest is malformed"));
    }
    let mut out=[0u8;32];
    for i in 0..32{
        out[i]=u8::from_str_radix(&value[2*i..2*i+2],16)
            .map_err(|_|denied("SCM installed digest invalid"))?;
    }
    if out.iter().all(|v|*v==0){
        return Err(denied("SCM digest cannot be zero"));
    }
    Ok(out)
}
fn parse_control_host_record(raw:&[u8])->io::Result<InstalledScmCorePins>{
    let text=std::str::from_utf8(raw)
        .map_err(|_|denied("SCM control host UTF-8 invalid"))?;
    let parts:Vec<_>=text.lines().collect();
    if parts.len()!=4 ||raw.len()>2048 ||text.contains('\r'){
        return Err(denied("SCM control host record shape invalid"));
    }
    let image_path=parts[0].strip_prefix("image=")
        .ok_or_else(||denied("SCM image pin missing"))?;
    let hash=parts[1].strip_prefix("sha256=")
        .ok_or_else(||denied("SCM binary digest missing"))?;
    let signer=parts[2].strip_prefix("signer=")
        .ok_or_else(||denied("SCM publisher pin missing"))?;
    let sid=parts[3].strip_prefix("user_sid=")
        .ok_or_else(||denied("SCM user SID missing"))?;
    if !Path::new(image_path).is_absolute()||image_path.len()>1024||
       image_path.contains('\0') ||
       !sid.starts_with("S-1-5-")||sid.len()>128||
       !sid.bytes().all(|b|b.is_ascii_digit()||b==b'-'||b==b'S'){
        return Err(denied("SCM installed Core path or SID invalid"));
    }
    Ok(InstalledScmCorePins{image_path:image_path.into(),
        image_sha256:decode_hex32(hash)?,
        signer_sha256:decode_hex32(signer)?,user_sid:sid.into()})
}
pub fn read_installed_scm_core_pins()->io::Result<InstalledScmCorePins>{
    let record=load_fixed_named_record(&os_program_data()?,
        "scm-control-core-host-v1.txt",false)?;
    parse_control_host_record(&record)
}
pub fn read_installed_scm_core_command_public_key()->io::Result<[u8;72]>{
    let bytes=load_fixed_named_record(
        &os_program_data()?,"scm-control-core-pub-v1.bin",true)?;
    bytes.try_into().map_err(|_|denied("Core command public key pin wrong size"))
}
/// Exact fixed executable selected ONLY by the protected Core installer.
#[derive(Debug,Clone)]
pub struct InstalledScmExecutorPins{
    pub image_path:PathBuf,
    pub image_sha256:[u8;32],
    pub signer_sha256:[u8;32],
}
fn parse_executor_record(raw:&[u8])->io::Result<InstalledScmExecutorPins>{
    let text=std::str::from_utf8(raw)
        .map_err(|_|denied("SCM fixed executor pin not UTF-8"))?;
    let lines:Vec<_>=text.lines().collect();
    if lines.len()!=3||text.len()>2048||text.contains('\r'){
        return Err(denied("SCM fixed executor record shape invalid"));
    }
    let path=lines[0].strip_prefix("image=")
        .ok_or_else(||denied("SCM fixed executor image missing"))?;
    let digest=lines[1].strip_prefix("sha256=")
        .ok_or_else(||denied("SCM fixed executor digest missing"))?;
    let signer=lines[2].strip_prefix("signer=")
        .ok_or_else(||denied("SCM fixed executor signer missing"))?;
    if !Path::new(path).is_absolute()||path.len()>1024||path.contains('\0'){
        return Err(denied("SCM fixed executable path invalid"));
    }
    Ok(InstalledScmExecutorPins{
        image_path:PathBuf::from(path),image_sha256:decode_hex32(digest)?,
        signer_sha256:decode_hex32(signer)?,
    })
}
pub fn read_installed_scm_executor_pins()->io::Result<InstalledScmExecutorPins>{
    let raw=load_fixed_named_record(&os_program_data()?,
        "scm-fixed-executor-v1.txt",false)?;
    parse_executor_record(&raw)
}

/// Fixed installer-owned NVP enrollment. Never accept a client-selected file
/// path or borrow the Lu IDE credential/record as a substitute.
pub fn read_installed_nvp_film_pins()->io::Result<Vec<u8>>{
    load_fixed_named_record(&os_program_data()?,"nvp.film.json",false)
}
/// The independent native NVP SCM P-256 GPU-worker-stop signer public pin.
/// Its private CNG key is never created by Lu or supplied over an IPC frame.
pub fn read_installed_nvp_gpu_stop_public_key()->io::Result<[u8;72]>{
    let blob=load_fixed_named_record(&os_program_data()?,
        "nvp-gpu-stop-p256.pub",true)?;
    if blob.len()!=72 || u32::from_le_bytes(blob[0..4].try_into().unwrap())!=0x31534345 ||
       u32::from_le_bytes(blob[4..8].try_into().unwrap())!=32 ||
       blob[8..].iter().all(|x|*x==0){
        return Err(denied("invalid installed NVP GPU worker signer public blob"))
    }
    let mut out=[0u8;72];out.copy_from_slice(&blob);Ok(out)
}
/// Only accepts a fixed lease-derived name inside an ACL-protected directory
/// where the installed NVP supervisor publishes immutable signed receipts.
pub fn read_installed_nvp_gpu_stop_receipt(lease_ref:&str)->io::Result<Vec<u8>>{
    if lease_ref.is_empty()||lease_ref.len()>128||
       !lease_ref.bytes().all(|b|b.is_ascii_alphanumeric()||b=='-') {
        return Err(denied("invalid NVP GPU stop lease reference"))
    }
    let root=os_program_data()?;
    let lu=root.join("Lu");let ipc=lu.join("IPC");
    let receipts=ipc.join("nvp-gpu-stops-v1");
    for dir in [&lu,&ipc,&receipts]{
        let file=open_custodian(dir,true)?;
        if !file.metadata()?.is_dir(){
            return Err(denied("invalid NVP stop receipt directory"))
        }
    }
    let path=receipts.join(format!("{lease_ref}.stop.v1"));
    let mut file=open_custodian(&path,false)?;
    let len=file.metadata()?.len();
    if !(65..=4096).contains(&len){
        return Err(denied("NVP stop receipt length invalid"))
    }
    let mut bytes=Vec::with_capacity(len as usize);
    file.read_to_end(&mut bytes)?;
    if bytes.len()!=len as usize {
        return Err(denied("NVP stop receipt mutated during read"))
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
    fn no_untrusted_core_host_record_or_drifted_public_pin(){
        let original=format!("image=C:\\Program Files\\Lu\\LuCore.exe\nsha256={}\nsigner={}\nuser_sid=S-1-5-21-100\n",
            "a".repeat(64),"b".repeat(64));
        let parsed=parse_control_host_record(original.as_bytes()).unwrap();
        assert_eq!(parsed.user_sid,"S-1-5-21-100");
        assert_eq!(parsed.image_sha256,[0xaa;32]);
        assert_eq!(parsed.signer_sha256,[0xbb;32]);
        assert!(parse_control_host_record(original.replace("signer=","owner=").as_bytes()).is_err());
        assert!(parse_control_host_record(original.replace("S-1-5-21-100","S-1-5-21-X").as_bytes()).is_err());
        assert!(parse_control_host_record(original.replace("sha256=","sha256=00").as_bytes()).is_err());
    }
    #[test]
    fn fixed_executor_is_never_from_client_arguments(){
        let raw=format!("image=C:\\Program Files\\Lu\\LuNativeWorker.exe\nsha256={}\nsigner={}\n",
            "1".repeat(64),"2".repeat(64));
        assert!(parse_executor_record(raw.as_bytes()).is_ok());
        assert!(parse_executor_record(raw.replace("image=","argv=").as_bytes()).is_err());
        assert!(parse_executor_record(raw.replace("signer=","command=").as_bytes()).is_err());
    }
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
