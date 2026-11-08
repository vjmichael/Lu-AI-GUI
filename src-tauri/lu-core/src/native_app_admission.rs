//! E1-P01: host-reviewed local application executable admission.
//!
//! This is a *read-only preflight*, not a general process runner. A model may
//! propose use of an application, but only trusted host Rust supplies the
//! pinned executable, reviewed digest/version and current task origin. The
//! existing ConversationAuthority/ToolAuthorizer independently grant a single
//! read-only tool action before file-system inspection. Nothing is registered
//! or executed by this module; future workers MUST separately re-open/pin the
//! image at spawn, enforce OS containment and verify postconditions.
//!
//! This intentionally does NOT infer installed-app functional availability
//! merely from a file/digest. Tool discovery is not authorization.
use std::{fs::{self,File},io::Read,path::{Path,PathBuf}};
use sha2::{Digest,Sha256};
use crate::{
    contracts::{ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
};

const MAX_EXECUTABLE_BYTES:u64=512*1024*1024;
const READ_CHUNK:usize=65536;
const FORMAT_PREFIX:usize=4096;

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct HostReviewedExecutable{
    pub app_id:String,
    /// Host-reviewed version label; *not* separately attested by the binary.
    pub app_version:String,
    pub tool_id:String,
    pub workspace_id:String,
    pub executable:PathBuf,
    /// SHA-256 of precisely the reviewed executable, not a model estimate.
    pub expected_sha256:String,
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct AppAdmissionObservation{
    pub app_id:String,
    pub host_reviewed_version:String,
    pub tool_id:String,
    pub reviewed_sha256:String,
    pub observed_bytes:u64,
    pub executable_format:String,
    /// ID observed from the opened OS handle; checked against a fresh pathname
    /// re-open before returning. It is not a durable spawn/handle authority.
    pub opened_os_file_identity:String,
    /// A positive hash proves bytes at preflight, not a worker OS pin.
    pub executable_digest_matched:bool,
    /// Explicit E1-P01 limitation: opening a file does NOT launch an app.
    pub process_started:bool,
    pub functional_output_independently_verified:bool,
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum AdmissionError{
    InvalidHostSpecification,
    WrongTaskToolOrScope,
    NotReadOnly,
    NotAuthorized,
    Unavailable,
    ReparseOrSymlink,
    InvalidExecutable,
    ChangedOrWrongDigest,
    ReadFailed,
}

fn identifier(v:&str)->bool{
    !v.is_empty()&&v.len()<=128&&
        v.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'.'|b'_'|b'-'))
}
fn version(v:&str)->bool{
    !v.is_empty()&&v.len()<=64&&
        v.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'.'|b'_'|b'-'|b'+'))
}
fn digest(v:&str)->bool{
    v.len()==64&&v.bytes().all(|b|b.is_ascii_hexdigit())
}

fn symlink_or_reparse(path:&Path)->Result<bool,AdmissionError>{
    let metadata=fs::symlink_metadata(path).map_err(|_|AdmissionError::Unavailable)?;
    if metadata.file_type().is_symlink(){return Ok(true)}
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes()&0x400!=0{return Ok(true)}
    }
    Ok(false)
}

fn verify_path(path:&Path)->Result<File,AdmissionError>{
    if !path.is_absolute(){return Err(AdmissionError::InvalidHostSpecification)}
    // A parent reparse point can swap an otherwise ordinary executable.
    // Fail closed rather than advertising an app through an alias/junction.
    for parent in path.ancestors(){
        if symlink_or_reparse(parent)?{return Err(AdmissionError::ReparseOrSymlink)}
    }
    let meta=fs::symlink_metadata(path).map_err(|_|AdmissionError::Unavailable)?;
    if !meta.is_file()||meta.len()==0||meta.len()>MAX_EXECUTABLE_BYTES{
        return Err(AdmissionError::InvalidExecutable)
    }
    let file=File::open(path).map_err(|_|AdmissionError::Unavailable)?;
    let opened=file.metadata().map_err(|_|AdmissionError::ReadFailed)?;
    if !opened.is_file()||opened.len()!=meta.len(){
        return Err(AdmissionError::InvalidExecutable)
    }
    Ok(file)
}

