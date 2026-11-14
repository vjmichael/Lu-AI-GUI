//! FIN-04W2 read-only source/PE/PDB staging for the untrusted AppContainer.
//! Writable LocalState is only scratch; it is NEVER the executable or PDB
//! staging root. Only an independently authenticated SCM service may stage.
//! This module writes an exact 3-principal protected DACL (SYSTEM, service,
//! exact package SID), then independently reopens the OS descriptor and
//! compares every ACE. No Everyone, Users, AllPackages, inheritance or
//! package write/delete/control is admitted. Exact immutable bytes + ACL
//! readback are BOTH mandatory. No process starts or resumes here.
#![cfg(all(windows, debug_assertions, feature="fin04w2-split-native"))]

use crate::{windows_fin04w2_kernel, windows_service_security, SupervisorError};
use std::{ffi::{c_void,OsStr},fs,mem::size_of,os::windows::{
    ffi::OsStrExt,fs::MetadataExt,
},path::{Path,PathBuf},ptr};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            ConvertStringSidToSidW, GetNamedSecurityInfoW, SetNamedSecurityInfoW,
            SE_FILE_OBJECT,
        },
        EqualSid,GetAce,GetAclInformation,GetSecurityDescriptorControl,
        GetSecurityDescriptorDacl,ACCESS_ALLOWED_ACE,ACL_SIZE_INFORMATION,
        AclSizeInformation,DACL_SECURITY_INFORMATION,PROTECTED_DACL_SECURITY_INFORMATION,
        SE_DACL_PROTECTED,PSID,
    },
};

