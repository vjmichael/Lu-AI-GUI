//! Wave 2 native reversible trash and restore.
 //! A model/WebView cannot choose filesystem paths or invent restore custody.
 //! Native OS picker + exact SHA + final native confirmation -> one fresh
 //! persisted human turn -> native ToolAuthorizer -> durable EffectJournal.
 //! The trusted host writes a **pre-effect** custody intent to app data, so a
 //! crash between link/unlink and publication is observable after restart.
 //! Trash is Lu-owned, not the platform recycle bin. Cross-volume hard-links
 //! are refused rather than replacing atomicity with a silent copy/delete.
use std::{
    collections::{HashMap,BTreeMap},
    fs::{self,File,OpenOptions},
    io::{Read,Write},path::{Path,PathBuf},
    sync::{Arc,Mutex},
};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
    },
    journal::{EffectJournal,EffectState},
    model::ModelToolCatalog,
    native_file_transfer::{source_digest,TrashReceipt},
    native_file_trash_tool::{
        register_trash_restore_tools,ExactTrashPolicy,ExactRestorePolicy,
        TRASH_ID,RESTORE_ID,TRASH_INPUT,TRASH_OUTPUT,RESTORE_INPUT,RESTORE_OUTPUT,
    },
    policy::{CapabilityGrant,PolicyEngine},
    registry::NativeToolRegistry,
    wire::{self,proto},
};
use prost::Message;
use serde::{Deserialize,Serialize};
use crate::chat_store::ChatStore;

