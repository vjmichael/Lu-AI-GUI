//! Native-picked, journaled reversible same-directory trash and restore.
//! User reviews the exact file/hash and approves in a separate native modal.
//! Host custody persists before effects, and both success and restoration are
//! verified from the independent EffectJournal; no blind retry on uncertainty.
use std::{collections::HashMap,fs,path::Path,sync::{Arc,Mutex}};
use cap_std::{ambient_authority,fs::Dir};
use prost::Message;
use serde::Serialize;
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority},
    journal::EffectJournal,model::ModelToolCatalog,
    native_file_transfer::{source_digest,TrashReceipt,MAX_TRANSFER_BYTES},
    native_file_trash_tool::{register_trash_restore_tools,ExactTrashPolicy,ExactRestorePolicy,
        TRASH_ID,RESTORE_ID,TRASH_INPUT,TRASH_OUTPUT,RESTORE_INPUT,RESTORE_OUTPUT},
    native_trash_custody::{CustodyRecord,CustodyState,TrashCustodyLedger},
    policy::{CapabilityGrant,PolicyEngine},registry::NativeToolRegistry,wire::proto,
};
use crate::chat_store::ChatStore;
const MAX_PENDING:usize=8;
fn safe_name(s:&str)->bool{!s.is_empty()&&s.len()<=240&&s!="."&&s!=".."
    &&!s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
    &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))}
fn id_ok(s:&str)->bool{(16..=128).contains(&s.len())&&s.bytes().all(|b|
    b.is_ascii_alphanumeric()||b==b'-'||b==b'_')}
fn absent(root:&Dir,name:&str)->Result<bool,String>{
    match root.symlink_metadata(name){
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(true),
        Ok(_)=>Ok(false),
        Err(_)=>Err("Cannot safely determine whether file name is occupied".into()),
    }
}
fn pinned_folder(path:&Path)->Result<Arc<Dir>,String>{
    let meta=fs::symlink_metadata(path).map_err(|_|"Parent unavailable")?;
    if !meta.is_dir()||meta.file_type().is_symlink(){return Err("Linked parent rejected".into())}
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return Err("Reparse parent rejected".into())}}
    Ok(Arc::new(Dir::open_ambient_dir(path,ambient_authority())
        .map_err(|_|"Selected parent inaccessible")?))
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TrashPreview{pub review_id:String,pub chat_id:String,pub source:String,
    pub stored_name:String,pub bytes:u64,pub sha256:String}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct RestorePreview{pub review_id:String,pub chat_id:String,pub custody_id:String,
    pub original:String,pub trash_entry:String,pub bytes:u64,pub sha256:String}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct CustodyView{pub custody_id:String,pub original:String,pub bytes:u64,
    pub sha256:String,pub state:String,pub observation:String,pub restorable:bool,
    pub trash_effect_id:String,pub restore_effect_id:Option<String>}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct CustodyOutcome{pub custody_id:String,pub original:String,
    pub state:String,pub bytes:u64,pub sha256:String,pub effect_call_id:String}
