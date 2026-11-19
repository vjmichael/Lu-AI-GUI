//! CP-06D-R1: bounded inert model intent -> distinct trusted native review ->
//! one-use durable custody -> independently observed outcome. No ambient path
//! or grant may be supplied by the model. The trusted host owns the picker and
//! confirmation; the same original authenticated human turn remains the origin.
use std::{sync::Arc,io::ErrorKind,path::Path,fs};
use cap_std::{fs::Dir,ambient_authority};
use prost::Message;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use crate::{
    contracts::{AuthorizationError,ProtoPayload,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,TrustedUserTurnVerifier},
    inference_host::Cancellation,
    journal::{EffectJournal,EffectState,IndependentOutcomeVerdict},
    model::ModelToolCatalog,
    model_json_boundary::parse_exact_boundary_object,
    model_turn_journal::{ModelAuditKind as K,ModelTurnJournal},
    native_file_transfer::{source_digest,TrashReceipt},
    native_file_trash_tool::{register_trash_restore_tools,ExactTrashPolicy,ExactRestorePolicy,TRASH_ID,RESTORE_ID,TRASH_OUTPUT,RESTORE_OUTPUT},
    native_trash_custody::{CustodyRecord,CustodyState,TrashCustodyLedger},
    policy::{CapabilityGrant,PolicyEngine},registry::NativeToolRegistry,
    supervised_tool_generation::ModelStepGenerator,wire::{self,proto},
};
fn sha(data:&[u8])->String{format!("{:x}",Sha256::digest(data))}
fn record(j:&ModelTurnJournal,chat:&str,turn:&str,kind:K,value:&str)->Result<(),String>{
    j.append(chat,turn,kind,value,None).map(|_|()).map_err(|_|"Native operation audit unavailable".into())
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum CustodyAction{Trash,Restore}
impl CustodyAction{
    pub fn operation(self)->&'static str{match self{Self::Trash=>"trash",Self::Restore=>"restore"}}
    fn selection(self)->&'static str{match self{Self::Trash=>"trash_file",Self::Restore=>"restore_file"}}
    pub fn native_id(self)->&'static str{match self{Self::Trash=>TRASH_ID,Self::Restore=>RESTORE_ID}}
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct CustodyIntent{original:String,action:CustodyAction,leaf:Option<String>}
impl CustodyIntent{
    /// Only explicit reversible single-file requests; no permanent deletion.
    pub fn parse(original:&str)->Option<Self>{
        if original.len()>512||!original.is_ascii()||original.chars().any(char::is_control){return None}
        let body=original.trim().trim_end_matches(['?','!','.']).trim();
        let low=body.to_ascii_lowercase();
        let (action,prefix)=if low.starts_with("trash "){(CustodyAction::Trash,"trash ")}else if low.starts_with("restore "){(CustodyAction::Restore,"restore ")}else{return None};
        let rest=&body[prefix.len()..];let lower=rest.to_ascii_lowercase();
        let leaf=if ["a file","the file","my file","this file"].contains(&lower.as_str()){None}else{
            let prefix=["the file ","my file ","file "].into_iter().find(|p|lower.starts_with(p))?;
            let s=&rest[prefix.len()..];
            if s.len()>128||!s.contains('.')||s.starts_with('.')||s.ends_with('.')||s.contains("..")||s.to_ascii_lowercase().starts_with("lu-trash-")||!s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'.'|b'_'|b'-')){return None}
            Some(s.to_owned())
        };
        Some(Self{original:original.into(),action,leaf})
    }
    pub fn action(&self)->CustodyAction{self.action}
    pub fn matches_leaf(&self,leaf:&str)->bool{self.leaf.as_ref().is_none_or(|n|n==leaf)}
    pub fn original(&self)->&str{&self.original}
    /// No resource handle, target or authority exists during model inference.
    pub fn propose(self,chat:&str,turn:&str,verifier:&dyn TrustedUserTurnVerifier,
        model:&mut dyn ModelStepGenerator,cancel:&Cancellation,journal:&ModelTurnJournal,
        checkpoint:&(dyn Fn()->Result<(),String>+Send+Sync))->Result<Option<CustodyProposal>,String>{
        checkpoint()?;
        if cancel.is_cancelled()||!verifier.verify_user_turn(chat,turn){return Err("Cancelled original file operation".into())}
        if !journal.events_for(chat,turn).map_err(|_|"File operation audit unavailable")?.is_empty(){return Err("File operation proposal already exists; no replay".into())}
        record(journal,chat,turn,K::UserRequest,&self.original)?;
        let schema=json!({"type":"object","properties":{"selection":{"enum":[self.action.selection(),null]}},"required":["selection"],"additionalProperties":false});
        let prompt=format!("Select a file operation for REVIEW only. The original request asks to {} one regular file using Lu reversible custody in its original folder. If this is the requested operation, select {}; otherwise select null. You cannot choose a path or give approval. Nothing is executed by selecting. Return only one JSON selection object. Original request (JSON string): {}",self.action.operation(),self.action.selection(),json!(self.original));
        let raw=match model.generate_step_constrained(&prompt,128,cancel,&schema){Ok(v)=>v,Err(_)=>{record(journal,chat,turn,K::GenerationFailure,"MODEL_CUSTODY_PROPOSAL_FAILED")?;checkpoint()?;if cancel.is_cancelled(){return Err("Cancelled original file operation".into())}return Err("No file operation was authorized or performed".into())}};
        record(journal,chat,turn,K::ModelOutput,&raw)?;
        checkpoint()?;
        if cancel.is_cancelled()||!verifier.verify_user_turn(chat,turn){return Err("Cancelled original file operation".into())}
        let selected=parse_exact_boundary_object(&raw,4096).ok().is_some_and(|p|p.len()==1&&p.get("selection").and_then(Value::as_str)==Some(self.action.selection()));
        if !selected{record(journal,chat,turn,K::Rejected,"MODEL_CUSTODY_NO_VALID_SELECTION")?;return Ok(None)}
        let model_sha=sha(raw.as_bytes());
        record(journal,chat,turn,K::ParsedCandidate,&json!({"selection":self.action.selection(),"model_output_sha256":model_sha,"executed":false}).to_string())?;
        Ok(Some(CustodyProposal{intent:self,chat:chat.into(),turn:turn.into(),model_sha}))
    }
}
/// Non-cloneable, inert proposal. Dropping it authorizes nothing.
pub struct CustodyProposal{intent:CustodyIntent,chat:String,turn:String,model_sha:String}
impl CustodyProposal{
    pub fn action(&self)->CustodyAction{self.intent.action}
    pub fn matches_leaf(&self,leaf:&str)->bool{self.intent.matches_leaf(leaf)}
    // A caller must continue against the original durable audit, never an
    // empty/replacement journal or an output belonging to another turn.
    fn check_proposal_audit(&self,journal:&ModelTurnJournal,count:usize)->Result<Vec<crate::model_turn_journal::ModelAuditRecord>,String>{
        let rows=journal.events_for(&self.chat,&self.turn).map_err(|_|"File proposal audit unavailable")?;
        if rows.len()!=count||rows[0].kind!=K::UserRequest||rows[0].payload!=self.intent.original||
            rows[1].kind!=K::ModelOutput||sha(rows[1].payload.as_bytes())!=self.model_sha||rows[2].kind!=K::ParsedCandidate{return Err("Original file proposal audit does not match".into())}
        let candidate:Value=serde_json::from_str(&rows[2].payload).map_err(|_|"Invalid file proposal audit")?;
        if candidate!=json!({"selection":self.intent.action.selection(),"model_output_sha256":self.model_sha,"executed":false}){return Err("Original file proposal audit differs".into())}
        Ok(rows)
    }
    pub fn deny(self,journal:&ModelTurnJournal)->Result<(),String>{self.check_proposal_audit(journal,3)?;record(journal,&self.chat,&self.turn,K::Rejected,"HUMAN_DECLINED_CUSTODY_SELECTION")}
    pub fn review_trash(self,file:&Path,custody_id:&str,effect_id:&str,journal:&ModelTurnJournal)->Result<ReviewedCustody,String>{
        if self.action()!=CustodyAction::Trash{return Err("Wrong operation".into())}
        let name=file.file_name().and_then(|n|n.to_str()).ok_or("Invalid selected filename")?;
        if name.to_ascii_lowercase().starts_with("lu-trash-")||!self.matches_leaf(name){return Err("Selected file differs from original request".into())}
        let parent=file.parent().ok_or("Selected folder unavailable")?;let root=pinned_folder(parent)?;
        let (bytes,sha256)=source_digest(&root,name).map_err(|_|"Selected file unavailable")?;
        let r=CustodyRecord{id:custody_id.into(),chat_id:self.chat.clone(),parent:parent.to_str().ok_or("Invalid folder")?.into(),original:name.into(),trash_entry:format!("lu-trash-{effect_id}"),bytes,sha256,trash_effect_id:effect_id.into(),restore_effect_id:None,state:CustodyState::Pending};
        self.review(root,r,None,effect_id,journal)
    }
    pub fn review_restore(self,custody_id:&str,effect_id:&str,ledger:&TrashCustodyLedger,effects:&EffectJournal,journal:&ModelTurnJournal)->Result<ReviewedCustody,String>{
        if self.action()!=CustodyAction::Restore{return Err("Wrong operation".into())}
        let (r,receipt)=ledger.validated_trash_receipt(custody_id,&self.chat,effects).map_err(|_|"No verified recoverable file belongs to this conversation")?;
        if !self.matches_leaf(&r.original){return Err("Recovered file differs from original request".into())}
        let root=pinned_folder(Path::new(&r.parent))?;self.review(root,r,Some(receipt),effect_id,journal)
    }
    fn review(self,root:Arc<Dir>,custody:CustodyRecord,restore:Option<TrashReceipt>,effect_id:&str,journal:&ModelTurnJournal)->Result<ReviewedCustody,String>{
        self.check_proposal_audit(journal,3)?;
        if !(16..=128).contains(&effect_id.len())||!effect_id.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_')){return Err("Invalid operation identity".into())}
        let registry=NativeToolRegistry::new();let mut catalog=ModelToolCatalog::new();register_trash_restore_tools(&registry,&mut catalog,Arc::clone(&root),Arc::clone(&root),restore.clone())?;
        let (name,args)=if restore.is_some(){("lu_file_restore",json!({"trusted_receipt_id":custody.trash_entry}))}else{("lu_file_trash",json!({"source_name":custody.original,"expected_sha256":custody.sha256}))};
        let input=catalog.encode_call(&std::collections::BTreeSet::from([name.to_owned()]),name,"review-custody",Some("review"),&args.to_string()).map_err(|_|"Invalid reviewed file operation")?.input;
        let reviewed=ReviewedCustody{proposal:self,root,custody,restore,effect_id:effect_id.into(),preview_sha:String::new(),input};reviewed.check_files()?;
        let preview=reviewed.preview();let preview_sha=sha(preview.to_string().as_bytes());record(journal,&reviewed.proposal.chat,&reviewed.proposal.turn,K::HostRouteSelected,&json!({"phase":"native_preview","review_sha256":preview_sha,"review":preview,"approved":false}).to_string())?;
        Ok(ReviewedCustody{preview_sha,..reviewed})
    }
}
fn pinned_folder(parent:&Path)->Result<Arc<Dir>,String>{
    if !parent.is_absolute(){return Err("Folder must be absolute".into())}let meta=fs::symlink_metadata(parent).map_err(|_|"Folder unavailable")?;
    if !meta.is_dir()||meta.file_type().is_symlink(){return Err("Linked folder refused".into())}
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;if meta.file_attributes()&0x400!=0{return Err("Reparse folder refused".into())}}
    Ok(Arc::new(Dir::open_ambient_dir(parent,ambient_authority()).map_err(|_|"Folder unavailable")?))
}
pub struct ReviewedCustody{proposal:CustodyProposal,root:Arc<Dir>,custody:CustodyRecord,restore:Option<TrashReceipt>,effect_id:String,preview_sha:String,input:ProtoPayload}
impl ReviewedCustody{
    pub fn custody(&self)->&CustodyRecord{&self.custody}
    pub fn action(&self)->CustodyAction{self.proposal.action()}
    fn preview(&self)->Value{json!({"action":self.action().operation(),"custody_id":self.custody.id,"parent":self.custody.parent,"original":self.custody.original,"trash_entry":self.custody.trash_entry,"bytes":self.custody.bytes,"sha256":self.custody.sha256,"prior_trash_effect":self.custody.trash_effect_id,"effect_call_id":self.effect_id,"original_request_sha256":sha(self.proposal.intent.original.as_bytes()),"model_output_sha256":self.proposal.model_sha,"input_sha256":sha(&self.input.bytes)})}
    fn check_files(&self)->Result<(),String>{
        let (from,to)=if self.restore.is_some(){(&self.custody.trash_entry,&self.custody.original)}else{(&self.custody.original,&self.custody.trash_entry)};
        if source_digest(&self.root,from).ok()!=Some((self.custody.bytes,self.custody.sha256.clone()))||!matches!(self.root.symlink_metadata(to),Err(e) if e.kind()==ErrorKind::NotFound){return Err("Source changed or destination occupied; no operation authorized".into())}Ok(())
    }
    pub fn deny(self,journal:&ModelTurnJournal)->Result<(),String>{self.check_review(journal)?;record(journal,&self.proposal.chat,&self.proposal.turn,K::Rejected,"HUMAN_DECLINED_EXACT_CUSTODY")}
    fn check_review(&self,journal:&ModelTurnJournal)->Result<(),String>{let rows=self.proposal.check_proposal_audit(journal,4)?;let v:Value=serde_json::from_str(&rows[3].payload).map_err(|_|"Invalid review audit")?;if rows[3].kind!=K::HostRouteSelected||v!=json!({"phase":"native_preview","review_sha256":self.preview_sha,"review":self.preview(),"approved":false}){return Err("Original native review differs".into())}Ok(())}
    pub async fn execute(self,confirmed:bool,verifier:&dyn TrustedUserTurnVerifier,ledger:&TrashCustodyLedger,effects:&EffectJournal,journal:&ModelTurnJournal,checkpoint:&(dyn Fn()->Result<(),String>+Send+Sync))->Result<Option<VerifiedCustody>,String>{
        if !confirmed{self.deny(journal)?;return Ok(None)}self.check_review(journal)?;checkpoint()?;
        let chat=&self.proposal.chat;let turn=&self.proposal.turn;let action=self.action();let id=&self.effect_id;
        if !verifier.verify_user_turn(chat,turn)||effects.get(id).map_err(|_|"Effect history unavailable")?.is_some(){return Err("Original approval expired or effect already exists; do not retry".into())}
        self.check_files()?;
        if let Some(receipt)=&self.restore{let (r,prior)=ledger.validated_trash_receipt(&self.custody.id,chat,effects).map_err(|_|"Custody no longer recoverable")?;if r!=self.custody||&prior!=receipt{return Err("Custody changed during review".into())}}
        let registry=NativeToolRegistry::new();let mut catalog=ModelToolCatalog::new();register_trash_restore_tools(&registry,&mut catalog,Arc::clone(&self.root),Arc::clone(&self.root),self.restore.clone())?;
        let task=format!("custody-{id}");let scope=format!("custody-scope-{id}");let authority=ConversationAuthority::new();let policy=PolicyEngine::new();
        authority.open_user_task(TaskAuthority{conversation_id:chat.clone(),user_turn_id:turn.clone(),task_id:task.clone(),execution_scope_id:scope.clone(),capabilities:vec![AuthorizedCapability{tool_id:action.native_id().into(),effect:ToolEffect::WriteWorkspace}],expires_at:None},verifier).map_err(|_|"Original custody task unavailable")?;
        policy.grant(CapabilityGrant{tool_id:action.native_id().into(),effect:ToolEffect::WriteWorkspace,scope_id:scope.clone(),expires_at:None}).map_err(|_|"Custody grant unavailable")?;
        let live=LivePolicy{policy:&policy,checkpoint,verifier,chat,turn};
        record(journal,chat,turn,K::HostRouteSelected,&json!({"phase":"native_confirmation","approved":true,"review_sha256":self.preview_sha,"effect_call_id":id,"native_tool_id":action.native_id(),"authority_task":task,"input_sha256":sha(&self.input.bytes)}).to_string())?;
        if self.restore.is_some(){ledger.begin_restore(&self.custody.id,chat,id).map_err(|_|"Restore intent not durable")?;}else{ledger.stage(&self.custody).map_err(|_|"Trash intent not durable")?;}
        let call=ToolInvocation{call_id:id.clone(),tool_id:action.native_id().into(),workspace_id:Some(scope.clone()),input:self.input.clone()};let origin=BoundCallOrigin{conversation_id:chat.clone(),user_turn_id:turn.clone(),task_id:task.clone()};
        let output=if let Some(receipt)=&self.restore{authority.dispatch_bound_durable(&registry,effects,call,origin,&ExactRestorePolicy{host_policy:&live,trusted_receipt_id:&receipt.trash_entry,receipt}).await}else{authority.dispatch_bound_durable(&registry,effects,call,origin,&ExactTrashPolicy{host_policy:&live,trusted_source:&self.root,source_name:&self.custody.original,source_sha256:&self.custody.sha256}).await};let _=authority.revoke_task(&task);let _=policy.revoke_scope(&scope);
        let verified=(||->Result<VerifiedCustody,String>{
            let output=output.map_err(|_|"Native operation failed or uncertain; inspect custody before any retry")?;let row=effects.get(id).map_err(|_|"Effect history unavailable")?.ok_or("Missing effect receipt")?;let raw=row.receipt.as_ref().ok_or("Missing receipt")?;let durable=wire::decode_reply(raw).map_err(|_|"Invalid receipt")?;let r=&self.custody;
            let (from,to,typ,match_reply)=if self.restore.is_some(){let reply=proto::FileRestoreResult::decode(output.payload.bytes.as_slice()).map_err(|_|"Invalid restore result")?;(&r.trash_entry,&r.original,RESTORE_OUTPUT,reply.restored_name==r.original&&reply.bytes==r.bytes&&reply.sha256==r.sha256)}else{let reply=proto::FileTrashResult::decode(output.payload.bytes.as_slice()).map_err(|_|"Invalid trash result")?;(&r.original,&r.trash_entry,TRASH_OUTPUT,reply.original_name==r.original&&reply.trash_entry==r.trash_entry&&reply.bytes==r.bytes&&reply.sha256==r.sha256)};
            let correct=row.state==EffectState::Applied&&row.tool_id==action.native_id()&&durable.call_id==*id&&durable.output_type_url==typ&&output.payload.type_url==typ&&matches!(durable.outcome,Some(proto::tool_reply::Outcome::Output(ref b)) if b==&output.payload.bytes)&&match_reply&&source_digest(&self.root,to).ok()==Some((r.bytes,r.sha256.clone()))&&matches!(self.root.symlink_metadata(from),Err(e) if e.kind()==ErrorKind::NotFound);
            if !correct{if row.state==EffectState::Applied{effects.record_independent_outcome(id,raw,IndependentOutcomeVerdict::Conflict,"FILE_CUSTODY_DIRECT_V1",None).map_err(|_|"Could not journal custody conflict")?;}return Err("Native custody state differs from approved receipt; manual reconciliation required".into())}
            let observed=json!({"action":action.operation(),"custody_id":r.id,"original":r.original,"trash_entry":r.trash_entry,"bytes":r.bytes,"sha256":r.sha256,"original_present":self.restore.is_some()});let observed_sha=sha(observed.to_string().as_bytes());let receipt_sha=sha(raw);
            effects.record_independent_outcome(id,raw,IndependentOutcomeVerdict::Verified,"FILE_CUSTODY_DIRECT_V1",Some(&observed_sha)).map_err(|_|"Could not journal verified custody")?;
            if self.restore.is_some(){ledger.confirm_restore(&r.id,chat,effects)}else{ledger.confirm_trash(&r.id,chat,effects)}.map_err(|_|"Custody transition unverified")?;
            record(journal,chat,turn,K::ToolFeedback,&json!({"effect_call_id":id,"native_tool_id":action.native_id(),"receipt_sha256":receipt_sha,"observed_sha256":observed_sha,"review_sha256":self.preview_sha,"observed":observed}).to_string())?;
            Ok(VerifiedCustody{chat:chat.clone(),turn:turn.clone(),action,receipt_sha,observed_sha,call_id:id.clone(),custody_id:r.id.clone(),original:r.original.clone(),bytes:r.bytes})
        })();match verified{Ok(v)=>Ok(Some(v)),Err(e)=>{let _=ledger.mark_uncertain(&self.custody.id,chat);record(journal,chat,turn,K::Rejected,"CUSTODY_UNCONFIRMED_NO_REPLAY")?;Err(e)}}
    }
}
struct LivePolicy<'a>{policy:&'a PolicyEngine,checkpoint:&'a(dyn Fn()->Result<(),String>+Send+Sync),verifier:&'a dyn TrustedUserTurnVerifier,chat:&'a str,turn:&'a str}
impl ToolAuthorizer for LivePolicy<'_>{fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)->Result<(),AuthorizationError>{
    (self.checkpoint)().map_err(|_|AuthorizationError::new("Original file operation no longer live"))?;
    if !self.verifier.verify_user_turn(self.chat,self.turn){return Err(AuthorizationError::new("Original human turn changed"))}
    self.policy.authorize(call,desc)
}}
pub struct VerifiedCustody{chat:String,turn:String,action:CustodyAction,receipt_sha:String,observed_sha:String,call_id:String,custody_id:String,original:String,bytes:u64}
impl VerifiedCustody{
    pub fn present(self,model:&mut dyn ModelStepGenerator,cancel:&Cancellation,journal:&ModelTurnJournal,effects:&EffectJournal,ledger:&TrashCustodyLedger,checkpoint:&(dyn Fn()->Result<(),String>+Send+Sync))->Result<String,String>{
        checkpoint()?;if cancel.is_cancelled(){return Err("Cancelled after file operation; consult custody history".into())}
        if !effects.independent_verified_receipt(&self.call_id,self.action.native_id(),&self.receipt_sha).map_err(|_|"Verified receipt unavailable")?{return Err("Verified receipt unavailable".into())}
        let r=ledger.get(&self.custody_id).map_err(|_|"Custody unavailable")?.ok_or("Custody unavailable")?;
        if r.chat_id!=self.chat||r.original!=self.original||r.bytes!=self.bytes||match self.action{CustodyAction::Trash=>r.state!=CustodyState::Trashed||r.trash_effect_id!=self.call_id,CustodyAction::Restore=>r.state!=CustodyState::Restored||r.restore_effect_id.as_deref()!=Some(&self.call_id)}{return Err("Verified custody state changed".into())}
        let rows=journal.events_for(&self.chat,&self.turn).map_err(|_|"Custody audit unavailable")?;let facts:Value=serde_json::from_str(&rows.last().filter(|e|e.kind==K::ToolFeedback).ok_or("No original feedback")?.payload).map_err(|_|"Invalid original feedback")?;
        if rows.len()!=6||facts["effect_call_id"]!=self.call_id||facts["receipt_sha256"]!=self.receipt_sha||facts["observed_sha256"]!=self.observed_sha||sha(facts["observed"].to_string().as_bytes())!=self.observed_sha{return Err("Original feedback differs".into())}
        let schema=json!({"type":"object","properties":{"status":{"enum":["verified_completion","unverified"]}},"required":["status"],"additionalProperties":false});let prompt=format!("The native reversible file operation succeeded and its receipt and actual file state were independently checked. Return only JSON status verified_completion if reporting this verified result, otherwise unverified. Do not request tools or invent facts. Receipt: {}",json!({"operation":self.action.operation(),"bytes":self.bytes}));
        let selected=match model.generate_step_constrained(&prompt,128,cancel,&schema){Ok(raw)=>{record(journal,&self.chat,&self.turn,K::ModelOutput,&raw)?;parse_exact_boundary_object(&raw,4096).ok().is_some_and(|p|p.len()==1&&p.get("status").and_then(Value::as_str)==Some("verified_completion"))},Err(_)=>{record(journal,&self.chat,&self.turn,K::GenerationFailure,"CUSTODY_CONTINUATION_FAILED")?;false}};
        checkpoint()?;if cancel.is_cancelled(){return Err("Cancelled after operation; consult custody history".into())}if !selected{record(journal,&self.chat,&self.turn,K::Rejected,"CUSTODY_CONTINUATION_UNVERIFIED")?;}
        let answer=format!("{}{} {} ({} bytes). {}",if selected{""}else{"I couldn't verify my generated explanation. The native receipt confirms: "},if self.action==CustodyAction::Trash{"Moved to Lu's reversible trash:"}else{"Restored"},crate::evidence_grounding::literal(&self.original),self.bytes,if self.action==CustodyAction::Trash{"The contents were verified in the same folder's Lu custody entry; the original name was absent. This is not the OS recycle bin."}else{"The original name and contents were verified; the custody entry was absent."});record(journal,&self.chat,&self.turn,K::EvidencePresentation,&json!({"origin":if selected{"model_selected_verified_effect_host_rendered"}else{"host_verified_effect_fallback"},"effect_call_id":self.call_id,"custody_id":self.custody_id,"receipt_sha256":self.receipt_sha,"observed_sha256":self.observed_sha,"answer":answer}).to_string())?;Ok(answer)
    }
}
