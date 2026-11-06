//! Wave 2 native desktop copy/move. OS selects exact file; host owns directory
//! capability, source hash, and unpredictable NEW destination filename.
//! Copy and move are separate one-use WriteWorkspace grants, journaled by
//! authenticated user turn and independently approved in native modal.
//! Never run automatic recovery after partial hard-link/unlink operations.
use std::{collections::HashMap,fs,path::Path,sync::{Arc,Mutex}};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority},
    journal::EffectJournal,model::ModelToolCatalog,
    native_file_transfer::{source_digest,MAX_TRANSFER_BYTES},
    native_file_transfer_tool::{
        register_file_transfer_tools,ExactFileTransferPolicy,
        COPY_ID,MOVE_ID,INPUT,OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},registry::NativeToolRegistry,
    wire::proto,
};
use serde::Serialize;
use prost::Message;
use sha2::{Digest,Sha256};
use crate::chat_store::ChatStore;
const MAX_PENDING:usize=8;
fn safe_name(n:&str)->bool{
    !n.is_empty()&&n.len()<=240&&n!="."&&n!=".."
        &&!n.starts_with(".lu-stage-")&&!n.ends_with('.')&&!n.ends_with(' ')
        &&!n.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn safe_id(s:&str)->bool{
    (16..=128).contains(&s.len())&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||b==b'_'||b==b'-')
}
fn native_source(path:&Path)->Result<(String,Arc<Dir>,u64,String),String>{
    let name=path.file_name().and_then(|s|s.to_str())
        .ok_or("Selected filename unavailable")?;
    if !safe_name(name){return Err("Selected file name not supported".into())}
    let meta=fs::symlink_metadata(path).map_err(|_|"File unavailable")?;
    if !meta.is_file()||meta.file_type().is_symlink()
        ||meta.len()>MAX_TRANSFER_BYTES{
        return Err("File is not a supported regular file under 32 MiB".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err("Windows reparse-point file refused".into())
        }
    }
    let parent=path.parent().ok_or("Containing folder unavailable")?;
    let handle=Arc::new(Dir::open_ambient_dir(parent,ambient_authority())
        .map_err(|_|"Cannot open selected file folder")?);
    let (bytes,digest)=source_digest(&handle,name)
        .map_err(|_|"Cannot hash selected file")?;
    Ok((name.into(),handle,bytes,digest))
}
fn generated_target(source:&str,kind:&str,id:&str)->Result<String,String>{
    let dot=source.rfind('.');
    let (base,ext)=match dot{
        Some(i) if i>0=>(&source[..i],&source[i..]),
        _=>(source,""),
    };
    let base=base.chars().filter(|c|c.is_ascii_alphanumeric()||*c=='-'||*c=='_')
        .take(60).collect::<String>();
    let base=if base.is_empty(){"Lu-File"}else{base.as_str()};
    let suffix=id.bytes().filter(|b|b.is_ascii_alphanumeric())
        .take(10).map(char::from).collect::<String>();
    let name=format!("{base}-{kind}-{suffix}{ext}");
    if !safe_name(&name)||name.eq_ignore_ascii_case(source){
        return Err("Invalid generated new file name".into())
    }
    Ok(name)
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct FileTransferReviewView{
    pub review_id:String,pub chat_id:String,
    pub operation:String,pub source_display:String,
    pub destination_display:String,pub source_name:String,
    pub target_name:String,pub source_sha256:String,
    pub bytes:u64,pub removes_original:bool,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct FileTransferReceiptView{
    pub operation:String,pub target_name:String,
    pub exact_destination:String,pub bytes:u64,
    pub sha256:String,pub source_removed:bool,
    pub effect_call_id:String,
}
#[derive(Clone)]
pub struct PinnedFileTransfer{
    chat_id:String,review_id:String,source_name:String,target_name:String,
    source_display:String,destination_display:String,
    operation:String,bytes:u64,sha256:String,root:Arc<Dir>,
}
impl PinnedFileTransfer{
    pub fn view(&self)->FileTransferReviewView{
        FileTransferReviewView{
            review_id:self.review_id.clone(),chat_id:self.chat_id.clone(),
            operation:self.operation.clone(),
            source_display:self.source_display.clone(),
            destination_display:self.destination_display.clone(),
            source_name:self.source_name.clone(),target_name:self.target_name.clone(),
            source_sha256:self.sha256.clone(),bytes:self.bytes,
            removes_original:self.operation=="move",
        }
    }
    pub fn same_as(&self,other:&Self)->bool{
        self.chat_id==other.chat_id&&self.review_id==other.review_id
            &&self.source_name==other.source_name&&self.target_name==other.target_name
            &&self.operation==other.operation&&self.sha256==other.sha256
            &&self.bytes==other.bytes
            &&Arc::ptr_eq(&self.root,&other.root)
    }
}
#[derive(Default)]
pub struct FileTransferReviewQueue{
    pending:Mutex<HashMap<String,PinnedFileTransfer>>,
}
impl FileTransferReviewQueue{
    pub fn stage(&self,chat:&str,id:&str,source:&Path,operation:&str)
        ->Result<FileTransferReviewView,String>{
        if chat.is_empty()||!safe_id(id)||!matches!(operation,"copy"|"move"){
            return Err("Invalid file operation".into())
        }
        let (source_name,root,bytes,sha256)=native_source(source)?;
        let target_name=generated_target(&source_name,operation,id)?;
        if root.symlink_metadata(&target_name).is_ok(){
            return Err("Generated target already exists".into())
        }
        let target=source.parent().ok_or("Source has no parent")?.join(&target_name);
        let pending=PinnedFileTransfer{
            chat_id:chat.into(),review_id:id.into(),source_name,target_name,
            source_display:source.to_string_lossy().into_owned(),
            destination_display:target.to_string_lossy().into_owned(),
            operation:operation.into(),bytes,sha256,root,
        };
        let mut queue=self.pending.lock().map_err(|_|"File review unavailable")?;
        if queue.len()>=MAX_PENDING{return Err("Too many pending file operations".into())}
        if queue.contains_key(id){return Err("Duplicate file review".into())}
        let view=pending.view();
        queue.insert(id.into(),pending);
        Ok(view)
    }
    pub fn inspect(&self,chat:&str,id:&str)->Result<PinnedFileTransfer,String>{
        let queue=self.pending.lock().map_err(|_|"File review unavailable")?;
        let item=queue.get(id).ok_or("File review expired")?;
        if item.chat_id!=chat{return Err("Wrong file review conversation".into())}
        Ok(item.clone())
    }
    pub fn consume(&self,chat:&str,id:&str)->Result<PinnedFileTransfer,String>{
        let mut queue=self.pending.lock().map_err(|_|"File review unavailable")?;
        let item=queue.get(id).ok_or("File review expired")?;
        if item.chat_id!=chat{return Err("Wrong file review conversation".into())}
        queue.remove(id).ok_or("File review already consumed".into())
    }
    pub fn deny(&self,chat:&str,id:&str)->Result<(),String>{
        let _=self.consume(chat,id)?; Ok(())
    }
}
pub async fn execute_approved_transfer(
    chats:&ChatStore,effects:&EffectJournal,chat:&str,
    user_turn:&str,effect_id:&str,
    reviewed:&PinnedFileTransfer,
)->Result<FileTransferReceiptView,String>{
    chats.append_user_with_turn_id(chat,user_turn,
        &format!("Approved {} from {} to NEW file {}. {} bytes. SHA-256 {}. One-use grant.",
            reviewed.operation,reviewed.source_display,
            reviewed.destination_display,reviewed.bytes,reviewed.sha256))?;
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_file_transfer_tools(&registry,&mut catalog,Arc::clone(&reviewed.root))
        .map_err(|_|"File transfer registration rejected")?;
    let tool=if reviewed.operation=="copy"{COPY_ID}else{MOVE_ID};
    let authority=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task_id=format!("file-transfer-{user_turn}");
    let scope=format!("file-transfer-scope-{user_turn}");
    authority.open_user_task(TaskAuthority{
        conversation_id:chat.into(),user_turn_id:user_turn.into(),
        task_id:task_id.clone(),execution_scope_id:scope.clone(),
        capabilities:vec![AuthorizedCapability{
            tool_id:tool.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },chats).map_err(|_|"File transfer user-turn admission failed")?;
    policy.grant(CapabilityGrant{
        tool_id:tool.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:scope.clone(),expires_at:None,
    }).map_err(|_|"File transfer effect authorization failed")?;
    let fence=ExactFileTransferPolicy{
        host_policy:&policy,source_root:&reviewed.root,
        tool_id:tool,source_name:&reviewed.source_name,
        target_name:&reviewed.target_name,
        source_sha256:&reviewed.sha256,
    };
    let output=authority.dispatch_bound_durable(
        &registry,effects,ToolInvocation{
            call_id:effect_id.into(),tool_id:tool.into(),
            workspace_id:Some(scope),
            input:ProtoPayload{
                type_url:INPUT.into(),
                bytes:proto::FileTransferRequest{
                    source_name:reviewed.source_name.clone(),
                    target_name:reviewed.target_name.clone(),
                    expected_source_sha256:reviewed.sha256.clone(),
                }.encode_to_vec(),
            },
        },BoundCallOrigin{
            conversation_id:chat.into(),user_turn_id:user_turn.into(),task_id,
        },&fence,
    ).await.map_err(|e|format!("File operation failed or needs manual reconciliation: {e:?}"))?;
    if output.payload.type_url!=OUTPUT{
        return Err("File operation returned unexpected receipt".into())
    }
    let result=proto::FileTransferResult::decode(output.payload.bytes.as_slice())
        .map_err(|_|"File operation receipt invalid")?;
    if result.sha256!=reviewed.sha256||
        result.source_name!=reviewed.source_name||
        result.target_name!=reviewed.target_name||
        result.bytes_written!=reviewed.bytes||
        result.source_removed!=(reviewed.operation=="move"){
        return Err("Published file differs from approved operation; reconcile manually".into())
    }
    // Rehash the actual new file after publication. A race or crash must
    // become uncertain, never a silent successful second attempt.
    let (length,actual)=source_digest(&reviewed.root,&reviewed.target_name)
        .map_err(|_|"Cannot verify created target; manual reconciliation required")?;
    if length!=reviewed.bytes||actual!=reviewed.sha256{
        return Err("Created target content differs from approved file; reconcile".into())
    }
    if reviewed.operation=="move"&&
        reviewed.root.symlink_metadata(&reviewed.source_name).is_ok(){
        return Err("Move source still present after publication; reconcile manually".into())
    }
    Ok(FileTransferReceiptView{
        operation:reviewed.operation.clone(),
        target_name:result.target_name,
        exact_destination:reviewed.destination_display.clone(),
        bytes:result.bytes_written,sha256:result.sha256,
        source_removed:result.source_removed,
        effect_call_id:effect_id.into(),
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn native_selected_copy_move_review_never_mutates_before_approval(){
        let root=std::env::temp_dir().join(format!(
            "lu-transfer-desktop-{}",crate::next_id("test-")
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("original.txt"),b"original").unwrap();
        let queue=FileTransferReviewQueue::default();
        for kind in ["copy","move"]{
            let id=crate::next_id("review-");
            let preview=queue.stage("chat-one",&id,&root.join("original.txt"),kind).unwrap();
            assert_eq!(preview.operation,kind);
            assert_eq!(preview.bytes,8);
            assert!(!root.join(&preview.target_name).exists());
            assert!(queue.inspect("wrong-chat",&id).is_err());
            queue.deny("chat-one",&id).unwrap();
            assert!(root.join("original.txt").exists());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
