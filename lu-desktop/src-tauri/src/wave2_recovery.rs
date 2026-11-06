//! Host-owned read-only recovery index for Wave 2 file/document operations.
//! A native modal writes a durable intent BEFORE beginning a consequential
//! effect. On restart we join this chat-scoped record to EffectJournal.
//! Nothing here restores a file, grants a capability or retries a tool.
//! APPLIED with matching receipt verifies past journal evidence. Archive
//! snapshots also permit independent read-only full-tree integrity checks.
use std::{fs::{self,File,OpenOptions},io::{Read,Write},path::{Path,PathBuf},sync::Mutex};
use lu_core::{
    journal::{EffectJournal,EffectState},
    native_archive_tree_tool,
    native_archive_tree::{TreeManifest,verify_tree_manifest,observe_tree_completion},
    native_docx_tool,native_pptx_tool,native_office_revision_tool,native_office_batch_tool,
    native_file_transfer_tool,
    wire::{self,proto},
};
use prost::Message;
use serde::{Deserialize,Serialize};

const MAX_RECORDS:usize=2048;
const MAX_RECORD_BYTES:u64=1024*1024;
fn effect_id(s:&str)->bool{
    (16..=128).contains(&s.len())&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn sha_ok(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn known_tool(s:&str)->bool{
    matches!(s,
        native_archive_tree_tool::TOOL_ID|
        native_docx_tool::TOOL_ID|
        native_pptx_tool::TOOL_ID|
        native_office_revision_tool::TOOL_ID|
        native_office_batch_tool::TOOL_ID|
        native_file_transfer_tool::COPY_ID|
        native_file_transfer_tool::MOVE_ID)
}
fn expected_output(s:&str)->Option<&'static str>{
    match s{
        native_archive_tree_tool::TOOL_ID=>Some(native_archive_tree_tool::OUTPUT),
        native_docx_tool::TOOL_ID=>Some(native_docx_tool::OUTPUT),
        native_pptx_tool::TOOL_ID=>Some(native_pptx_tool::OUTPUT),
        native_office_revision_tool::TOOL_ID=>Some(native_office_revision_tool::OUTPUT),
        native_office_batch_tool::TOOL_ID=>Some(native_office_batch_tool::OUTPUT),
        native_file_transfer_tool::COPY_ID|native_file_transfer_tool::MOVE_ID=>
            Some(native_file_transfer_tool::OUTPUT),
        _=>None,
    }
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct IntentRecord{
    schema:u32,
    chat_id:String,
    effect_call_id:String,
    tool_id:String,
    exact_target:String,
    target_name:String,
    approved_sha256:String,
    summary:String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    archive_manifest:Option<TreeManifest>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct OperationRecoveryView{
    pub effect_call_id:String,
    pub tool_id:String,
    pub exact_target:String,
    pub summary:String,
    pub approved_sha256:String,
    pub state:String,
    pub receipt_matched:bool,
    pub target_integrity:String,
    pub assessment:String,
}
pub struct Wave2OperationLedger{
    root:PathBuf,
    guard:Mutex<()>,
}
impl Wave2OperationLedger{
    pub fn new(app_data:&Path)->Result<Self,String>{
        let root=app_data.join("wave2-operation-intents");
        fs::create_dir_all(&root).map_err(|_|"Cannot initialize Wave 2 recovery store")?;
        let metadata=fs::symlink_metadata(&root)
            .map_err(|_|"Wave 2 recovery folder unavailable")?;
        if !metadata.is_dir()||metadata.file_type().is_symlink(){
            return Err("Unsafe Wave 2 recovery folder".into())
        }
        Ok(Self{root,guard:Mutex::new(())})
    }
    fn intent_path(&self,id:&str)->Result<PathBuf,String>{
        if !effect_id(id){return Err("Invalid recovery effect ID".into())}
        Ok(self.root.join(format!("operation-{id}.json")))
    }
    /// Native confirmation calls this before running the effect. No WebView
    /// path or permission is an input; all values come from pinned host review.
    pub fn record_intent(&self,chat:&str,effect:&str,tool:&str,
        exact_target:&str,target_name:&str,approved_sha256:&str,summary:&str
    )->Result<(),String>{
        self.record_with_manifest(chat,effect,tool,exact_target,target_name,
            approved_sha256,summary,None)
    }
    /// Bind a complete native-reviewed archive snapshot to the pre-effect
    /// intent. A legacy receipt-only record is never treated as full tree proof.
    pub fn record_archive_intent(&self,chat:&str,effect:&str,
        exact_target:&str,target_name:&str,manifest:&TreeManifest,summary:&str
    )->Result<(),String>{
        if !verify_tree_manifest(manifest){
            return Err("Untrusted approved archive manifest".into())
        }
        self.record_with_manifest(chat,effect,native_archive_tree_tool::TOOL_ID,
            exact_target,target_name,&manifest.sha256,summary,Some(manifest))
    }
    fn record_with_manifest(&self,chat:&str,effect:&str,tool:&str,
        exact_target:&str,target_name:&str,approved_sha256:&str,summary:&str,
        manifest:Option<&TreeManifest>
    )->Result<(),String>{
        if manifest.is_some()&&tool!=native_archive_tree_tool::TOOL_ID{
            return Err("Archive snapshot cannot authorize another tool".into())
        }
        if chat.is_empty()||chat.len()>128||!effect_id(effect)
            ||!known_tool(tool)||!sha_ok(approved_sha256)
            ||exact_target.is_empty()||exact_target.len()>2048
            ||target_name.is_empty()||target_name.len()>240
            ||summary.len()>2048{
            return Err("Invalid Wave 2 review intent".into())
        }
        let record=IntentRecord{
            schema:if manifest.is_some(){2}else{1},
            chat_id:chat.into(),effect_call_id:effect.into(),
            tool_id:tool.into(),exact_target:exact_target.into(),
            target_name:target_name.into(),approved_sha256:approved_sha256.into(),
            summary:summary.into(),archive_manifest:manifest.cloned(),
        };
        let bytes=serde_json::to_vec(&record).map_err(|_|"Recovery intent encoding failed")?;
        if bytes.len()>MAX_RECORD_BYTES as usize{
            return Err("Recovery intent too large".into())
        }
        let _guard=self.guard.lock().map_err(|_|"Recovery index busy")?;
        let path=self.intent_path(effect)?;
        let mut opts=OpenOptions::new();
        opts.create_new(true).write(true);
        let mut file=opts.open(path)
            .map_err(|_|"Recovery intent already exists or cannot be recorded")?;
        file.write_all(&bytes).map_err(|_|"Recovery intent write incomplete")?;
        file.sync_all().map_err(|_|"Recovery intent not durably synced")?;
        // A file fsync alone does not make its *new directory entry* crash-
        // durable on Unix filesystems. Sync the containing directory before
        // permitting the effect to be journaled and dispatched.
        #[cfg(unix)]
        File::open(&self.root).and_then(|d|d.sync_all())
            .map_err(|_|"Recovery intent directory entry not durable")?;
        Ok(())
    }
    fn read_intent(&self,id:&str)->Result<IntentRecord,String>{
        let path=self.intent_path(id)?;
        let stat=fs::symlink_metadata(&path)
            .map_err(|_|"Recovery intent missing")?;
        if !stat.is_file()||stat.file_type().is_symlink()
            ||stat.len()>MAX_RECORD_BYTES{
            return Err("Unsafe stored recovery record".into())
        }
        let mut data=Vec::new();
        File::open(path).map_err(|_|"Recovery record unavailable")?
            .take(MAX_RECORD_BYTES+1).read_to_end(&mut data)
            .map_err(|_|"Recovery record read failed")?;
        serde_json::from_slice(&data)
            .map_err(|_|"Recovery record malformed")
    }
    /// A journal receipt is a typed ToolReply. Validate tool, call ID,
    /// output kind, exact destination/name and the approved content witness.
    fn journal_receipt_matched(
        rec:&IntentRecord,receipt:&[u8]
    )->bool{
        let Ok(reply)=wire::decode_reply(receipt) else{return false};
        if reply.call_id!=rec.effect_call_id||
            Some(reply.output_type_url.as_str())!=expected_output(&rec.tool_id) {
            return false
        }
        let Some(proto::tool_reply::Outcome::Output(data))=reply.outcome else{
            return false
        };
        match rec.tool_id.as_str(){
            native_archive_tree_tool::TOOL_ID=>{
                let Ok(r)=proto::ArchiveTreeExtractResult::decode(data.as_slice())
                    else{return false};
                r.target_folder==rec.target_name&&
                    r.manifest_sha256==rec.approved_sha256
            },
            native_docx_tool::TOOL_ID=>{
                let Ok(r)=proto::DocxCreateResult::decode(data.as_slice())
                    else{return false};
                r.file_name==rec.target_name&&r.sha256==rec.approved_sha256
            },
            native_pptx_tool::TOOL_ID=>{
                let Ok(r)=proto::PptxCreateResult::decode(data.as_slice())
                    else{return false};
                r.file_name==rec.target_name&&r.sha256==rec.approved_sha256
            },
            native_office_revision_tool::TOOL_ID|
            native_office_batch_tool::TOOL_ID=>{
                let Ok(r)=proto::OfficeRevisionResult::decode(data.as_slice())
                    else{return false};
                r.target_new_name==rec.target_name&&
                    r.revised_sha256==rec.approved_sha256
            },
            native_file_transfer_tool::COPY_ID|native_file_transfer_tool::MOVE_ID=>{
                let Ok(r)=proto::FileTransferResult::decode(data.as_slice())
                    else{return false};
                r.target_name==rec.target_name&&r.sha256==rec.approved_sha256
                    &&r.source_removed==(rec.tool_id==native_file_transfer_tool::MOVE_ID)
            },
            _=>false,
        }
    }
    /// Read-only observation of the exact output path previously selected
    /// through native dialogs. Never reopen a stored path for writes.
    fn inspect_archive_target(rec:&IntentRecord)->String{
        let Some(manifest)=rec.archive_manifest.as_ref() else{
            return "NOT_RECHECKED".into()
        };
        if !verify_tree_manifest(manifest)||manifest.sha256!=rec.approved_sha256{
            return "INVALID_SNAPSHOT".into()
        }
        let target=Path::new(&rec.exact_target);
        if !target.is_absolute()||
            target.file_name().and_then(|s|s.to_str())!=Some(rec.target_name.as_str()){
            return "INVALID_TARGET".into()
        }
        let Some(parent)=target.parent() else{return "INVALID_TARGET".into()};
        let parent_meta=match fs::symlink_metadata(parent){
            Ok(m)=>m,
            Err(_)=>return "UNAVAILABLE".into(),
        };
        if !parent_meta.is_dir()||parent_meta.file_type().is_symlink(){
            return "UNAVAILABLE".into()
        }
        #[cfg(windows)]{
            use std::os::windows::fs::MetadataExt;
            if parent_meta.file_attributes()&0x400!=0{return "UNAVAILABLE".into()}
        }
        let destination_meta=match fs::symlink_metadata(target){
            Ok(m)=>m,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>
                return "MISSING".into(),
            Err(_)=>return "UNAVAILABLE".into(),
        };
        if !destination_meta.is_dir()||destination_meta.file_type().is_symlink(){
            return "INCOMPLETE_OR_CHANGED".into()
        }
        #[cfg(windows)]{
            use std::os::windows::fs::MetadataExt;
            if destination_meta.file_attributes()&0x400!=0{
                return "INCOMPLETE_OR_CHANGED".into()
            }
        }
        let Ok(root)=cap_std::fs::Dir::open_ambient_dir(
            parent,cap_std::ambient_authority()
        ) else{return "UNAVAILABLE".into()};
        match observe_tree_completion(&root,&rec.target_name,manifest){
            Ok(true)=>"VERIFIED".into(),
            Ok(false)=>"INCOMPLETE_OR_CHANGED".into(),
            Err(_)=>"UNAVAILABLE".into(),
        }
    }
    pub fn list(&self,chat:&str,effects:&EffectJournal)
        ->Result<Vec<OperationRecoveryView>,String>{
        let mut out=Vec::new();
        let mut scanned=0usize;
        for entry in fs::read_dir(&self.root).map_err(|_|"Cannot read operation recovery")?{
            let entry=entry.map_err(|_|"Cannot scan operation recovery")?;
            let name=entry.file_name();
            let Some(name)=name.to_str() else{continue};
            if !name.starts_with("operation-")||!name.ends_with(".json"){continue}
            scanned+=1;
            if scanned>MAX_RECORDS{return Err("Too many recovery records".into())}
            let id=&name[10..name.len()-5];
            if !effect_id(id){return Err("Invalid recovery entry name".into())}
            let rec=self.read_intent(id)?;
            if !matches!(rec.schema,1|2)||rec.effect_call_id!=id||
                !known_tool(&rec.tool_id)||!sha_ok(&rec.approved_sha256)||
                (rec.schema==2&&rec.archive_manifest.is_none())||
                rec.archive_manifest.as_ref().is_some_and(|m|
                    rec.tool_id!=native_archive_tree_tool::TOOL_ID||
                    !verify_tree_manifest(m)||m.sha256!=rec.approved_sha256){
                return Err("Untrusted recovery record".into())
            }
            if rec.chat_id!=chat{continue}
            let integrity=if rec.tool_id==native_archive_tree_tool::TOOL_ID{
                Self::inspect_archive_target(&rec)
            }else{"NOT_RECHECKED".into()};
            let evidence=effects.get(id).map_err(|_|"Effect journal unavailable")?;
            let (state,receipt_matched,assessment)=match evidence{
                None=>("INTENT_RECORDED".into(),false,
                    "Approved intent exists, but no effect was journaled; no automatic replay".into()),
                Some(ref e) if e.tool_id!=rec.tool_id=>(
                    "JOURNAL_CONFLICT".into(),false,
                    "Stored tool identity conflicts with durable approval".into(),
                ),
                Some(e)=>{
                    let matched=e.state==EffectState::Applied &&
                        e.receipt.as_ref().is_some_and(|b|
                            Self::journal_receipt_matched(&rec,b));
                    let status=match e.state{
                        EffectState::Prepared=>"PREPARED",
                        EffectState::Executing=>"EXECUTING",
                        EffectState::Applied if matched=>"APPLIED_RECEIPT_MATCHED",
                        EffectState::Applied=>"RECEIPT_MISMATCH",
                        EffectState::Failed=>"FAILED",
                        EffectState::Uncertain=>"UNCERTAIN",
                    };
                    let description=match status{
                        "APPLIED_RECEIPT_MATCHED"=>
                            "Durable effect receipt matches approved source/output witness; current filesystem contents not rechecked",
                        "PREPARED"|"EXECUTING"=>
                            "Operation has not reached verified completion; do not restart automatically",
                        "FAILED"=>
                            "Operation reported failure; inspect target before another approved attempt",
                        _=>"Manual reconciliation needed; never automatically retry",
                    };
                    (status.into(),matched,description.into())
                },
            };
            let assessment=if rec.tool_id==native_archive_tree_tool::TOOL_ID &&
                integrity!="NOT_RECHECKED"{
                format!("{assessment}; current full-tree integrity: {integrity}. Historical journal state is unchanged; no automatic replay")
            }else{assessment};
            out.push(OperationRecoveryView{
                effect_call_id:rec.effect_call_id,tool_id:rec.tool_id,
                exact_target:rec.exact_target,summary:rec.summary,
                approved_sha256:rec.approved_sha256,
                state,receipt_matched,target_integrity:integrity,assessment,
            });
        }
        out.sort_by(|a,b|a.effect_call_id.cmp(&b.effect_call_id));
        Ok(out)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use lu_core::{contracts::{ToolInvocation,ToolOutput,ProtoPayload},registry::ToolResponse};
    #[test]
    fn read_only_recovery_persists_chat_scope_and_matches_actual_typed_receipt(){
        let root=std::env::temp_dir().join(format!(
            "lu-wave2-ledger-test-{}",crate::next_id("test-")
        ));
        fs::create_dir_all(&root).unwrap();
        let ledger=Wave2OperationLedger::new(&root).unwrap();
        let effects=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
        let call=crate::next_id("g-");
        let digest="a".repeat(64);
        ledger.record_intent("chat-one",&call,native_docx_tool::TOOL_ID,
            "C:/Documents/new.docx","new.docx",&digest,"New DOCX").unwrap();
        assert!(ledger.record_intent("chat-one",&call,native_docx_tool::TOOL_ID,
            "C:/Documents/new.docx","new.docx",&digest,"New DOCX").is_err());
        assert_eq!(ledger.list("chat-two",&effects).unwrap().len(),0);
        assert_eq!(ledger.list("chat-one",&effects).unwrap()[0].state,
            "INTENT_RECORDED");
        let invoked=ToolInvocation{
            call_id:call.clone(),tool_id:native_docx_tool::TOOL_ID.into(),
            workspace_id:None,
            input:ProtoPayload{type_url:native_docx_tool::INPUT.into(),bytes:vec![1]},
        };
        effects.prepare(&invoked).unwrap();
        effects.mark_executing(&call).unwrap();
        let response:ToolResponse=Ok(ToolOutput{payload:ProtoPayload{
            type_url:native_docx_tool::OUTPUT.into(),
            bytes:proto::DocxCreateResult{
                file_name:"new.docx".into(),bytes_written:123,
                sha256:digest.clone(),
            }.encode_to_vec(),
        }});
        let wire=wire::encode_reply(&call,&response).unwrap();
        effects.mark_applied(&call,&wire).unwrap();
        let reloaded=Wave2OperationLedger::new(&root).unwrap();
        let status=reloaded.list("chat-one",&effects).unwrap();
        assert_eq!(status[0].state,"APPLIED_RECEIPT_MATCHED");
        assert!(status[0].receipt_matched);
        assert!(status[0].assessment.contains("not rechecked"));
        drop(effects);drop(reloaded);drop(ledger);
        fs::remove_dir_all(root).unwrap();
    }
}


// Ensure the originally approved manifest, not just a journal receipt, is
// available after Lu restarts. This test never grants a new write capability.
#[cfg(test)]
mod archive_restart_tests{
    use super::*;
    use cap_std::{ambient_authority,fs::Dir};
    use lu_core::{
        contracts::{ToolInvocation,ToolOutput,ProtoPayload},
        native_archive_tree::{preview_archive_tree,extract_archive_tree_new},
        native_linux_archive::create_linux_archive_new,
        registry::ToolResponse,
    };
    #[test]
    fn restart_rechecks_real_tree_and_detects_external_tampering(){
        let root=std::env::temp_dir().join(format!(
            "lu-wave2-archive-restart-{}",crate::next_id("test-")
        ));
        fs::create_dir_all(&root).unwrap();
        let dir=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
        fs::write(root.join("input.txt"),b"approved bytes").unwrap();
        create_linux_archive_new(&dir,"input.tar",&["input.txt".into()],
            "archive-recovery-stage-12345").unwrap();
        let manifest=preview_archive_tree(&dir,"input.tar").unwrap();
        let ledger=Wave2OperationLedger::new(&root).unwrap();
        let effects=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
        let call=crate::next_id("g-");
        let target=root.join("expanded");
        ledger.record_archive_intent("chat-one",&call,
            target.to_str().unwrap(),"expanded",&manifest,
            "Approved archive extraction").unwrap();
        assert_eq!(ledger.list("chat-one",&effects).unwrap()[0].target_integrity,"MISSING");
        let invoked=ToolInvocation{
            call_id:call.clone(),tool_id:native_archive_tree_tool::TOOL_ID.into(),
            workspace_id:None,
            input:ProtoPayload{type_url:native_archive_tree_tool::INPUT.into(),bytes:vec![1]},
        };
        effects.prepare(&invoked).unwrap();
        effects.mark_executing(&call).unwrap();
        let receipt=extract_archive_tree_new(
            &dir,"input.tar",&dir,"expanded",&manifest.sha256
        ).unwrap();
        let response:ToolResponse=Ok(ToolOutput{payload:ProtoPayload{
            type_url:native_archive_tree_tool::OUTPUT.into(),
            bytes:proto::ArchiveTreeExtractResult{
                target_folder:"expanded".into(),
                files_extracted:receipt.files_extracted as u32,
                bytes_extracted:receipt.bytes_extracted,
                manifest_sha256:receipt.manifest_sha256,
            }.encode_to_vec(),
        }});
        let wire=wire::encode_reply(&call,&response).unwrap();
        effects.mark_applied(&call,&wire).unwrap();
        let reloaded=Wave2OperationLedger::new(&root).unwrap();
        let observed=reloaded.list("chat-one",&effects).unwrap();
        assert_eq!(observed[0].state,"APPLIED_RECEIPT_MATCHED");
        assert!(observed[0].receipt_matched);
        assert_eq!(observed[0].target_integrity,"VERIFIED");
        fs::write(root.join("expanded/input.txt"),b"modified elsewhere").unwrap();
        let observed=reloaded.list("chat-one",&effects).unwrap();
        assert_eq!(observed[0].state,"APPLIED_RECEIPT_MATCHED");
        assert_eq!(observed[0].target_integrity,"INCOMPLETE_OR_CHANGED");
        assert!(observed[0].assessment.contains("no automatic replay"));
        drop(reloaded);drop(effects);drop(ledger);drop(dir);
        fs::remove_dir_all(root).unwrap();
    }
}