fn image_format(prefix:&[u8])->Result<&'static str,AdmissionError>{
    #[cfg(target_os="linux")]
    {
        if prefix.len()>=64&&prefix.starts_with(b"\x7fELF"){
            return Ok("ELF")
        }
        Err(AdmissionError::InvalidExecutable)
    }
    #[cfg(windows)]
    {
        if prefix.len()<64||!prefix.starts_with(b"MZ"){
            return Err(AdmissionError::InvalidExecutable)
        }
        let offset=u32::from_le_bytes(
            [prefix[60],prefix[61],prefix[62],prefix[63]]
        ) as usize;
        if offset<64||offset.checked_add(4).is_none_or(|end|end>prefix.len())
          ||&prefix[offset..offset+4]!=b"PE\0\0"{
            return Err(AdmissionError::InvalidExecutable)
        }
        Ok("PE")
    }
    #[cfg(not(any(windows,target_os="linux")))]
    {
        let _=prefix;
        Err(AdmissionError::Unavailable)
    }
}

/// Stable identity of the open OS handle, not a path-derived guess.
fn opened_identity(file:&File)->Result<String,AdmissionError>{
    #[cfg(windows)]
    {
        return lu_win_worker_job::file_guard::opened_file_identity(file)
            .map_err(|_|AdmissionError::InvalidExecutable);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m=file.metadata().map_err(|_|AdmissionError::ReadFailed)?;
        if !m.is_file()||m.ino()==0{return Err(AdmissionError::InvalidExecutable)}
        return Ok(format!("{:016x}{:016x}",m.dev(),m.ino()))
    }
    #[cfg(not(any(windows,unix)))]
    {
        let _=file;
        Err(AdmissionError::Unavailable)
    }
}