const ALL: u32=0x001F_01FF;
const READ_EXECUTE: u32=0x0012_00A9;
const REPARSE:u32=0x400;
const INHERITED:u8=0x10;
const OICI:u8=0x03;
const MAX_PDB:u64=512*1024*1024;
fn denied()->SupervisorError{SupervisorError::NoContainment}
fn w(os:&OsStr)->Vec<u16>{os.encode_wide().chain([0]).collect()}
struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self){if !self.0.is_null(){unsafe{LocalFree(self.0);}}}
}
fn literal_sid(text:&str)->Result<Local,SupervisorError>{
    let mut raw=ptr::null_mut();
    if unsafe{ConvertStringSidToSidW(w(OsStr::new(text)).as_ptr(),&mut raw)}==0
        ||raw.is_null(){return Err(denied());}
    Ok(Local(raw))
}
fn string_sid(sid:PSID)->Result<String,SupervisorError>{
    let mut raw=ptr::null_mut();
    if sid.is_null()
        ||unsafe{ConvertSidToStringSidW(sid,&mut raw)}==0||raw.is_null(){
        return Err(denied());
    }
    let owned=Local(raw.cast());
    let mut n=0usize;
    while n<2048&&unsafe{*raw.add(n)}!=0{n+=1;}
    if n==2048{return Err(denied());}
    let _owned=owned;
    String::from_utf16(unsafe{std::slice::from_raw_parts(raw,n)})
        .map_err(|_|denied())
}
fn eligible_file(path:&Path)->Result<(),SupervisorError>{
    if !path.is_absolute(){return Err(denied());}
    let m=fs::symlink_metadata(path).map_err(|_|denied())?;
    if !m.is_file()||m.file_attributes()&REPARSE!=0||m.len()==0||m.len()>MAX_PDB{
        return Err(denied());
    }
    Ok(())
}
/// The stage lives under a service-only directory. Only after both private
/// copies have been sealed do we permit the EXACT package read/traverse ACE.
fn apply_protected_dacl(path:&Path, package:&str, service:&str,dir:bool)
    ->Result<(),SupervisorError>{
    let inherit=if dir{"(A;OICI;"}else{"(A;;"};
    let sddl=if dir{
        format!("D:P(A;OICI;0x{ALL:08x};;;SY)(A;OICI;0x{ALL:08x};;;{service})\
            {inherit}0x{READ_EXECUTE:08x};;;{package})")
    }else{
        format!("D:P(A;;0x{ALL:08x};;;SY)(A;;0x{ALL:08x};;;{service})\
            {inherit}0x{READ_EXECUTE:08x};;;{package})")
    };
    // Parse the descriptor locally, but DO NOT accept it as the proof.
    let mut sd=ptr::null_mut();
    let mut sd_len=0u32;
    if unsafe{ConvertStringSecurityDescriptorToSecurityDescriptorW(
        w(OsStr::new(&sddl)).as_ptr(),1,&mut sd,&mut sd_len)
    }==0||sd.is_null()||sd_len==0{return Err(denied());}
    let sd=Local(sd);
    let mut present=0i32;
    let mut dacl=ptr::null_mut();
    let mut defaulted=0i32;
    if unsafe{GetSecurityDescriptorDacl(
        sd.0,&mut present,&mut dacl,&mut defaulted)
    }==0||present==0||dacl.is_null(){return Err(denied());}
    if unsafe{SetNamedSecurityInfoW(
        w(path.as_os_str()).as_mut_ptr(),SE_FILE_OBJECT,
        DACL_SECURITY_INFORMATION|PROTECTED_DACL_SECURITY_INFORMATION,
        ptr::null_mut(),ptr::null_mut(),dacl,ptr::null_mut(),
    )}!=0{return Err(denied());}
    Ok(())
}
/// Independent kernel DACL readback. This intentionally does not trust the
/// in-memory SDDL from the writer or a stamped metadata/receipt flag.
pub(crate) fn verify_readonly_stage_entry(
    path:&Path,package_sid:PSID,is_dir:bool,
)->Result<(),SupervisorError>{
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    let meta=fs::symlink_metadata(path).map_err(|_|denied())?;
    if !path.is_absolute()||meta.file_attributes()&REPARSE!=0
        ||(is_dir&&!meta.is_dir())||(!is_dir&&!meta.is_file()){
        return Err(denied());
    }
    let service=literal_sid(&windows_service_security::sid_of_named_service(
        "LuIdeDebuggerBroker")?)?;
    let system=literal_sid("S-1-5-18")?;
    if package_sid.is_null(){return Err(denied());}
    let mut dacl=ptr::null_mut();
    let mut sd=ptr::null_mut();
    if unsafe{GetNamedSecurityInfoW(
        w(path.as_os_str()).as_ptr(),SE_FILE_OBJECT,DACL_SECURITY_INFORMATION,
        ptr::null_mut(),ptr::null_mut(),&mut dacl,ptr::null_mut(),&mut sd)
    }!=0||sd.is_null()||dacl.is_null(){return Err(denied());}
    let _sd=Local(sd);
    let mut control=0u16;
    let mut revision=0u32;
    if unsafe{GetSecurityDescriptorControl(
        sd,&mut control,&mut revision)
    }==0||control&SE_DACL_PROTECTED==0{return Err(denied());}
    let mut info=ACL_SIZE_INFORMATION::default();
    if unsafe{GetAclInformation(
        dacl,(&mut info as *mut ACL_SIZE_INFORMATION).cast(),
        size_of::<ACL_SIZE_INFORMATION>() as u32,AclSizeInformation)
    }==0||info.AceCount!=3{return Err(denied());}
    let mut seen=[false;3];
    for i in 0..3 {
        let mut raw=ptr::null_mut();
        if unsafe{GetAce(dacl,i,&mut raw)}==0||raw.is_null(){return Err(denied());}
        let ace=unsafe{&*(raw as *const ACCESS_ALLOWED_ACE)};
        let flags=if is_dir{OICI}else{0};
        if ace.Header.AceType!=0||ace.Header.AceFlags!=flags
            ||ace.Header.AceFlags&INHERITED!=0
            ||ace.Header.AceSize<size_of::<ACCESS_ALLOWED_ACE>() as u16 {
            return Err(denied());
        }
        let sid=(&ace.SidStart as *const u32).cast_mut().cast();
        let which=if unsafe{EqualSid(sid,system.0)}!=0{0}
            else if unsafe{EqualSid(sid,service.0)}!=0{1}
            else if unsafe{EqualSid(sid,package_sid)}!=0{2}
            else {return Err(denied());};
        if seen[which]
            ||ace.Mask!=if which==2{READ_EXECUTE}else{ALL}{
            return Err(denied());
        }
        seen[which]=true;
    }
    if !seen.into_iter().all(|b|b){return Err(denied());}
    Ok(())
}
/// The parent never grants package access: only the exact new per-job leaf
/// may be traversed by this target SID, after its files are sealed.
fn apply_private_parent_sddl(path:&Path,descriptor:&str)
    ->Result<(),SupervisorError>{
    let mut sd=ptr::null_mut();
    let mut sd_len=0u32;
    if unsafe{ConvertStringSecurityDescriptorToSecurityDescriptorW(
        w(OsStr::new(descriptor)).as_ptr(),1,&mut sd,&mut sd_len)
    }==0||sd.is_null()||sd_len==0{return Err(denied());}
    let sd=Local(sd);
    let mut present=0i32;
    let mut dacl=ptr::null_mut();
    let mut defaulted=0i32;
    if unsafe{GetSecurityDescriptorDacl(
        sd.0,&mut present,&mut dacl,&mut defaulted)
    }==0||present==0||dacl.is_null(){return Err(denied());}
    if unsafe{SetNamedSecurityInfoW(
        w(path.as_os_str()).as_mut_ptr(),SE_FILE_OBJECT,
        DACL_SECURITY_INFORMATION|PROTECTED_DACL_SECURITY_INFORMATION,
        ptr::null_mut(),ptr::null_mut(),dacl,ptr::null_mut(),
    )}!=0{return Err(denied());}
    Ok(())
}
fn verify_private_stage_parent(path:&Path)->Result<(),SupervisorError>{
    let service=literal_sid(&windows_service_security::sid_of_named_service(
        "LuIdeDebuggerBroker")?)?;
    let system=literal_sid("S-1-5-18")?;
    let mut dacl=ptr::null_mut();
    let mut sd=ptr::null_mut();
    if unsafe{GetNamedSecurityInfoW(
        w(path.as_os_str()).as_ptr(),SE_FILE_OBJECT,DACL_SECURITY_INFORMATION,
        ptr::null_mut(),ptr::null_mut(),&mut dacl,ptr::null_mut(),&mut sd)
    }!=0||sd.is_null()||dacl.is_null(){return Err(denied());}
    let _sd=Local(sd);
    let mut control=0u16;
    let mut revision=0u32;
    if unsafe{GetSecurityDescriptorControl(
        sd,&mut control,&mut revision)
    }==0||control&SE_DACL_PROTECTED==0{return Err(denied());}
    let mut info=ACL_SIZE_INFORMATION::default();
    if unsafe{GetAclInformation(
        dacl,(&mut info as *mut ACL_SIZE_INFORMATION).cast(),
        size_of::<ACL_SIZE_INFORMATION>() as u32,AclSizeInformation)
    }==0||info.AceCount!=2{return Err(denied());}
    let mut seen=[false;2];
    for i in 0..2 {
        let mut raw=ptr::null_mut();
        if unsafe{GetAce(dacl,i,&mut raw)}==0||raw.is_null(){return Err(denied());}
        let ace=unsafe{&*(raw as *const ACCESS_ALLOWED_ACE)};
        if ace.Header.AceType!=0||ace.Header.AceFlags!=OICI
            ||ace.Header.AceSize<size_of::<ACCESS_ALLOWED_ACE>() as u16
            ||ace.Mask!=ALL{return Err(denied());}
        let sid=(&ace.SidStart as *const u32).cast_mut().cast();
        let k=if unsafe{EqualSid(sid,system.0)}!=0{0}
            else if unsafe{EqualSid(sid,service.0)}!=0{1}
            else{return Err(denied());};
        if seen[k]{return Err(denied());}
        seen[k]=true;
    }
    if seen!=[true,true]{return Err(denied());}
    Ok(())
}
struct IncompleteStage(PathBuf);
impl Drop for IncompleteStage {
    fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}
}

