//! CP-06D: bounded inert model intent -> distinct trusted native review ->
//! one-use durable transfer -> independently observed outcome. No ambient path
//! or grant may be supplied by the model. The trusted host owns the picker and
//! confirmation; the same original authenticated human turn remains the origin.
use std::{sync::Arc,io::ErrorKind};
use cap_std::fs::Dir;
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
    native_file_transfer::source_digest,
    native_file_transfer_tool::{register_file_transfer_tools,ExactFileTransferPolicy,COPY_ID,MOVE_ID,INPUT,OUTPUT},
    policy::{CapabilityGrant,PolicyEngine},registry::NativeToolRegistry,
    supervised_tool_generation::ModelStepGenerator,wire::{self,proto},
};
fn sha(data:&[u8])->String{format!("{:x}",Sha256::digest(data))}
fn record(j:&ModelTurnJournal,chat:&str,turn:&str,kind:K,value:&str)->Result<(),String>{
    j.append(chat,turn,kind,value,None).map(|_|()).map_err(|_|"Native operation audit unavailable".into())
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum TransferAction{Copy,Move}
impl TransferAction{
    pub fn operation(self)->&'static str{match self{Self::Copy=>"copy",Self::Move=>"move"}}
    fn selection(self)->&'static str{match self{Self::Copy=>"copy_file",Self::Move=>"move_file"}}
    pub fn native_id(self)->&'static str{match self{Self::Copy=>COPY_ID,Self::Move=>MOVE_ID}}
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct TransferIntent{original:String,action:TransferAction,leaf:Option<String>}
impl TransferIntent{
    /// Initial exact grammar. Copy is a new name in the chosen file's folder.
    /// Moving is supported only when the user explicitly asks for a new name.
    /// Destination paths or a user-specified target require a different review.
    pub fn parse(original:&str)->Option<Self>{
        if original.len()>512||!original.is_ascii()||original.chars().any(char::is_control){return None}
        let body=original.trim().trim_end_matches(['?','!','.']).trim();
        let low=body.to_ascii_lowercase();
        let action=if low.starts_with("copy "){TransferAction::Copy}else if low.starts_with("move "){TransferAction::Move}else{return None};
        let mut rest=&body[5..];let mut lower=rest.to_ascii_lowercase();
        let suffix=" to a new name";
        if lower.ends_with(suffix){rest=&rest[..rest.len()-suffix.len()];lower=rest.to_ascii_lowercase();}
        else if action==TransferAction::Move{return None}
        let leaf=if ["a file","the file","my file","this file"].contains(&lower.as_str()){None}else{
            let prefix=["the file ","my file ","file "].into_iter().find(|p|lower.starts_with(p))?;
            let s=&rest[prefix.len()..];
            if s.len()>128||!s.contains('.')||s.starts_with('.')||s.ends_with('.')||s.contains("..")||!s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'.'|b'_'|b'-')){return None}
            Some(s.to_owned())
        };
        Some(Self{original:original.into(),action,leaf})
    }
    pub fn action(&self)->TransferAction{self.action}
    pub fn matches_leaf(&self,leaf:&str)->bool{self.leaf.as_ref().is_none_or(|n|n==leaf)}
    pub fn original(&self)->&str{&self.original}
    /// No resource handle, target or authority exists during model inference.
    pub fn propose(self,chat:&str,turn:&str,verifier:&dyn TrustedUserTurnVerifier,
        model:&mut dyn ModelStepGenerator,cancel:&Cancellation,journal:&ModelTurnJournal,
        checkpoint:&(dyn Fn()->Result<(),String>+Send+Sync))->Result<Option<TransferProposal>,String>{
        checkpoint()?;
        if cancel.is_cancelled()||!verifier.verify_user_turn(chat,turn){return Err("Cancelled original file operation".into())}
        if !journal.events_for(chat,turn).map_err(|_|"File operation audit unavailable")?.is_empty(){return Err("File operation proposal already exists; no replay".into())}
        record(journal,chat,turn,K::UserRequest,&self.original)?;
        let schema=json!({"type":"object","properties":{"selection":{"enum":[self.action.selection(),null]}},"required":["selection"],"additionalProperties":false});
        let prompt=format!("Select a file operation for REVIEW only. The original request asks to {} one regular file to a NEW name in its own folder. If this is the requested operation, select {}; otherwise select null. You cannot choose a path or give approval. Nothing is executed by selecting. Return only one JSON selection object. Original request (JSON string): {}",self.action.operation(),self.action.selection(),json!(self.original));
        let raw=match model.generate_step_constrained(&prompt,128,cancel,&schema){Ok(v)=>v,Err(_)=>{record(journal,chat,turn,K::GenerationFailure,"MODEL_TRANSFER_PROPOSAL_FAILED")?;checkpoint()?;if cancel.is_cancelled(){return Err("Cancelled original file operation".into())}return Err("No file operation was authorized or performed".into())}};
        record(journal,chat,turn,K::ModelOutput,&raw)?;
        checkpoint()?;
        if cancel.is_cancelled()||!verifier.verify_user_turn(chat,turn){return Err("Cancelled original file operation".into())}
        let selected=parse_exact_boundary_object(&raw,4096).ok().is_some_and(|p|p.len()==1&&p.get("selection").and_then(Value::as_str)==Some(self.action.selection()));
        if !selected{record(journal,chat,turn,K::Rejected,"MODEL_TRANSFER_NO_VALID_SELECTION")?;return Ok(None)}
        let model_sha=sha(raw.as_bytes());
        record(journal,chat,turn,K::ParsedCandidate,&json!({"selection":self.action.selection(),"model_output_sha256":model_sha,"executed":false}).to_string())?;
        Ok(Some(TransferProposal{intent:self,chat:chat.into(),turn:turn.into(),model_sha}))
    }
}
/// Non-cloneable, inert proposal. Dropping it authorizes nothing.
pub struct TransferProposal{intent:TransferIntent,chat:String,turn:String,model_sha:String}
impl TransferProposal{
    pub fn action(&self)->TransferAction{self.intent.action}
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
    /// Only the trusted native picker/review adapter supplies these values.
    /// Recheck source bytes and no-clobber before constructing the review.
    pub fn review(self,root:Arc<Dir>,source:&str,target:&str,bytes:u64,digest:&str,
        journal:&ModelTurnJournal)->Result<ReviewedModelTransfer,String>{
        self.check_proposal_audit(journal,3)?;
        if !self.matches_leaf(source)||source.eq_ignore_ascii_case(target){return Err("Selected file does not match the reviewed operation".into())}
        // The real native codec validates the exact approved leaf names before review.
        let registry=NativeToolRegistry::new();let mut catalog=ModelToolCatalog::new();
        register_file_transfer_tools(&registry,&mut catalog,Arc::clone(&root))?;
        let name=if self.action()==TransferAction::Copy{"lu_file_copy"}else{"lu_file_move"};
        let args=json!({"source_name":source,"target_name":target,"expected_source_sha256":digest});
        let allow=std::collections::BTreeSet::from([name.to_owned()]);
        let input=catalog.encode_call(&allow,name,"review-transfer",Some("review"),&args.to_string()).map_err(|_|"Invalid reviewed transfer")?.input;
        if source_digest(&root,source).ok().as_ref()!=Some(&(bytes,digest.to_owned()))||!matches!(root.symlink_metadata(target),Err(e) if e.kind()==ErrorKind::NotFound){return Err("Selected file or new filename no longer matches the reviewed operation".into())}
        let preview=json!({"action":self.action().operation(),"source":source,"target":target,"bytes":bytes,"sha256":digest,"original_request_sha256":sha(self.intent.original.as_bytes()),"model_output_sha256":self.model_sha,"input_sha256":sha(&input.bytes)});
        let preview_sha=sha(preview.to_string().as_bytes());
        record(journal,&self.chat,&self.turn,K::HostRouteSelected,&json!({"phase":"native_preview","review_sha256":preview_sha,"review":preview,"approved":false}).to_string())?;
        Ok(ReviewedModelTransfer{proposal:self,root,source:source.into(),target:target.into(),bytes,digest:digest.into(),preview_sha,input})
    }
    pub fn deny(self,journal:&ModelTurnJournal)->Result<(),String>{record(journal,&self.chat,&self.turn,K::Rejected,"HUMAN_DECLINED_FILE_SELECTION")}
}
pub struct ReviewedModelTransfer{proposal:TransferProposal,root:Arc<Dir>,source:String,target:String,bytes:u64,digest:String,preview_sha:String,input:ProtoPayload}
struct LivePolicy<'a>{policy:&'a PolicyEngine,checkpoint:&'a(dyn Fn()->Result<(),String>+Send+Sync),verifier:&'a dyn TrustedUserTurnVerifier,chat:&'a str,turn:&'a str}
impl ToolAuthorizer for LivePolicy<'_>{fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)->Result<(),AuthorizationError>{
    (self.checkpoint)().map_err(|_|AuthorizationError::new("Original file operation no longer live"))?;
    if !self.verifier.verify_user_turn(self.chat,self.turn){return Err(AuthorizationError::new("Original human turn changed"))}
    self.policy.authorize(call,desc)
}}
/// Only independently verified, persisted native outcome may become success.
/// No model-authored prose is accepted as an effect receipt.
pub struct VerifiedTransferOutcome{chat:String,turn:String,action:TransferAction,source:String,target:String,bytes:u64,receipt_sha:String,call_id:String,observed_sha:String}
impl ReviewedModelTransfer{
    pub fn deny(self,journal:&ModelTurnJournal)->Result<(),String>{record(journal,&self.proposal.chat,&self.proposal.turn,K::Rejected,"HUMAN_DECLINED_EXACT_TRANSFER")}
    /// Trusted caller invokes only after separate native confirmation. A bool is
    /// never read from a model, WebView, previous audit row or recovered task.
    pub async fn execute(self,confirmed:bool,effect_id:&str,verifier:&dyn TrustedUserTurnVerifier,
        effects:&EffectJournal,journal:&ModelTurnJournal,
        checkpoint:&(dyn Fn()->Result<(),String>+Send+Sync))->Result<Option<VerifiedTransferOutcome>,String>{
        if !confirmed{self.deny(journal)?;return Ok(None)}
        let chat=&self.proposal.chat;let turn=&self.proposal.turn;let action=self.proposal.action();
        let rows=self.proposal.check_proposal_audit(journal,4)?;
        let preview:Value=serde_json::from_str(&rows[3].payload).map_err(|_|"Invalid exact native review audit")?;
        if rows[3].kind!=K::HostRouteSelected||preview["phase"]!="native_preview"||preview["approved"]!=false||preview["review_sha256"]!=self.preview_sha||sha(preview["review"].to_string().as_bytes())!=self.preview_sha{return Err("Exact native review audit differs".into())}

        checkpoint()?;
        if !verifier.verify_user_turn(chat,turn){return Err("Original human turn changed".into())}
        if effects.get(effect_id).map_err(|_|"Effect history unavailable")?.is_some(){return Err("File operation already has an effect record; do not retry".into())}
        let task=format!("transfer-{effect_id}");let scope=format!("transfer-scope-{effect_id}");
        let registry=NativeToolRegistry::new();let mut catalog=ModelToolCatalog::new();
        register_file_transfer_tools(&registry,&mut catalog,Arc::clone(&self.root))?;
        let authority=ConversationAuthority::new();let policy=PolicyEngine::new();
        authority.open_user_task(TaskAuthority{conversation_id:chat.clone(),user_turn_id:turn.clone(),task_id:task.clone(),execution_scope_id:scope.clone(),capabilities:vec![AuthorizedCapability{tool_id:action.native_id().into(),effect:ToolEffect::WriteWorkspace}],expires_at:None},verifier).map_err(|_|"File action origin not admitted")?;
        policy.grant(CapabilityGrant{tool_id:action.native_id().into(),effect:ToolEffect::WriteWorkspace,scope_id:scope.clone(),expires_at:None}).map_err(|_|"File action grant unavailable")?;
        let live=LivePolicy{policy:&policy,checkpoint,verifier,chat,turn};
        let fence=ExactFileTransferPolicy{host_policy:&live,source_root:&self.root,tool_id:action.native_id(),source_name:&self.source,target_name:&self.target,source_sha256:&self.digest};
        record(journal,chat,turn,K::HostRouteSelected,&json!({"phase":"native_confirmation","approved":true,"review_sha256":self.preview_sha,"effect_call_id":effect_id,"native_tool_id":action.native_id(),"authority_task":task,"input_sha256":sha(&self.input.bytes)}).to_string())?;
        let output=authority.dispatch_bound_durable(&registry,effects,ToolInvocation{call_id:effect_id.into(),tool_id:action.native_id().into(),workspace_id:Some(scope.clone()),input:self.input.clone()},BoundCallOrigin{conversation_id:chat.clone(),user_turn_id:turn.clone(),task_id:task.clone()},&fence).await;
        let _=authority.revoke_task(&task);let _=policy.revoke_scope(&scope);
        let output=match output{Ok(v)=>v,Err(_)=>{record(journal,chat,turn,K::Rejected,"TRANSFER_UNCONFIRMED_NO_REPLAY")?;return Err("The file operation did not return a verified result. Inspect the selected folder before retrying.".into())}};
        let row=effects.get(effect_id).map_err(|_|"Effect history unavailable")?.ok_or("Missing durable file receipt")?;
        let raw=row.receipt.as_ref().ok_or("Missing durable file receipt")?;
        let durable=wire::decode_reply(raw).map_err(|_|"Invalid durable file receipt")?;
        let decoded=proto::FileTransferResult::decode(output.payload.bytes.as_slice()).map_err(|_|"Invalid file result")?;
        let target=source_digest(&self.root,&self.target).ok();
        let source_preserved=if action==TransferAction::Move{matches!(self.root.symlink_metadata(&self.source),Err(e) if e.kind()==ErrorKind::NotFound)}else{source_digest(&self.root,&self.source).ok().as_ref()==Some(&(self.bytes,self.digest.clone()))};
        let correct=row.state==EffectState::Applied&&row.tool_id==action.native_id()&&durable.call_id==effect_id&&durable.output_type_url==OUTPUT&&output.payload.type_url==OUTPUT&&matches!(durable.outcome,Some(proto::tool_reply::Outcome::Output(ref bytes)) if bytes==&output.payload.bytes)&&decoded.source_name==self.source&&decoded.target_name==self.target&&decoded.bytes_written==self.bytes&&decoded.sha256==self.digest&&decoded.source_removed==(action==TransferAction::Move)&&target.as_ref()==Some(&(self.bytes,self.digest.clone()))&&source_preserved;
        if !correct{
            if row.state==EffectState::Applied{effects.record_independent_outcome(effect_id,raw,IndependentOutcomeVerdict::Conflict,"FILE_TRANSFER_DIRECT_V1",None).map_err(|_|"Could not record transfer conflict")?;}
            record(journal,chat,turn,K::Rejected,"TRANSFER_INDEPENDENT_OUTCOME_CONFLICT")?;
            return Err("The file operation needs manual reconciliation. Its result could not be verified; no retry was performed.".into())
        }
        // Checked target content AND the original's retained/removed state.
        let observed=json!({"action":action.operation(),"source":self.source,"target":self.target,"bytes":self.bytes,"sha256":self.digest,"source_removed":action==TransferAction::Move});
        let observed_sha=sha(observed.to_string().as_bytes());let receipt_sha=sha(raw);
        effects.record_independent_outcome(effect_id,raw,IndependentOutcomeVerdict::Verified,"FILE_TRANSFER_DIRECT_V1",Some(&observed_sha)).map_err(|_|"Verified file result could not be journaled")?;
        if !effects.independent_verified_receipt(effect_id,action.native_id(),&receipt_sha).map_err(|_|"Verified file receipt unavailable")?{return Err("Verified file receipt unavailable".into())}
        record(journal,chat,turn,K::ToolFeedback,&json!({"effect_call_id":effect_id,"native_tool_id":action.native_id(),"receipt_sha256":receipt_sha,"observed_sha256":observed_sha,"review_sha256":self.preview_sha,"observed":observed}).to_string())?;
        Ok(Some(VerifiedTransferOutcome{chat:chat.clone(),turn:turn.clone(),action,source:self.source,target:self.target,bytes:self.bytes,receipt_sha,call_id:effect_id.into(),observed_sha}))
    }
}
impl VerifiedTransferOutcome{
    /// Post-effect model continuation sees only a verified status, not a grant
    /// or a callable tool. The host renders factual output from the receipt;
    /// abstention/invalid output gets distinct fallback attribution, no replay.
    pub fn present(self,model:&mut dyn ModelStepGenerator,cancel:&Cancellation,journal:&ModelTurnJournal,effects:&EffectJournal,
        checkpoint:&(dyn Fn()->Result<(),String>+Send+Sync))->Result<String,String>{
        checkpoint()?;if cancel.is_cancelled(){return Err("Cancelled after file operation; inspect the selected folder".into())}
        if !effects.independent_verified_receipt(&self.call_id,self.action.native_id(),&self.receipt_sha).map_err(|_|"Verified file receipt unavailable")?{return Err("Verified file receipt unavailable".into())}
        let rows=journal.events_for(&self.chat,&self.turn).map_err(|_|"File operation audit unavailable")?;
        let feedback=rows.last().filter(|r|r.kind==K::ToolFeedback).ok_or("Original verified feedback unavailable")?;
        let facts:Value=serde_json::from_str(&feedback.payload).map_err(|_|"Invalid verified feedback")?;
        if rows.len()!=6||facts["effect_call_id"]!=self.call_id||facts["receipt_sha256"]!=self.receipt_sha||facts["observed_sha256"]!=self.observed_sha{return Err("Original verified feedback differs".into())}
        let schema=json!({"type":"object","properties":{"status":{"enum":["verified_completion","unverified"]}},"required":["status"],"additionalProperties":false});
        let prompt=format!("The independently checked native file operation succeeded. The host verified the new file contents and the original name state. Do not request a tool or invent a fact. Return only JSON with status verified_completion if reporting this verified receipt, otherwise status unverified. Receipt facts (JSON): {}",json!({"operation":self.action.operation(),"bytes":self.bytes,"original_removed":self.action==TransferAction::Move}));
        let selected=match model.generate_step_constrained(&prompt,128,cancel,&schema){Ok(raw)=>{record(journal,&self.chat,&self.turn,K::ModelOutput,&raw)?;parse_exact_boundary_object(&raw,4096).ok().is_some_and(|p|p.len()==1&&p.get("status").and_then(Value::as_str)==Some("verified_completion"))},Err(_)=>{record(journal,&self.chat,&self.turn,K::GenerationFailure,"TRANSFER_CONTINUATION_FAILED")?;false}};
        checkpoint()?;if cancel.is_cancelled(){return Err("Cancelled after file operation; inspect the selected folder".into())}
        if !selected{record(journal,&self.chat,&self.turn,K::Rejected,"TRANSFER_CONTINUATION_UNVERIFIED")?;}
        let answer=format!("{}{} {} to {} in the selected folder ({} bytes). {}",if selected{""}else{"I couldn't verify my generated explanation. The native receipt confirms: "},if self.action==TransferAction::Copy{"Copied"}else{"Moved"},crate::evidence_grounding::literal(&self.source),crate::evidence_grounding::literal(&self.target),self.bytes,if self.action==TransferAction::Copy{"The original was verified unchanged."}else{"The original name was verified absent."});
        record(journal,&self.chat,&self.turn,K::EvidencePresentation,&json!({"origin":if selected{"model_selected_verified_effect_host_rendered"}else{"host_verified_effect_fallback"},"effect_call_id":self.call_id,"receipt_sha256":self.receipt_sha,"observed_sha256":self.observed_sha,"answer":answer}).to_string())?;
        Ok(answer)
    }
}