#[derive(Clone)]
pub struct PinnedTrash{
    chat:String,id:String,effect_id:String,name:String,parent:String,root:Arc<Dir>,
    bytes:u64,sha:String,
}
impl PinnedTrash{
    pub fn view(&self)->TrashPreview{TrashPreview{
        review_id:self.id.clone(),chat_id:self.chat.clone(),
        source:Path::new(&self.parent).join(&self.name).to_string_lossy().into_owned(),
        stored_name:format!("lu-trash-{}",self.effect_id),
        bytes:self.bytes,sha256:self.sha.clone(),
    }}
    pub fn same(&self,other:&Self)->bool{self.chat==other.chat&&self.id==other.id
        &&self.effect_id==other.effect_id&&self.name==other.name&&self.parent==other.parent
        &&self.bytes==other.bytes&&self.sha==other.sha&&Arc::ptr_eq(&self.root,&other.root)}
}
#[derive(Default)]
pub struct TrashQueue{pending:Mutex<HashMap<String,PinnedTrash>>}
impl TrashQueue{
    pub fn stage(&self,chat:&str,id:&str,effect_id:&str,file:&Path)->Result<TrashPreview,String>{
        if !id_ok(chat)||!id_ok(id)||!id_ok(effect_id){return Err("Invalid review identity".into())}
        let meta=fs::symlink_metadata(file).map_err(|_|"Source missing")?;
        if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>MAX_TRANSFER_BYTES{
            return Err("Only regular files under 32 MiB may be trashed".into())
        }
        #[cfg(windows)]{use std::os::windows::fs::MetadataExt;
            if meta.file_attributes()&0x400!=0{return Err("Reparse file refused".into())}}
        let name=file.file_name().and_then(|s|s.to_str()).ok_or("Invalid filename")?;
        if !safe_name(name){return Err("Unsafe filename".into())}
        let parent=file.parent().ok_or("Parent unavailable")?;
        let root=pinned_folder(parent)?;
        let (bytes,sha)=source_digest(&root,name).map_err(|_|"Source hash unavailable")?;
        if root.symlink_metadata(format!("lu-trash-{effect_id}")).is_ok(){
            return Err("Generated trash destination exists".into())
        }
        let item=PinnedTrash{chat:chat.into(),id:id.into(),effect_id:effect_id.into(),
            name:name.into(),parent:parent.to_string_lossy().into_owned(),root,bytes,sha};
        let mut pending=self.pending.lock().map_err(|_|"Trash queue unavailable")?;
        if pending.len()>=MAX_PENDING||pending.contains_key(id){return Err("Review queue full".into())}
        let view=item.view();pending.insert(id.into(),item);Ok(view)
    }
    pub fn inspect(&self,chat:&str,id:&str)->Result<PinnedTrash,String>{
        let guard=self.pending.lock().map_err(|_|"Trash queue unavailable")?;
        let item=guard.get(id).ok_or("Trash review expired")?;
        if item.chat!=chat{return Err("Review belongs to another conversation".into())}
        Ok(item.clone())
    }
    pub fn take(&self,chat:&str,id:&str)->Result<PinnedTrash,String>{
        let mut guard=self.pending.lock().map_err(|_|"Trash queue unavailable")?;
        if guard.get(id).is_none_or(|x|x.chat!=chat){return Err("Trash review expired".into())}
        guard.remove(id).ok_or("Already consumed".into())
    }
}
#[derive(Clone)]
pub struct PinnedRestore{
    chat:String,id:String,effect_id:String,
    record:CustodyRecord,receipt:TrashReceipt,root:Arc<Dir>,
}
impl PinnedRestore{
    pub fn view(&self)->RestorePreview{RestorePreview{
        review_id:self.id.clone(),chat_id:self.chat.clone(),custody_id:self.record.id.clone(),
        original:Path::new(&self.record.parent).join(&self.record.original)
            .to_string_lossy().into_owned(),
        trash_entry:self.record.trash_entry.clone(),
        bytes:self.record.bytes,sha256:self.record.sha256.clone(),
    }}
    pub fn same(&self,other:&Self)->bool{self.chat==other.chat&&self.id==other.id
        &&self.effect_id==other.effect_id&&self.record==other.record
        &&self.receipt==other.receipt&&Arc::ptr_eq(&self.root,&other.root)}
}
#[derive(Default)]
pub struct RestoreQueue{pending:Mutex<HashMap<String,PinnedRestore>>}
impl RestoreQueue{
    pub fn stage(&self,chat:&str,id:&str,effect_id:&str,custody_id:&str,
        ledger:&TrashCustodyLedger,effects:&EffectJournal)->Result<RestorePreview,String>{
        if !id_ok(chat)||!id_ok(id)||!id_ok(effect_id){return Err("Bad restore identity".into())}
        let (record,receipt)=ledger.validated_trash_receipt(custody_id,chat,effects)
            .map_err(|_|"No verified recoverable custody receipt")?;
        let root=pinned_folder(Path::new(&record.parent))?;
        let (bytes,sha)=source_digest(&root,&record.trash_entry)
            .map_err(|_|"Trashed file missing or unsafe")?;
        if bytes!=record.bytes||sha!=record.sha256||
            !absent(&root,&record.original)?{
            return Err("Original name occupied or trashed contents changed".into())
        }
        let item=PinnedRestore{chat:chat.into(),id:id.into(),effect_id:effect_id.into(),
            record,receipt,root};
        let mut guard=self.pending.lock().map_err(|_|"Restore queue unavailable")?;
        if guard.len()>=MAX_PENDING||guard.contains_key(id){return Err("Restore queue full".into())}
        let view=item.view();guard.insert(id.into(),item);Ok(view)
    }
    pub fn inspect(&self,chat:&str,id:&str)->Result<PinnedRestore,String>{
        let guard=self.pending.lock().map_err(|_|"Restore queue unavailable")?;
        let item=guard.get(id).ok_or("Restore review expired")?;
        if item.chat!=chat{return Err("Wrong conversation".into())}
        Ok(item.clone())
    }
    pub fn take(&self,chat:&str,id:&str)->Result<PinnedRestore,String>{
        let mut guard=self.pending.lock().map_err(|_|"Restore queue unavailable")?;
        if guard.get(id).is_none_or(|x|x.chat!=chat){return Err("Restore review expired".into())}
        guard.remove(id).ok_or("Already consumed".into())
    }
}
async fn dispatch(
    chats:&ChatStore,effects:&EffectJournal,chat:&str,user_turn:&str,
    call_id:&str,root:Arc<Dir>,mode:&str,receipt:Option<TrashReceipt>,
    name:&str,sha:&str,
)->Result<ProtoPayload,String>{
    let registry=NativeToolRegistry::new();let mut catalog=ModelToolCatalog::new();
    register_trash_restore_tools(&registry,&mut catalog,Arc::clone(&root),root.clone(),
        receipt.clone()).map_err(|e|format!("Cannot register reversible tool: {e}"))?;
    let tool=if mode=="trash"{TRASH_ID}else{RESTORE_ID};
    let auth=ConversationAuthority::new();let policy=PolicyEngine::new();
    let task_id=format!("file-{mode}-{user_turn}");
    let scope=format!("file-{mode}-scope-{user_turn}");
    auth.open_user_task(TaskAuthority{
        conversation_id:chat.into(),user_turn_id:user_turn.into(),task_id:task_id.clone(),
        execution_scope_id:scope.clone(),capabilities:vec![AuthorizedCapability{
            tool_id:tool.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },chats).map_err(|_|"Persisted human turn not admitted")?;
    policy.grant(CapabilityGrant{tool_id:tool.into(),
        effect:ToolEffect::WriteWorkspace,scope_id:scope.clone(),expires_at:None
    }).map_err(|_|"Native write grant rejected")?;
    let (type_url,input)=if mode=="trash"{
        (TRASH_INPUT,proto::FileTrashRequest{
            source_name:name.into(),expected_sha256:sha.into(),
        }.encode_to_vec())
    }else{
        let r=receipt.as_ref().ok_or("No verified restore receipt")?;
        (RESTORE_INPUT,proto::FileRestoreRequest{
            trusted_receipt_id:r.trash_entry.clone(),
        }.encode_to_vec())
    };
    let call=ToolInvocation{call_id:call_id.into(),tool_id:tool.into(),
        workspace_id:Some(scope),input:ProtoPayload{
            type_url:type_url.into(),bytes:input,
        }};
    let origin=BoundCallOrigin{conversation_id:chat.into(),
        user_turn_id:user_turn.into(),task_id};
    let reply=if mode=="trash"{
        auth.dispatch_bound_durable(&registry,effects,call,origin,
            &ExactTrashPolicy{host_policy:&policy,trusted_source:&root,
                source_name:name,source_sha256:sha}).await
    }else{
        let receipt=receipt.as_ref().ok_or("No restore custody")?;
        auth.dispatch_bound_durable(&registry,effects,call,origin,
            &ExactRestorePolicy{host_policy:&policy,
                trusted_receipt_id:&receipt.trash_entry,receipt}).await
    }.map_err(|e|format!("Native file {mode} failed or uncertain: {e:?}"))?;
    Ok(reply.payload)
}
pub async fn apply_trash(
    chats:&ChatStore,effects:&EffectJournal,ledger:&TrashCustodyLedger,
    chat:&str,user_turn:&str,review:&PinnedTrash,
)->Result<CustodyOutcome,String>{
    chats.append_user_with_turn_id(chat,user_turn,&format!(
        "Approved reversible file trash: {} ({} bytes, SHA-256 {}). Lu will retain a new custody entry in the SAME folder, not in the OS recycle bin.",
        review.view().source,review.bytes,review.sha))?;
    let record=CustodyRecord{id:review.id.clone(),chat_id:chat.into(),
        parent:review.parent.clone(),original:review.name.clone(),
        trash_entry:format!("lu-trash-{}",review.effect_id),
        bytes:review.bytes,sha256:review.sha.clone(),trash_effect_id:review.effect_id.clone(),
        restore_effect_id:None,state:CustodyState::Pending};
    ledger.stage(&record).map_err(|e|format!("Trash intent not durable: {e:?}"))?;
    let result=async{
        let output=dispatch(chats,effects,chat,user_turn,&review.effect_id,
            Arc::clone(&review.root),"trash",None,&review.name,&review.sha).await?;
        if output.type_url!=TRASH_OUTPUT{return Err("Unexpected trash receipt".into())}
        let receipt=proto::FileTrashResult::decode(output.bytes.as_slice())
            .map_err(|_|"Malformed trash receipt")?;
        if receipt.original_name!=record.original||receipt.trash_entry!=record.trash_entry
            ||receipt.bytes!=record.bytes||receipt.sha256!=record.sha256{
            return Err("Trash receipt differs from approved source".into())
        }
        let (bytes,sha)=source_digest(&review.root,&record.trash_entry)
            .map_err(|_|"Published trash entry cannot be verified")?;
        if bytes!=record.bytes||sha!=record.sha256||
            !absent(&review.root,&record.original)?{
            return Err("Physical trash state does not match approved receipt".into())
        }
        ledger.confirm_trash(&record.id,chat,effects)
            .map_err(|e|format!("Journaled trash receipt invalid: {e:?}"))?;
        Ok(CustodyOutcome{custody_id:record.id.clone(),original:record.original.clone(),
            state:"TRASHED".into(),bytes,sha256:sha,effect_call_id:review.effect_id.clone()})
    }.await;
    if result.is_err(){let _=ledger.mark_uncertain(&record.id,chat);}
    result
}
pub async fn apply_restore(
    chats:&ChatStore,effects:&EffectJournal,ledger:&TrashCustodyLedger,
    chat:&str,user_turn:&str,review:&PinnedRestore,
)->Result<CustodyOutcome,String>{
    let (record,receipt)=ledger.validated_trash_receipt(&review.record.id,chat,effects)
        .map_err(|_|"Durable receipt no longer valid")?;
    if record!=review.record||receipt!=review.receipt{
        return Err("Custody changed during review".into())
    }
    chats.append_user_with_turn_id(chat,user_turn,&format!(
        "Approved separately restoring custody {} to {} (SHA-256 {}).",
        record.id,review.view().original,record.sha256))?;
    ledger.begin_restore(&record.id,chat,&review.effect_id)
        .map_err(|e|format!("Restore intent not durable: {e:?}"))?;
    let result=async{
        let output=dispatch(chats,effects,chat,user_turn,&review.effect_id,
            Arc::clone(&review.root),"restore",Some(receipt),
            &record.original,&record.sha256).await?;
        if output.type_url!=RESTORE_OUTPUT{return Err("Unexpected restore receipt".into())}
        let reply=proto::FileRestoreResult::decode(output.bytes.as_slice())
            .map_err(|_|"Malformed restore receipt")?;
        if reply.restored_name!=record.original||reply.bytes!=record.bytes
            ||reply.sha256!=record.sha256{return Err("Restore receipt differs from custody".into())}
        let (bytes,sha)=source_digest(&review.root,&record.original)
            .map_err(|_|"Restored file cannot be rehashed")?;
        if bytes!=record.bytes||sha!=record.sha256||
            !absent(&review.root,&record.trash_entry)?{
            return Err("Physical restored file differs from custody".into())
        }
        ledger.confirm_restore(&record.id,chat,effects)
            .map_err(|_|"Restore effect journal could not be verified")?;
        Ok(CustodyOutcome{custody_id:record.id.clone(),original:record.original.clone(),
            state:"RESTORED".into(),bytes,sha256:sha,effect_call_id:review.effect_id.clone()})
    }.await;
    if result.is_err(){let _=ledger.mark_uncertain(&record.id,chat);}
    result
}
pub fn history(chats:&ChatStore,ledger:&TrashCustodyLedger,
    effects:&EffectJournal,chat:&str)->Result<Vec<CustodyView>,String>{
    chats.history(chat)?;
    let records=ledger.list_chat(chat).map_err(|_|"Cannot read custody history")?;
    let mut out=Vec::new();
    for record in records{
        let mut observation=if record.state==CustodyState::Uncertain{
            "MANUAL_RECONCILIATION_REQUIRED".to_owned()
        }else{"NOT_VERIFIED".to_owned()};
        let mut restorable=false;
        if record.state==CustodyState::Trashed{
            if ledger.validated_trash_receipt(&record.id,chat,effects).is_ok(){
                if let Ok(root)=pinned_folder(Path::new(&record.parent)){
                    if let Ok((bytes,sha))=source_digest(&root,&record.trash_entry){
                        if bytes==record.bytes&&sha==record.sha256&&
                            absent(&root,&record.original).unwrap_or(false){
                            restorable=true;observation="TRASH_BYTES_MATCH_JOURNAL".into();
                        }else{observation="TRASH_CHANGED_OR_ORIGINAL_PRESENT".into();}
                    }
                }
            }
        }else if record.state==CustodyState::Restored{
            if let Ok(root)=pinned_folder(Path::new(&record.parent)){
                if let Ok((bytes,sha))=source_digest(&root,&record.original){
                    if bytes==record.bytes&&sha==record.sha256{
                        observation="RESTORED_BYTES_VERIFIED".into();
                    }else{observation="RESTORED_FILE_CHANGED".into();}
                }
            }
        }
        out.push(CustodyView{custody_id:record.id,original:record.original,
            bytes:record.bytes,sha256:record.sha256,
            state:record.state.as_str().into(),observation,restorable,
            trash_effect_id:record.trash_effect_id,
            restore_effect_id:record.restore_effect_id});
    }
    Ok(out)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn reviewed_trash_keeps_source_until_explicit_native_approval(){
        let root=std::env::temp_dir().join(crate::next_id("lu-trash-ui-"));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("notes.txt"),b"important").unwrap();
        let queue=TrashQueue::default();
        let chat="c-0123456789abcdef0123456789abcdef";
        let id=crate::next_id("ct-");let effect=crate::next_id("g-");
        let view=queue.stage(chat,&id,&effect,&root.join("notes.txt")).unwrap();
        assert_eq!(view.bytes,9);
        assert!(root.join("notes.txt").exists());
        assert!(!root.join(&view.stored_name).exists());
        assert!(queue.inspect("wrong-chat",&id).is_err());
        queue.take(chat,&id).unwrap();
        assert!(queue.inspect(chat,&id).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