pub(crate) struct SealedDebugTarget {
    pub(crate) image:PathBuf,
    pub(crate) pdb:PathBuf,
    pub(crate) root:PathBuf,
}
/// Exact sealed target copies. The parent is the restricted SCM service
/// root, never a user-writable AppContainer local state, Git worktree, etc.
pub(crate) fn stage_and_verify(
    service_root:&Path,original_job_id:&str,
    package_sid:PSID,image:&Path,image_sha256:&str,
    pdb:&Path,pdb_sha256:&str,
)->Result<SealedDebugTarget,SupervisorError>{
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    let service=windows_service_security::sid_of_named_service("LuIdeDebuggerBroker")?;
    windows_service_security::verify_private_evidence_root_for(service_root,&service)?;
    if original_job_id.len()!=36||!original_job_id.starts_with("job-")
        ||!original_job_id[4..].bytes().all(|b|b.is_ascii_hexdigit())
        ||image==pdb{return Err(denied());}
    windows_service_security::hash_executable(image,image_sha256)?;
    eligible_file(pdb)?;
    windows_service_security::hash_executable(pdb,pdb_sha256)?;
    let parent=service_root.join("sealed-debug-targets");
    // The service root is already independently validated; a NEW parent
    // inherits no package capability and is immediately locked to two SIDs.
    if !parent.exists(){
        fs::create_dir(&parent).map_err(|_|denied())?;
        let descriptor=format!(
            "D:P(A;OICI;0x{ALL:08x};;;SY)(A;OICI;0x{ALL:08x};;;{service})"
        );
        apply_private_parent_sddl(&parent,&descriptor)?;
    }
    let parent_meta=fs::symlink_metadata(&parent).map_err(|_|denied())?;
    if !parent_meta.is_dir()||parent_meta.file_attributes()&REPARSE!=0{
        return Err(denied());
    }
    verify_private_stage_parent(&parent)?;
    let root=parent.join(original_job_id);
    if root.exists(){return Err(denied());}
    fs::create_dir(&root).map_err(|_|denied())?;
    let mut incomplete=Some(IncompleteStage(root.clone()));
    let exe=root.join("debug-target.exe");
    let symbols=root.join("debug-target.pdb");
    fs::copy(image,&exe).map_err(|_|denied())?;
    fs::copy(pdb,&symbols).map_err(|_|denied())?;
    windows_service_security::hash_executable(&exe,image_sha256)?;
    windows_service_security::hash_executable(&symbols,pdb_sha256)?;
    let package=string_sid(package_sid)?;
    // Seal the individual files before making the stage root traversable.
    apply_protected_dacl(&exe,&package,&service,false)?;
    apply_protected_dacl(&symbols,&package,&service,false)?;
    verify_readonly_stage_entry(&exe,package_sid,false)?;
    verify_readonly_stage_entry(&symbols,package_sid,false)?;
    apply_protected_dacl(&root,&package,&service,true)?;
    verify_readonly_stage_entry(&root,package_sid,true)?;
    // Rehash AFTER ACL application with FILE_SHARE_READ-only access.
    windows_service_security::hash_executable(&exe,image_sha256)?;
    windows_service_security::hash_executable(&symbols,pdb_sha256)?;
    // Only the successful, independently reobserved stage escapes cleanup.
    incomplete.take();
    Ok(SealedDebugTarget{image:exe,pdb:symbols,root})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unapproved_local_runner_cannot_stage_a_mutable_debuggee(){
        // An ordinary user or elevated CI runner is NOT the service SID.
        // This prevents a "use writable LocalState and hash it" fallback.
        let result=stage_and_verify(
            Path::new(r"C:\ProgramData\LuIDEDebuggerBroker"),
            "job-00000000000000000000000000000000",
            ptr::null_mut(),
            Path::new(r"C:\Temp\untrusted-debuggee.exe"),&"a".repeat(64),
            Path::new(r"C:\Temp\untrusted-debuggee.pdb"),&"b".repeat(64),
        );
        assert!(result.is_err());
    }
}