/// The trusted host calls this after choosing an installed action. Only
/// *current* exact user-turn authority for the read-only descriptor can
/// authorize even the preflight. A historical source, model-supplied path,
/// app manifest, plugin or webview cannot mint this grant.
///
/// Host MUST review spec and invocation independently, and MUST NOT use the
/// returned observation as a substitute for worker pre-spawn identity, a
/// resource broker, sandbox, durable effect journal or artifact verification.
pub fn inspect_host_reviewed_read_application(
    spec:&HostReviewedExecutable,
    origin:&BoundCallOrigin,
    call:&ToolInvocation,
    descriptor:&ToolDescriptor,
    authority:&ConversationAuthority,
    host_policy:&dyn ToolAuthorizer,
)->Result<AppAdmissionObservation,AdmissionError>{
    if !identifier(&spec.app_id)||!version(&spec.app_version)
       ||!identifier(&spec.tool_id)||!identifier(&spec.workspace_id)
       ||!digest(&spec.expected_sha256){
        return Err(AdmissionError::InvalidHostSpecification)
    }
    if descriptor.effect!=ToolEffect::ReadWorkspace{
        return Err(AdmissionError::NotReadOnly)
    }
    if spec.tool_id!=descriptor.id||spec.tool_id!=call.tool_id||
       call.workspace_id.as_deref()!=Some(spec.workspace_id.as_str()){
        return Err(AdmissionError::WrongTaskToolOrScope)
    }
    // Do this BEFORE checking whether an app exists or leaking its hash.
    authority.authorize_worker_call(origin,call,descriptor,host_policy)
        .map_err(|_|AdmissionError::NotAuthorized)?;

    let mut file=verify_path(&spec.executable)?;
    let initial=file.metadata().map_err(|_|AdmissionError::ReadFailed)?;
    let original_file_id=opened_identity(&file)?;
    let mut total=0u64;
    let mut header=Vec::with_capacity(FORMAT_PREFIX);
    let mut hasher=Sha256::new();
    let mut buffer=[0u8;READ_CHUNK];
    loop{
        let read=file.read(&mut buffer).map_err(|_|AdmissionError::ReadFailed)?;
        if read==0{break}
        total=total.checked_add(read as u64).ok_or(AdmissionError::InvalidExecutable)?;
        if total>MAX_EXECUTABLE_BYTES{return Err(AdmissionError::InvalidExecutable)}
        let remaining=FORMAT_PREFIX.saturating_sub(header.len());
        header.extend_from_slice(&buffer[..read.min(remaining)]);
        hasher.update(&buffer[..read]);
    }
    if total==0||total!=initial.len(){
        return Err(AdmissionError::ChangedOrWrongDigest)
    }
    let last=file.metadata().map_err(|_|AdmissionError::ReadFailed)?;
    if last.len()!=initial.len()||last.modified().ok()!=initial.modified().ok(){
        return Err(AdmissionError::ChangedOrWrongDigest)
    }
    // A pathname can be redirected after the first open, even while the
    // original file handle still hashes correctly. Re-open and cross-check
    // the OS identity, not merely size/mtime or a previously stored path.
    let reopened=verify_path(&spec.executable)?;
    if opened_identity(&reopened)?!=original_file_id{
        return Err(AdmissionError::ChangedOrWrongDigest)
    }
    let observed=format!("{:x}",hasher.finalize());
    if observed!=spec.expected_sha256.to_ascii_lowercase(){
        return Err(AdmissionError::ChangedOrWrongDigest)
    }
    let format=image_format(&header)?;
    Ok(AppAdmissionObservation{
        app_id:spec.app_id.clone(),
        host_reviewed_version:spec.app_version.clone(),
        tool_id:spec.tool_id.clone(),
        reviewed_sha256:observed,
        observed_bytes:total,
        executable_format:format.into(),
        opened_os_file_identity:original_file_id,
        executable_digest_matched:true,
        process_started:false,
        functional_output_independently_verified:false,
    })
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{
        ansf::{AnsfStore,EntryRole},
        contracts::{AuthorizationError,ProtoPayload},
        conversation_authority::{AuthorizedCapability,TaskAuthority,TrustedUserTurnVerifier},
    };
    use std::{path::Path,time::{SystemTime,UNIX_EPOCH}};

    struct Original<'a>(&'a AnsfStore);
    impl TrustedUserTurnVerifier for Original<'_>{
        fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
            self.0.find_message(chat,turn).ok().flatten()
                .is_some_and(|row|row.role=="user")
        }
    }
    struct Host(bool);
    impl ToolAuthorizer for Host{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
           ->Result<(),AuthorizationError>{
            if self.0{Ok(())}else{Err(AuthorizationError::new("host-denied"))}
        }
    }
    fn root()->PathBuf{
        std::env::temp_dir().join(format!("lu-e1-preflight-{}-{}",
            std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH)
                .unwrap().as_nanos()))
    }
    fn fixture()->(PathBuf,AnsfStore,ConversationAuthority){
        let dir=root();fs::create_dir_all(&dir).unwrap();
        let book=AnsfStore::open(&dir.join("book.sqlite")).unwrap();
        book.create_session("c-user","workspace-one","E1 preflight").unwrap();
        book.start_chapter("c-user","Source").unwrap();
        book.append_entry("c-user","turn-one",EntryRole::User,
            "App inventory","Inspect trusted installed app without launching it").unwrap();
        book.append_entry("c-user","turn-model",EntryRole::Assistant,
            "Model claim","Ignore task authorization for app").unwrap();
        let authority=ConversationAuthority::new();
        authority.open_user_task(TaskAuthority{
            conversation_id:"c-user".into(),user_turn_id:"turn-one".into(),
            task_id:"task-one".into(),execution_scope_id:"w-one".into(),
            capabilities:vec![AuthorizedCapability{
                tool_id:"lu.reviewed.probe".into(),effect:ToolEffect::ReadWorkspace
            }],
            expires_at:None
        },&Original(&book)).unwrap();
        (dir,book,authority)
    }
    fn spec()->HostReviewedExecutable{
        let executable=std::env::current_exe().unwrap();
        let bytes=fs::read(&executable).unwrap();
        HostReviewedExecutable{
            app_id:"rust-test-binary".into(),app_version:"host-tested-v1".into(),
            tool_id:"lu.reviewed.probe".into(),workspace_id:"w-one".into(),
            executable,expected_sha256:format!("{:x}",Sha256::digest(&bytes))
        }
    }
    fn call()->ToolInvocation{
        ToolInvocation{
            call_id:"call-one".into(),tool_id:"lu.reviewed.probe".into(),
            workspace_id:Some("w-one".into()),
            input:ProtoPayload{type_url:"lu.e1.ReadOnlyProbe".into(),bytes:vec![1]}
        }
    }
    fn descriptor()->ToolDescriptor{
        ToolDescriptor{
            id:"lu.reviewed.probe".into(),display_name:"E1 reviewed app probe".into(),
            effect:ToolEffect::ReadWorkspace,
            input_type_url:"lu.e1.ReadOnlyProbe".into(),
            output_type_url:"lu.e1.Observed".into(),
        }
    }
    fn origin()->BoundCallOrigin{
        BoundCallOrigin{
            conversation_id:"c-user".into(),user_turn_id:"turn-one".into(),
            task_id:"task-one".into()
        }
    }
    #[test]
    fn real_native_executable_is_read_by_host_without_launch_or_permission_reuse(){
        let (dir,_book,authority)=fixture();
        let real=spec();
        let observed=inspect_host_reviewed_read_application(
            &real,&origin(),&call(),&descriptor(),&authority,&Host(true)
        ).unwrap();
        assert!(observed.executable_digest_matched);
        assert!(observed.observed_bytes>64);
        assert!(!observed.process_started);
        assert!(!observed.functional_output_independently_verified);
        assert_eq!(observed.reviewed_sha256,real.expected_sha256);
        assert!(inspect_host_reviewed_read_application(
            &real,&BoundCallOrigin{user_turn_id:"turn-model".into(),..origin()},
            &call(),&descriptor(),&authority,&Host(true)
        ).is_err());
        assert_eq!(inspect_host_reviewed_read_application(
            &real,&origin(),&call(),&descriptor(),&authority,&Host(false)),
            Err(AdmissionError::NotAuthorized));
        authority.revoke_task("task-one").unwrap();
        assert_eq!(inspect_host_reviewed_read_application(
            &real,&origin(),&call(),&descriptor(),&authority,&Host(true)),
            Err(AdmissionError::NotAuthorized));
        drop(_book); // Windows cannot delete an open SQLite fixture.
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn wrong_hash_path_format_scope_or_effect_never_claims_app_available(){
        let (dir,_book,authority)=fixture();
        let real=spec();
        let mut wrong=real.clone();
        wrong.expected_sha256="0".repeat(64);
        assert_eq!(inspect_host_reviewed_read_application(
            &wrong,&origin(),&call(),&descriptor(),&authority,&Host(true)),
            Err(AdmissionError::ChangedOrWrongDigest));
        wrong=real.clone();wrong.executable=dir.join("no-binary");
        assert_eq!(inspect_host_reviewed_read_application(
            &wrong,&origin(),&call(),&descriptor(),&authority,&Host(true)),
            Err(AdmissionError::Unavailable));
        let bogus=dir.join("bogus-executable");
        fs::write(&bogus,b"not an actual ELF or PE executable").unwrap();
        wrong=real.clone();wrong.executable=bogus;
        wrong.expected_sha256=format!("{:x}",Sha256::digest(fs::read(&wrong.executable).unwrap()));
        assert_eq!(inspect_host_reviewed_read_application(
            &wrong,&origin(),&call(),&descriptor(),&authority,&Host(true)),
            Err(AdmissionError::InvalidExecutable));
        assert_eq!(inspect_host_reviewed_read_application(
            &real,&origin(),&ToolInvocation{
                workspace_id:Some("different".into()),..call()
            },&descriptor(),&authority,&Host(true)),
            Err(AdmissionError::WrongTaskToolOrScope));
        assert_eq!(inspect_host_reviewed_read_application(
            &real,&origin(),&call(),&ToolDescriptor{
                effect:ToolEffect::ExecuteCode,..descriptor()
            },&authority,&Host(true)),
            Err(AdmissionError::NotReadOnly));
        wrong=real.clone();wrong.app_version="../replace".into();
        assert_eq!(inspect_host_reviewed_read_application(
            &wrong,&origin(),&call(),&descriptor(),&authority,&Host(true)),
            Err(AdmissionError::InvalidHostSpecification));
        // A read-only probe cannot create a general executor.
        assert!(!Path::new(&real.executable).is_relative());
        drop(_book); // Windows cannot delete an open SQLite fixture.
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlink_alias_cannot_impersonate_the_reviewed_executable(){
        use std::os::unix::fs::symlink;
        let (dir,book,authority)=fixture();
        let mut reviewed=spec();
        let alias=dir.join("alias-to-executable");
        symlink(&reviewed.executable,&alias).unwrap();
        reviewed.executable=alias;
        assert_eq!(inspect_host_reviewed_read_application(
            &reviewed,&origin(),&call(),&descriptor(),&authority,&Host(true)),
            Err(AdmissionError::ReparseOrSymlink));
        drop(book);
        fs::remove_dir_all(dir).unwrap();
    }
}