const MAX_PENDING:usize=8;
const MAX_RECORDS:usize=1024;
const MAX_RECORD_BYTES:u64=8192;
fn opaque(s:&str)->bool{
    (16..=128).contains(&s.len()) &&
    s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn flat(s:&str)->bool{
    !s.is_empty()&&s.len()<=240&&s!="."&&s!=".."
        &&!s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn digest_ok(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn pinned_dir(path:&Path)->Result<Arc<Dir>,String>{
    let meta=fs::symlink_metadata(path).map_err(|_|"Selected folder unavailable")?;
    if !meta.is_dir()||meta.file_type().is_symlink(){
        return Err("Selected directory is not a real folder".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err("Windows reparse-point folder refused".into())
        }
    }
    Ok(Arc::new(Dir::open_ambient_dir(path,ambient_authority())
        .map_err(|_|"Cannot pin selected folder")?))
}
fn selected_file(path:&Path)->Result<(String,PathBuf,Arc<Dir>,u64,String),String>{
    let name=path.file_name().and_then(|s|s.to_str())
        .ok_or("Selected source filename unavailable")?;
    if !flat(name){return Err("Unsupported source filename".into())}
    let stat=fs::symlink_metadata(path).map_err(|_|"Source file unavailable")?;
    if !stat.is_file()||stat.file_type().is_symlink(){
        return Err("Only a real regular file can be trashed".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if stat.file_attributes()&0x400!=0{
            return Err("Windows reparse point cannot be trashed".into())
        }
    }
    let parent=path.parent().ok_or("Selected file folder missing")?
        .canonicalize().map_err(|_|"Cannot canonicalize selected folder")?;
    let root=pinned_dir(&parent)?;
    let (size,sha)=source_digest(&root,name)
        .map_err(|_|"Source file exceeds limit or cannot be hashed")?;
    Ok((name.into(),parent,root,size,sha))
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TrashReviewView{
    pub review_id:String,pub chat_id:String,
    pub exact_source:String,pub source_name:String,
    pub size:u64,pub sha256:String,pub reversible:bool,
}
#[derive(Clone)]
pub struct PinnedTrashReview{
    chat_id:String,review_id:String,
    source_name:String,source_parent:PathBuf,source_root:Arc<Dir>,
    size:u64,sha256:String,
}
impl PinnedTrashReview{
    pub fn view(&self)->TrashReviewView{
        TrashReviewView{
            review_id:self.review_id.clone(),chat_id:self.chat_id.clone(),
            exact_source:self.source_parent.join(&self.source_name)
                .to_string_lossy().into_owned(),
            source_name:self.source_name.clone(),size:self.size,
            sha256:self.sha256.clone(),reversible:true,
        }
    }
    pub fn same_as(&self,other:&Self)->bool{
        self.chat_id==other.chat_id&&self.review_id==other.review_id
            &&self.source_name==other.source_name
            &&self.source_parent==other.source_parent
            &&self.size==other.size&&self.sha256==other.sha256
            &&Arc::ptr_eq(&self.source_root,&other.source_root)
    }
}
#[derive(Default)]
pub struct TrashReviewQueue{
    entries:Mutex<HashMap<String,PinnedTrashReview>>,
}
impl TrashReviewQueue{
    pub fn stage(&self,chat:&str,id:&str,path:&Path)->Result<TrashReviewView,String>{
        if chat.is_empty()||!opaque(id){return Err("Invalid trash review ID".into())}
        let (source_name,source_parent,source_root,size,sha256)=selected_file(path)?;
        let review=PinnedTrashReview{
            chat_id:chat.into(),review_id:id.into(),
            source_name,source_parent,source_root,size,sha256,
        };
        let mut entries=self.entries.lock().map_err(|_|"Trash review unavailable")?;
        if entries.len()>=MAX_PENDING{return Err("Too many pending trash reviews".into())}
        if entries.contains_key(id){return Err("Duplicate trash review".into())}
        let view=review.view();
        entries.insert(id.into(),review);
        Ok(view)
    }
    pub fn inspect(&self,chat:&str,id:&str)->Result<PinnedTrashReview,String>{
        let entries=self.entries.lock().map_err(|_|"Trash review unavailable")?;
        let selected=entries.get(id).ok_or("Trash review expired")?;
        if selected.chat_id!=chat{return Err("Wrong trash review conversation".into())}
        Ok(selected.clone())
    }
    pub fn consume(&self,chat:&str,id:&str)->Result<PinnedTrashReview,String>{
        let mut entries=self.entries.lock().map_err(|_|"Trash review unavailable")?;
        let selected=entries.get(id).ok_or("Trash review expired")?;
        if selected.chat_id!=chat{return Err("Wrong trash review conversation".into())}
        entries.remove(id).ok_or("Trash review already consumed".into())
    }
    pub fn deny(&self,chat:&str,id:&str)->Result<(),String>{
        let _=self.consume(chat,id)?;Ok(())
    }
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CustodyIntent{
    schema:u32,
    chat_id:String,
    effect_call_id:String,
    pub(crate) source_parent:PathBuf,
    source_name:String,
    expected_size:u64,
    sha256:String,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreIntent{
    schema:u32,
    chat_id:String,
    source_effect_id:String,
    restore_effect_id:String,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TrashCustodyView{
    pub effect_call_id:String,
    pub file_name:String,
    pub exact_original:String,
    pub bytes:u64,pub sha256:String,
    pub status:String,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TrashOutcomeView{
    pub effect_call_id:String,
    pub original_name:String,
    pub original_display:String,
    pub trash_entry:String,
    pub bytes:u64,pub sha256:String,
    pub state:String,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct RestoreOutcomeView{
    pub effect_call_id:String,pub file_name:String,
    pub exact_target:String,pub bytes:u64,pub sha256:String,
}
pub struct TrashCustodyStore{
    intents_dir:PathBuf,
    trash_path:PathBuf,
    trash:Arc<Dir>,
    /// All custody index and restore intent transitions are serialized.
    guard:Mutex<()>,
}
impl TrashCustodyStore{
    pub fn new(app_data:&Path)->Result<Self,String>{
        let intents=app_data.join("wave2-trash-custody");
        let trash_path=app_data.join("wave2-reversible-trash");
        fs::create_dir_all(&intents).map_err(|e|format!("Cannot create custody store: {e}"))?;
        fs::create_dir_all(&trash_path).map_err(|e|format!("Cannot create trash store: {e}"))?;
        let _=pinned_dir(&intents)?;
        let trash=pinned_dir(&trash_path)?;
        Ok(Self{intents_dir:intents,trash_path,trash,guard:Mutex::new(())})
    }
    fn path(&self,kind:&str,effect:&str)->Result<PathBuf,String>{
        if !matches!(kind,"trash"|"restore")||!opaque(effect){
            return Err("Invalid effect ID".into())
        }
        Ok(self.intents_dir.join(format!("{kind}-{effect}.json")))
    }
    fn store_once<T:Serialize>(&self,kind:&str,effect:&str,record:&T)
        ->Result<(),String>{
        let _g=self.guard.lock().map_err(|_|"Custody store unavailable")?;
        let target=self.path(kind,effect)?;
        let bytes=serde_json::to_vec(record)
            .map_err(|_|"Cannot encode custody record")?;
        if bytes.len()>MAX_RECORD_BYTES as usize{
            return Err("Custody record exceeds safety limit".into())
        }
        let mut options=OpenOptions::new();
        options.create_new(true).write(true);
        let mut file=options.open(&target)
            .map_err(|e|format!("Cannot durably record write intent: {e}"))?;
        file.write_all(&bytes)
            .map_err(|e|format!("Cannot complete intent: {e}"))?;
        file.sync_all().map_err(|e|format!("Cannot sync intent: {e}"))?;
        Ok(())
    }
    fn read_record<T:for<'de>Deserialize<'de>>(&self,kind:&str,id:&str)
        ->Result<T,String>{
        let path=self.path(kind,id)?;
        let metadata=fs::symlink_metadata(&path).map_err(|_|"Custody receipt missing")?;
        if !metadata.is_file()||metadata.file_type().is_symlink()
            ||metadata.len()>MAX_RECORD_BYTES{
            return Err("Invalid custody record".into())
        }
        let mut data=Vec::new();
        File::open(path).map_err(|_|"Custody record inaccessible")?
            .take(MAX_RECORD_BYTES+1).read_to_end(&mut data)
            .map_err(|_|"Custody read failed")?;
        serde_json::from_slice(&data)
            .map_err(|_|"Custody record is not recoverable")
    }
    pub fn before_trash(&self,chat:&str,effect:&str,review:&PinnedTrashReview)
        ->Result<(),String>{
        if chat!=review.chat_id||!opaque(effect){return Err("Wrong source scope".into())}
        let intent=CustodyIntent{
            schema:1,chat_id:chat.into(),effect_call_id:effect.into(),
            source_parent:review.source_parent.clone(),
            source_name:review.source_name.clone(),expected_size:review.size,
            sha256:review.sha256.clone(),
        };
        self.store_once("trash",effect,&intent)
    }
    fn receipt_from_journal(
        &self,effects:&EffectJournal,intent:&CustodyIntent,
    )->Result<Option<TrashReceipt>,String>{
        let journal=effects.get(&intent.effect_call_id)
            .map_err(|_|"Cannot inspect durable trash effect")?;
        let Some(journal)=journal else{return Ok(None)};
        if journal.tool_id!=TRASH_ID{return Err("Custody effect belongs to wrong tool".into())}
        if journal.state!=EffectState::Applied{return Ok(None)}
        let bytes=journal.receipt.as_ref().ok_or("Applied trash missing receipt")?;
        let parsed=wire::decode_reply(bytes)
            .map_err(|_|"Stored trash reply invalid")?;
        if parsed.call_id!=intent.effect_call_id||parsed.output_type_url!=TRASH_OUTPUT{
            return Err("Stored trash receipt ID/type mismatch".into())
        }
        let Some(proto::tool_reply::Outcome::Output(output))=parsed.outcome else{
            return Err("Stored trash outcome is not applied output".into())
        };
        let result=proto::FileTrashResult::decode(output.as_slice())
            .map_err(|_|"Stored trash result malformed")?;
        let expected_id=format!("lu-trash-{}",intent.effect_call_id);
        if result.original_name!=intent.source_name ||
            result.trash_entry!=expected_id ||
            result.bytes!=intent.expected_size ||
            result.sha256!=intent.sha256{
            return Err("Trash receipt differs from approved source".into())
        }
        Ok(Some(TrashReceipt{
            original:result.original_name,trash_entry:result.trash_entry,
            bytes:result.bytes,sha256:result.sha256,
        }))
    }
    fn restore_intents(&self)->Result<Vec<RestoreIntent>,String>{
        let mut records=Vec::new();
        for entry in fs::read_dir(&self.intents_dir)
            .map_err(|_|"Cannot list restore intents")?{
            let entry=entry.map_err(|_|"Cannot scan restore custody")?;
            let name=entry.file_name();
            let Some(name)=name.to_str() else{continue};
            if !name.starts_with("restore-")||!name.ends_with(".json"){continue}
            if records.len()>=MAX_RECORDS{return Err("Too many restore records".into())}
            let id=&name[8..name.len()-5];
            let r:RestoreIntent=self.read_record("restore",id)?;
            if r.schema!=1||r.restore_effect_id!=id||!opaque(&r.source_effect_id){
                return Err("Restore record corrupted".into())
            }
            records.push(r);
        }
        Ok(records)
    }
    fn latest_restoration(&self,effect_id:&str)
        ->Result<Option<RestoreIntent>,String>{
        Ok(self.restore_intents()?.into_iter()
            .find(|r|r.source_effect_id==effect_id))
    }
    /// Status is read-only; a PREPARED/UNCERTAIN effect must NEVER be retried.
    pub fn list(&self,chat:&str,effects:&EffectJournal)
        ->Result<Vec<TrashCustodyView>,String>{
        let mut results=Vec::new();
        let restorations=self.restore_intents()?;
        for entry in fs::read_dir(&self.intents_dir)
            .map_err(|_|"Custody store unavailable")?{
            let entry=entry.map_err(|_|"Cannot list custody")?;
            let name=entry.file_name();
            let Some(name)=name.to_str() else{continue};
            if !name.starts_with("trash-")||!name.ends_with(".json"){continue}
            let id=&name[6..name.len()-5];
            if !opaque(id){continue}
            let intent:CustodyIntent=self.read_record("trash",id)?;
            if intent.schema!=1||intent.effect_call_id!=id||
                !flat(&intent.source_name)||!digest_ok(&intent.sha256){
                return Err("Unsafe stored trash intent".into())
            }
            if intent.chat_id!=chat{continue}
            let has_restore=restorations.iter()
                .any(|r|r.source_effect_id==id);
            let status=if has_restore{
                let restore=restorations.iter()
                    .find(|r|r.source_effect_id==id).unwrap();
                match effects.get(&restore.restore_effect_id)
                    .map_err(|_|"Restore journal unavailable")?{
                    Some(r) if r.tool_id==RESTORE_ID&&r.state==EffectState::Applied=>
                        "RESTORED",
                    _=>"RESTORE_NEEDS_REVIEW",
                }
            }else if self.receipt_from_journal(effects,&intent)?.is_some(){
                let live=self.trash.symlink_metadata(
                    &format!("lu-trash-{}",intent.effect_call_id));
                if live.is_ok(){"AVAILABLE"}else{"CUSTODY_NEEDS_REVIEW"}
            }else{
                "TRASH_NEEDS_REVIEW"
            };
            results.push(TrashCustodyView{
                effect_call_id:id.into(),file_name:intent.source_name.clone(),
                exact_original:intent.source_parent.join(&intent.source_name)
                    .to_string_lossy().into_owned(),
                bytes:intent.expected_size,sha256:intent.sha256,
                status:status.into(),
            });
        }
        results.sort_by(|a,b|a.effect_call_id.cmp(&b.effect_call_id));
        Ok(results)
    }
    pub fn restore_candidate(&self,chat:&str,id:&str,effects:&EffectJournal)
        ->Result<(CustodyIntent,TrashReceipt,Arc<Dir>),String>{
        let intent:CustodyIntent=self.read_record("trash",id)?;
        if intent.schema!=1||intent.chat_id!=chat||intent.effect_call_id!=id
            ||!flat(&intent.source_name)||!digest_ok(&intent.sha256){
            return Err("Restore request not part of this conversation".into())
        }
        if self.latest_restoration(id)?.is_some(){
            return Err("Restore was previously attempted; inspect durable recovery".into())
        }
        let receipt=self.receipt_from_journal(effects,&intent)?
            .ok_or("Original trash effect is not verified APPLIED")?;
        let original=pinned_dir(&intent.source_parent)?;
        if original.symlink_metadata(&intent.source_name).is_ok(){
            return Err("Restore target exists and must never be replaced".into())
        }
        let (length,sha)=source_digest(&self.trash,&receipt.trash_entry)
            .map_err(|_|"Trashed file no longer verifies")?;
        if length!=receipt.bytes||sha!=receipt.sha256{
            return Err("Trashed file content changed; restore refused".into())
        }
        Ok((intent,receipt,original))
    }
    pub fn before_restore(
        &self,chat:&str,source_id:&str,restore_effect_id:&str,
    )->Result<(),String>{
        if !opaque(restore_effect_id)||!opaque(source_id){
            return Err("Invalid restore identity".into())
        }
        if self.latest_restoration(source_id)?.is_some(){
            return Err("Restore already attempted; manual reconciliation required".into())
        }
        self.store_once("restore",restore_effect_id,&RestoreIntent{
            schema:1,chat_id:chat.into(),
            source_effect_id:source_id.into(),
            restore_effect_id:restore_effect_id.into(),
        })
    }
}
pub async fn execute_approved_trash(
    chats:&ChatStore,effects:&EffectJournal,store:&TrashCustodyStore,
    chat:&str,user_turn:&str,effect_id:&str,review:&PinnedTrashReview,
)->Result<TrashOutcomeView,String>{
    // Durable host-owned intent must be recorded BEFORE any journaled effect.
    store.before_trash(chat,effect_id,review)?;
    chats.append_user_with_turn_id(chat,user_turn,&format!(
        "Approved reversible trash of {} ({} bytes, SHA-256 {}). Lu custody receipt {}, single-use.",
        review.source_parent.join(&review.source_name).display(),
        review.size,review.sha256,effect_id,
    ))?;
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_trash_restore_tools(
        &registry,&mut catalog,Arc::clone(&review.source_root),
        Arc::clone(&store.trash),None
    ).map_err(|_|"Native reversible-trash tool registration failed")?;
    let authority=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task=format!("trash-{user_turn}");
    let scope=format!("trash-scope-{user_turn}");
    authority.open_user_task(TaskAuthority{
        conversation_id:chat.into(),user_turn_id:user_turn.into(),
        task_id:task.clone(),execution_scope_id:scope.clone(),
        capabilities:vec![AuthorizedCapability{
            tool_id:TRASH_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },chats).map_err(|_|"Trash user-turn admission failed")?;
    policy.grant(CapabilityGrant{
        tool_id:TRASH_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:scope.clone(),expires_at:None,
    }).map_err(|_|"Trash policy grant failed")?;
    let fence=ExactTrashPolicy{
        host_policy:&policy,trusted_source:&review.source_root,
        source_name:&review.source_name,source_sha256:&review.sha256,
    };
    let result=authority.dispatch_bound_durable(
        &registry,effects,ToolInvocation{
            call_id:effect_id.into(),tool_id:TRASH_ID.into(),
            workspace_id:Some(scope),
            input:ProtoPayload{
                type_url:TRASH_INPUT.into(),
                bytes:proto::FileTrashRequest{
                    source_name:review.source_name.clone(),
                    expected_sha256:review.sha256.clone(),
                }.encode_to_vec(),
            },
        },BoundCallOrigin{
            conversation_id:chat.into(),user_turn_id:user_turn.into(),task_id:task,
        },&fence,
    ).await.map_err(|e|format!("Trash effect failed or needs manual recovery: {e:?}"))?;
    if result.payload.type_url!=TRASH_OUTPUT{
        return Err("Trash receipt has wrong Protobuf output type".into())
    }
    let parsed=proto::FileTrashResult::decode(result.payload.bytes.as_slice())
        .map_err(|_|"Trash effect output corrupted")?;
    if parsed.original_name!=review.source_name||parsed.bytes!=review.size
        ||parsed.sha256!=review.sha256
        ||parsed.trash_entry!=format!("lu-trash-{effect_id}"){
        return Err("Trash result mismatched approved custody; review recovery".into())
    }
    if review.source_root.symlink_metadata(&review.source_name).is_ok(){
        return Err("Trashed original file still exists; manual recovery required".into())
    }
    // The journal-backed receipt is the source of truth for later restore.
    let intent:CustodyIntent=store.read_record("trash",effect_id)?;
    if store.receipt_from_journal(effects,&intent)?.is_none(){
        return Err("Cannot verify durable applied trash receipt".into())
    }
    Ok(TrashOutcomeView{
        effect_call_id:effect_id.into(),original_name:review.source_name.clone(),
        original_display:review.source_parent.join(&review.source_name)
            .to_string_lossy().into_owned(),
        trash_entry:parsed.trash_entry,
        bytes:parsed.bytes,sha256:parsed.sha256,state:"TRASHED".into(),
    })
}
pub async fn execute_approved_restore(
    chats:&ChatStore,effects:&EffectJournal,store:&TrashCustodyStore,
    chat:&str,user_turn:&str,effect_id:&str,original_effect_id:&str,
)->Result<RestoreOutcomeView,String>{
    let (intent,receipt,source)=store.restore_candidate(chat,original_effect_id,effects)?;
    store.before_restore(chat,original_effect_id,effect_id)?;
    chats.append_user_with_turn_id(chat,user_turn,&format!(
        "Approved restoring trusted Lu trash receipt {} to {}, {} bytes, exact SHA-256 {}. Single use.",
        receipt.trash_entry,intent.source_parent.join(&receipt.original).display(),
        receipt.bytes,receipt.sha256
    ))?;
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_trash_restore_tools(
        &registry,&mut catalog,Arc::clone(&source),
        Arc::clone(&store.trash),Some(receipt.clone())
    ).map_err(|_|"Native restore registration refused")?;
    let authority=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task=format!("restore-{user_turn}");
    let scope=format!("restore-scope-{user_turn}");
    authority.open_user_task(TaskAuthority{
        conversation_id:chat.into(),user_turn_id:user_turn.into(),
        task_id:task.clone(),execution_scope_id:scope.clone(),
        capabilities:vec![AuthorizedCapability{
            tool_id:RESTORE_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None,
    },chats).map_err(|_|"Restore turn authority failed")?;
    policy.grant(CapabilityGrant{
        tool_id:RESTORE_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:scope.clone(),expires_at:None,
    }).map_err(|_|"Restore policy refused")?;
    let output=authority.dispatch_bound_durable(
        &registry,effects,ToolInvocation{
            call_id:effect_id.into(),tool_id:RESTORE_ID.into(),
            workspace_id:Some(scope),
            input:ProtoPayload{
                type_url:RESTORE_INPUT.into(),
                bytes:proto::FileRestoreRequest{
                    trusted_receipt_id:receipt.trash_entry.clone(),
                }.encode_to_vec(),
            },
        },BoundCallOrigin{
            conversation_id:chat.into(),user_turn_id:user_turn.into(),task_id:task,
        },&ExactRestorePolicy{
            host_policy:&policy,trusted_receipt_id:&receipt.trash_entry,
            receipt:&receipt,
        },
    ).await.map_err(|e|format!("Restore effect failed or needs recovery: {e:?}"))?;
    if output.payload.type_url!=RESTORE_OUTPUT{
        return Err("Unexpected restore output receipt".into())
    }
    let result=proto::FileRestoreResult::decode(output.payload.bytes.as_slice())
        .map_err(|_|"Restore output unreadable")?;
    if result.restored_name!=receipt.original||
        result.bytes!=receipt.bytes||result.sha256!=receipt.sha256{
        return Err("Restored file differs from approved custody".into())
    }
    let (size,actual)=source_digest(&source,&result.restored_name)
        .map_err(|_|"Restored output content cannot be verified")?;
    if size!=receipt.bytes||actual!=receipt.sha256{
        return Err("Restored output bytes differ from trusted receipt".into())
    }
    Ok(RestoreOutcomeView{
        effect_call_id:effect_id.into(),file_name:result.restored_name,
        exact_target:intent.source_parent.join(&receipt.original)
            .to_string_lossy().into_owned(),
        bytes:result.bytes,sha256:result.sha256,
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn custody_intents_survive_restart_but_never_authorize_replay(){
        let root=std::env::temp_dir().join(format!(
            "lu-trash-desktop-test-{}",crate::next_id("t-")
        ));
        fs::create_dir_all(&root).unwrap();
        let source=root.join("original.txt");
        fs::write(&source,b"keep this").unwrap();
        let q=TrashReviewQueue::default();
        let id=crate::next_id("review-");
        let view=q.stage("chat-1",&id,&source).unwrap();
        assert_eq!(view.size,9);
        assert!(q.inspect("chat-2",&id).is_err());
        let pinned=q.inspect("chat-1",&id).unwrap();
        let custody=TrashCustodyStore::new(&root).unwrap();
        let effect=crate::next_id("g-");
        custody.before_trash("chat-1",&effect,&pinned).unwrap();
        assert!(custody.before_trash("chat-1",&effect,&pinned).is_err());
        let after_restart=TrashCustodyStore::new(&root).unwrap();
        let e:CustodyIntent=after_restart.read_record("trash",&effect).unwrap();
        assert_eq!(e.effect_call_id,effect);
        assert_eq!(e.sha256,view.sha256);
        q.deny("chat-1",&id).unwrap();
        assert!(source.exists());
        drop(after_restart);drop(custody);
        fs::remove_dir_all(root).unwrap();
    }
}
