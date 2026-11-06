//! Wave 2 desktop archive workflow. The OS dialogs, not model or WebView,
//! choose the archive and destination. A host-pinned preview is inert until
//! the user reviews all members and accepts a separate native confirmation.
//! Every write is scoped to a newly persisted user turn and EffectJournal.
//! Interrupted tree publications MUST NOT auto-replay; the completion marker
//! is verified against actual member bytes before reporting success.
use std::{
    collections::HashMap,
    fs,
    path::{Path,PathBuf},
    sync::{Arc,Mutex},
};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority},
    journal::EffectJournal,
    model::ModelToolCatalog,
    native_archive_tree::{preview_archive_tree,observe_tree_completion,TreeManifest},
    native_archive_tree_tool::{
        register_archive_tree_extractor,ExactArchiveTreePolicy,TOOL_ID,INPUT,OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::NativeToolRegistry,
    wire::proto,
};
use prost::Message;
use serde::Serialize;
use crate::chat_store::ChatStore;

const MAX_PENDING:usize=8;
fn portable_name(s:&str)->bool{
    !s.is_empty()&&s.len()<=200&&s!="."&&s!=".."
        &&!s.starts_with(".lu-stage-")
        &&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn physical_file(path:&Path)->Result<(String,PathBuf,Arc<Dir>),String>{
    let name=path.file_name().and_then(|n|n.to_str())
        .ok_or("Archive filename is not valid UTF-8")?.to_owned();
    if !portable_name(&name){
        return Err("Archive filename is not an approved flat filename".into())
    }
    let meta=fs::symlink_metadata(path)
        .map_err(|_|"Archive source unavailable")?;
    if !meta.is_file()||meta.file_type().is_symlink(){
        return Err("Archive source must be an actual regular file".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0 {
            return Err("Archive source cannot be a Windows reparse point".into())
        }
    }
    let parent=path.parent().ok_or("Archive has no parent directory")?;
    let capability=Arc::new(Dir::open_ambient_dir(parent,ambient_authority())
        .map_err(|_|"Cannot open archive parent directory")?);
    Ok((name,parent.to_path_buf(),capability))
}
fn physical_folder(path:&Path)->Result<Arc<Dir>,String>{
    let meta=fs::symlink_metadata(path).map_err(|_|"Chosen folder unavailable")?;
    if !meta.is_dir()||meta.file_type().is_symlink(){
        return Err("Choose an actual directory, not a link".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0 {
            return Err("Cannot select a Windows reparse-point directory".into())
        }
    }
    Ok(Arc::new(Dir::open_ambient_dir(path,ambient_authority())
        .map_err(|_|"Cannot open destination directory")?))
}
fn base_for_folder(archive_name:&str)->String{
    let lower=archive_name.to_ascii_lowercase();
    let suffix=[".cpio.gz",".cpio.xz",".cpio.bz2",".cpio.zst",
        ".tar.gz",".tar.xz",".tar.bz2",".tar.zst",".tgz",".tbz2",
        ".txz",".tzst",".zip",".cpio",".deb",".ar",".a",".tar",
        ".gz",".xz",".bz2",".zst"]
        .iter().find(|suffix|lower.ends_with(**suffix)).copied().unwrap_or("");
    let prefix=&archive_name[..archive_name.len()-suffix.len()];
    let trimmed=prefix.chars().filter(|c|c.is_ascii_alphanumeric()||*c=='-'||*c=='_')
        .take(40).collect::<String>();
    if trimmed.is_empty(){"Archive".into()}else{trimmed}
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ArchiveMemberView{
    pub path:String,
    pub kind:String,
    pub bytes:u64,
    pub sha256:String,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ArchiveReviewView{
    pub review_id:String,
    pub chat_id:String,
    pub source_display:String,
    pub destination_display:String,
    pub source_name:String,
    pub target_folder:String,
    pub files:usize,
    pub total_bytes:u64,
    pub manifest_sha256:String,
    pub entries:Vec<ArchiveMemberView>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ArchiveCreatedView{
    pub target_folder:String,
    pub exact_destination:String,
    pub files_extracted:u32,
    pub bytes_extracted:u64,
    pub manifest_sha256:String,
    pub effect_call_id:String,
}
#[derive(Clone)]
pub struct PinnedArchiveReview{
    chat_id:String,
    review_id:String,
    source_name:String,
    source_display:String,
    destination_display:String,
    new_folder:String,
    source_root:Arc<Dir>,
    destination_root:Arc<Dir>,
    manifest:TreeManifest,
}
impl PinnedArchiveReview{
    fn view(&self)->ArchiveReviewView{
        ArchiveReviewView{
            review_id:self.review_id.clone(),chat_id:self.chat_id.clone(),
            source_display:self.source_display.clone(),
            destination_display:self.destination_display.clone(),
            source_name:self.source_name.clone(),
            target_folder:self.new_folder.clone(),
            files:self.manifest.file_count,total_bytes:self.manifest.total_bytes,
            manifest_sha256:self.manifest.sha256.clone(),
            entries:self.manifest.entries.iter().map(|entry|ArchiveMemberView{
                path:entry.path.clone(),
                kind:if entry.is_directory{"directory".into()}else{"file".into()},
                bytes:entry.uncompressed_bytes,
                sha256:entry.sha256.clone(),
            }).collect(),
        }
    }
    pub fn view_for_modal(&self)->ArchiveReviewView{self.view()}
    pub fn same_as(&self,other:&Self)->bool{self.same_identity(other)}
    fn same_identity(&self,other:&Self)->bool{
        self.chat_id==other.chat_id&&self.review_id==other.review_id
            &&self.source_name==other.source_name
            &&self.source_display==other.source_display
            &&self.destination_display==other.destination_display
            &&self.new_folder==other.new_folder
            &&self.manifest==other.manifest
            &&Arc::ptr_eq(&self.source_root,&other.source_root)
            &&Arc::ptr_eq(&self.destination_root,&other.destination_root)
    }
}
#[derive(Default)]
pub struct ArchiveReviewQueue{
    pending:Mutex<HashMap<String,PinnedArchiveReview>>,
}
impl ArchiveReviewQueue{
    /// Only the host calls stage after two independent native OS dialogs.
    pub fn stage(&self,chat_id:&str,review_id:&str,
        source:&Path,destination:&Path
    )->Result<ArchiveReviewView,String>{
        if chat_id.is_empty()||review_id.is_empty()||review_id.len()>128{
            return Err("Invalid archive review identity".into())
        }
        let (source_name,_,source_root)=physical_file(source)?;
        let dest_root=physical_folder(destination)?;
        let manifest=preview_archive_tree(&source_root,&source_name)
            .map_err(|_|"Archive is unsafe, unsupported, or exceeds resource limits")?;
        let prefix=base_for_folder(&source_name);
        let unique=review_id.chars().filter(|c|c.is_ascii_alphanumeric())
            .take(12).collect::<String>();
        let folder=format!("{prefix}-extracted-{unique}");
        if !portable_name(&folder){
            return Err("Invalid generated extraction folder".into())
        }
        // A selected directory is only the PARENT of a new output folder.
        // The user cannot accidentally overwrite a preexisting directory.
        if dest_root.symlink_metadata(&folder).is_ok(){
            return Err("Generated destination already exists; choose another folder".into())
        }
        let pending=PinnedArchiveReview{
            chat_id:chat_id.into(),review_id:review_id.into(),
            source_name,source_display:source.to_string_lossy().into_owned(),
            destination_display:destination.join(&folder).to_string_lossy().into_owned(),
            new_folder:folder,source_root,destination_root:dest_root,manifest,
        };
        let mut entries=self.pending.lock().map_err(|_|"Archive review unavailable")?;
        if entries.len()>=MAX_PENDING{return Err("Too many pending archive reviews".into())}
        if entries.contains_key(review_id){return Err("Duplicate archive review".into())}
        let view=pending.view();
        entries.insert(review_id.into(),pending);
        Ok(view)
    }
    pub fn inspect(&self,chat_id:&str,review_id:&str)
        ->Result<PinnedArchiveReview,String>{
        let entries=self.pending.lock().map_err(|_|"Archive review unavailable")?;
        let review=entries.get(review_id).ok_or("Archive review expired")?;
        if review.chat_id!=chat_id{return Err("Wrong archive conversation".into())}
        Ok(review.clone())
    }
    pub fn consume(&self,chat_id:&str,review_id:&str)
        ->Result<PinnedArchiveReview,String>{
        let mut entries=self.pending.lock().map_err(|_|"Archive review unavailable")?;
        let review=entries.get(review_id).ok_or("Archive review expired")?;
        if review.chat_id!=chat_id{return Err("Wrong archive conversation".into())}
        entries.remove(review_id).ok_or("Archive review already consumed".into())
    }
    pub fn deny(&self,chat_id:&str,review_id:&str)->Result<(),String>{
        let _=self.consume(chat_id,review_id)?;
        Ok(())
    }
}
/// Only reached from an explicit native modal approval after complete member
/// list review; the directory handles and manifest are the pinned originals.
pub async fn execute_reviewed_archive_tree(
    chats:&ChatStore,effects:&EffectJournal,chat_id:&str,
    user_turn_id:&str,effect_call_id:&str,review:&PinnedArchiveReview
)->Result<ArchiveCreatedView,String>{
    chats.append_user_with_turn_id(chat_id,user_turn_id,
        &format!("Approved extracting {} into a NEW folder {}. {} files, {} bytes; full tree manifest SHA-256 {}.",
            review.source_name,review.destination_display,review.manifest.file_count,
            review.manifest.total_bytes,review.manifest.sha256))?;
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_archive_tree_extractor(&registry,&mut catalog,
        Arc::clone(&review.source_root),Arc::clone(&review.destination_root))
        .map_err(|_|"Cannot register native archive extraction")?;
    let authority=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task_id=format!("archive-{user_turn_id}");
    let scope=format!("archive-scope-{user_turn_id}");
    authority.open_user_task(TaskAuthority{
        conversation_id:chat_id.into(),user_turn_id:user_turn_id.into(),
        task_id:task_id.clone(),execution_scope_id:scope.clone(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },chats).map_err(|_|"Archive user turn authorization failed")?;
    policy.grant(CapabilityGrant{
        tool_id:TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:scope.clone(),expires_at:None,
    }).map_err(|_|"Archive scope authorization failed")?;
    let input=proto::ArchiveTreeExtractRequest{
        source_name:review.source_name.clone(),
        target_folder:review.new_folder.clone(),
        approved_manifest_sha256:review.manifest.sha256.clone(),
    };
    let call=ToolInvocation{
        call_id:effect_call_id.into(),tool_id:TOOL_ID.into(),
        workspace_id:Some(scope),
        input:ProtoPayload{type_url:INPUT.into(),bytes:input.encode_to_vec()},
    };
    let fence=ExactArchiveTreePolicy{
        host_policy:&policy,trusted_source:&review.source_root,
        source_name:&review.source_name,
        destination_folder:&review.new_folder,
        manifest_sha256:&review.manifest.sha256,
    };
    let output=authority.dispatch_bound_durable(
        &registry,effects,call,BoundCallOrigin{
            conversation_id:chat_id.into(),user_turn_id:user_turn_id.into(),
            task_id,
        },&fence,
    ).await.map_err(|e|format!("Archive extraction failed or needs manual reconciliation: {e:?}"))?;
    if output.payload.type_url!=OUTPUT{
        return Err("Archive returned incompatible write receipt; review recovery state".into())
    }
    let result=proto::ArchiveTreeExtractResult::decode(output.payload.bytes.as_slice())
        .map_err(|_|"Archive write receipt corrupted; review recovery state")?;
    if result.target_folder!=review.new_folder
        ||result.manifest_sha256!=review.manifest.sha256
        ||result.files_extracted as usize!=review.manifest.file_count
        ||result.bytes_extracted!=review.manifest.total_bytes{
        return Err("Archive receipt differs from approved manifest; review recovery state".into())
    }
    let verified=observe_tree_completion(
        &review.destination_root,&review.new_folder,&review.manifest
    ).map_err(|_|"Cannot reverify extracted folder; inspect for partial output")?;
    if !verified{
        return Err("Extracted output differs from approved tree; manual recovery required".into())
    }
    Ok(ArchiveCreatedView{
        target_folder:review.new_folder.clone(),
        exact_destination:review.destination_display.clone(),
        files_extracted:result.files_extracted,
        bytes_extracted:result.bytes_extracted,
        manifest_sha256:result.manifest_sha256,
        effect_call_id:effect_call_id.into(),
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn user_selected_destination_is_always_new_child(){
        assert_eq!(base_for_folder("archive.tar.gz"),"archive");
        assert_eq!(base_for_folder("photos.zip"),"photos");
        assert_eq!(base_for_folder("...zip"),"Archive");
        assert!(!portable_name("../file"));
        assert!(!portable_name(".lu-stage-fake"));
    }
    #[test]
    fn desktop_preview_pins_native_selected_source_and_new_output_folder(){
        use std::io::Write;
        use zip::{ZipWriter,write::SimpleFileOptions,CompressionMethod};
        let root=std::env::temp_dir().join(format!(
            "lu-archive-desktop-{}",crate::next_id("test-")
        ));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir(root.join("output")).unwrap();
        let source=root.join("backup.zip");
        let mut zip=ZipWriter::new(fs::File::create(&source).unwrap());
        zip.start_file("assets/notes.txt",SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)).unwrap();
        zip.write_all(b"Original content").unwrap();
        zip.finish().unwrap();
        let q=ArchiveReviewQueue::default();
        let view=q.stage("saved-chat","archive-review-123456789",
            &source,&root.join("output")).unwrap();
        assert_eq!(view.files,1);
        assert_eq!(view.entries[0].path,"assets/notes.txt");
        assert_eq!(view.entries[0].kind,"file");
        assert!(!root.join("output").join(&view.target_folder).exists());
        let pinned=q.inspect("saved-chat",&view.review_id).unwrap();
        assert!(q.inspect("another-chat",&view.review_id).is_err());
        assert!(pinned.same_as(&q.inspect("saved-chat",&view.review_id).unwrap()));
        q.deny("saved-chat",&view.review_id).unwrap();
        assert!(q.inspect("saved-chat",&view.review_id).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn archive_review_rejects_linked_input_and_linked_destination(){
        #[cfg(unix)]{
            use std::os::unix::fs::symlink;
            let root=std::env::temp_dir().join(format!(
                "lu-archive-desktop-link-{}",crate::next_id("test-")
            ));
            fs::create_dir_all(root.join("target")).unwrap();
            fs::write(root.join("real.zip"),b"not even an archive").unwrap();
            symlink("real.zip",root.join("source.zip")).unwrap();
            symlink("target",root.join("linked-output")).unwrap();
            let q=ArchiveReviewQueue::default();
            assert!(q.stage("chat","review-123456789",
                &root.join("source.zip"),&root.join("target")).is_err());
            // A real, valid source is required so this checks destination
            // link refusal rather than merely malformed ZIP rejection.
            let mut zip=ZipWriter::new(fs::File::create(root.join("valid.zip")).unwrap());
            zip.start_file("document.txt",SimpleFileOptions::default()
                .compression_method(CompressionMethod::Stored)).unwrap();
            zip.write_all(b"safe bytes").unwrap();
            zip.finish().unwrap();
            assert!(q.stage("chat","review-987654321",
                &root.join("valid.zip"),&root.join("linked-output")).is_err());
            fs::remove_dir_all(root).unwrap();
        }
    }
}
