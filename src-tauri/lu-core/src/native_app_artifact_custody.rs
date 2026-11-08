//! E1-P04 producer attribution and semantic publication.
//!
//! Actual reviewed OS worker -> host-owned STDOUT pipe -> private bounded
//! artifact bytes -> independent DOCX/XLSX parser -> host-owned Dir handle
//! create-new publication -> durable process/write receipts.
//!
//! No worker-created destination path, no model-supplied executable/argv,
//! no shell/network, and no automatic tool registration. A separate exact
//! ExecuteCode and WriteWorkspace user-turn grant are required. This is
//! deliberately a narrow first-party, fixed-content worker, NOT a general
//! untrusted third-party application sandbox.
use std::{
    collections::BTreeMap,ffi::OsString,
    sync::{Arc,Mutex},
};
use cap_std::fs::Dir;
use serde_json::json;
use sha2::{Digest,Sha256};
use crate::{
    contracts::{ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    journal::{EffectJournal,EffectState,PrepareOutcome},
    native_app_admission::inspect_host_reviewed_code_application,
    native_app_catalog::HostReviewedApplicationCatalog,
    native_app_execution::{E1_VERSION_MARKER_BYTES},
    native_artifact_inspect::inspect_bytes,
    native_atomic_write::{create_new_atomic_in_dir,WriteReceipt},
    native_background_process::{BackgroundProcessSupervisor,CapturedArtifact,TrustedWorkerPlan},
    native_reader_docx::read_docx_bytes,
    native_reader_office::read_xlsx_sheet,
    tasks::{TaskState,TaskStore},
};

pub const APP_ID:&str="lu-e1-artifact-worker";
pub const EXEC_TOOL_ID:&str="lu.e1.artifact.execute";
pub const PUBLISH_TOOL_ID:&str="lu.e1.artifact.publish";
pub const WORKER_MODE:&str="--lu-e1-artifact-one-shot";
pub const INPUT_TYPE:&str="type.googleapis.com/lu.e1.FixedArtifact";
pub const PUBLISH_TYPE:&str="type.googleapis.com/lu.e1.FixedPublish";
pub const EXEC_REQUEST:&[u8]=b"lu.e1.fixed-artifact.v1";
pub const PUBLISH_REQUEST:&[u8]=b"lu.e1.fixed-artifact-publish.v1";
pub const OUTPUT_TYPE:&str="type.googleapis.com/lu.e1.ArtifactCustodyReceipt";
pub const FIXED_DOCX_TEXT:&str="Lu E1 artifact custody verified\n";
pub const FIXED_XLSX_TEXT:&str="\n[Sheet: E1]\nA1=Status\nB1=Reviewed\nA2=Frames\nB2=48\n";

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ArtifactKind{Docx,Xlsx}
impl ArtifactKind{
    fn ext(self)->&'static str{
        match self{Self::Docx=>"docx",Self::Xlsx=>"xlsx"}
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CustodyError{
    InvalidHostApproval,NotAuthorized,NotReviewed,BinaryChanged,
    Busy,DuplicateEffect,JournalUnavailable,WorkerFailed,WorkerUncertain,
    WrongOrigin,OutputMissing,OutputMismatch,PublishUncertain,
    StoreUnavailable,LockUnavailable,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct HostApprovedArtifact {
    pub kind:ArtifactKind,
    /// Host-selected create-new filename; never worker/model supplied.
    pub target_name:String,
    /// Host-generated unpredictable stage identifier (>=16 chars).
    pub stage_id:String,
    /// Human-approved exact expected doc text or sheet/cell rendering.
    pub expected_semantics:String,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ArtifactCustodyReceipt {
    pub process_call_id:String,
    pub publish_call_id:String,
    pub format:String,
    pub target:String,
    pub bytes:u64,
    pub sha256:String,
    pub producer_attributed_to_reviewed_worker_pipe:bool,
    pub independent_semantics_verified:bool,
    pub worker_process_reaped:bool,
}
#[derive(Clone)]
struct Pending{
    origin:BoundCallOrigin,
    execute:ToolInvocation,
    publish:ToolInvocation,
    publish_desc:ToolDescriptor,
    approval:HostApprovedArtifact,
    revoked:bool,
}
pub struct E1ArtifactCustodyBroker{
    authority:ConversationAuthority,
    workers:Arc<BackgroundProcessSupervisor>,
    tasks:Arc<TaskStore>,
    effects:Arc<EffectJournal>,
    root:Arc<Dir>,
    active:Arc<Mutex<BTreeMap<String,Pending>>>,
}
fn valid_id(v:&str)->bool{
    !v.is_empty()&&v.len()<=128&&v.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
}
fn valid_target(t:&str,kind:ArtifactKind)->bool{
    t.len()>kind.ext().len()+1&&t.len()<=128&&
        t.ends_with(&format!(".{}",kind.ext()))&&
        !t.starts_with('.')&&t.bytes().all(|b|
            b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'))
}
fn valid_stage(s:&str)->bool{
    (16..=128).contains(&s.len())&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
}
fn valid_invocations(
    origin:&BoundCallOrigin,exec:&ToolInvocation,exec_desc:&ToolDescriptor,
    publish:&ToolInvocation,publish_desc:&ToolDescriptor,
    approval:&HostApprovedArtifact,
)->bool{
    valid_id(&origin.task_id)&&valid_id(&exec.call_id)
        &&valid_id(&publish.call_id)&&exec.call_id!=publish.call_id
        &&exec.tool_id==EXEC_TOOL_ID&&exec_desc.id==EXEC_TOOL_ID
        &&exec_desc.effect==ToolEffect::ExecuteCode
        &&exec_desc.input_type_url==INPUT_TYPE
        &&exec_desc.output_type_url==OUTPUT_TYPE
        &&exec.input.type_url==INPUT_TYPE
        &&exec.input.bytes.as_slice()==EXEC_REQUEST
        &&publish.tool_id==PUBLISH_TOOL_ID
        &&publish_desc.id==PUBLISH_TOOL_ID
        &&publish_desc.effect==ToolEffect::WriteWorkspace
        &&publish_desc.input_type_url==PUBLISH_TYPE
        &&publish_desc.output_type_url==OUTPUT_TYPE
        &&publish.input.type_url==PUBLISH_TYPE
        &&publish.input.bytes.as_slice()==PUBLISH_REQUEST
        &&exec.workspace_id==publish.workspace_id
        &&exec.workspace_id.as_deref().is_some_and(valid_id)
        &&valid_target(&approval.target_name,approval.kind)
        &&valid_stage(&approval.stage_id)
        &&approval.expected_semantics==match approval.kind{
            ArtifactKind::Docx=>FIXED_DOCX_TEXT,
            ArtifactKind::Xlsx=>FIXED_XLSX_TEXT,
        }
}
pub fn independently_validate_office(
    kind:ArtifactKind,bytes:&[u8],expected_semantics:&str
)->Result<String,CustodyError>{
    // Actual structure check plus independent Office XML parsers; a ZIP
    // signature or process exit code alone is never enough.
    let inspected=inspect_bytes(bytes,kind.ext(),"")
        .map_err(|_|CustodyError::OutputMismatch)?;
    if inspected.bytes!=bytes.len() as u64 {
        return Err(CustodyError::OutputMismatch)
    }
    let read=match kind{
        ArtifactKind::Docx=>read_docx_bytes(bytes,8192),
        ArtifactKind::Xlsx=>read_xlsx_sheet(bytes,8192,Some("E1")),
    }.map_err(|_|CustodyError::OutputMismatch)?;
    if read.truncated||read.text!=expected_semantics{
        return Err(CustodyError::OutputMismatch)
    }
    Ok(format!("{:x}",Sha256::digest(bytes)))
}
impl E1ArtifactCustodyBroker{
    pub fn new(
        authority:ConversationAuthority,
        workers:Arc<BackgroundProcessSupervisor>,
        tasks:Arc<TaskStore>,
        effects:Arc<EffectJournal>,
        root:Arc<Dir>,
    )->Result<Self,CustodyError>{
        let active:Arc<Mutex<BTreeMap<String,Pending>>>=Arc::new(
            Mutex::new(BTreeMap::new())
        );
        let on_revoke=Arc::clone(&active);
        let on_workers=Arc::clone(&workers);
        let on_effects=Arc::clone(&effects);
        authority.on_task_revoked(Arc::new(move|task_id|{
            if let Ok(mut map)=on_revoke.lock(){
                if let Some(p)=map.get_mut(task_id){
                    p.revoked=true;
                    let _=on_effects.mark_uncertain(&p.execute.call_id);
                    if on_workers.cancel_owned(
                        &p.execute.call_id,
                        p.execute.workspace_id.as_deref().unwrap_or("")
                    ).is_ok(){
                        let _=on_workers.discard_local_output_artifact(
                            &p.execute.call_id,
                            p.execute.workspace_id.as_deref().unwrap_or("")
                        );
                        map.remove(task_id);
                    }
                }
            }
        })).map_err(|_|CustodyError::LockUnavailable)?;
        Ok(Self{authority,workers,tasks,effects,root,active})
    }

    /// Trusted host-owned, exact fixed-content operation. Never used by a
    /// generic model-supplied command or an unreviewed plugin.
    #[allow(clippy::too_many_arguments)]
    pub fn launch(
        &self,catalog:&HostReviewedApplicationCatalog,
        origin:&BoundCallOrigin,
        exec:&ToolInvocation,exec_desc:&ToolDescriptor,
        publish:&ToolInvocation,publish_desc:&ToolDescriptor,
        approval:HostApprovedArtifact,
        policy:&dyn ToolAuthorizer,
    )->Result<(),CustodyError>{
        if !valid_invocations(origin,exec,exec_desc,publish,publish_desc,&approval){
            return Err(CustodyError::InvalidHostApproval)
        }
        let mut active=self.active.lock()
            .map_err(|_|CustodyError::LockUnavailable)?;
        if active.contains_key(&origin.task_id){return Err(CustodyError::Busy)}
        let mut reviewed=catalog.host_reviewed_entry(APP_ID)
            .ok_or(CustodyError::NotReviewed)?;
        if exec.workspace_id.as_deref()!=Some(reviewed.workspace_id.as_str()){
            return Err(CustodyError::InvalidHostApproval)
        }
        reviewed.tool_id=EXEC_TOOL_ID.into();
        // Both grants must be live *before* committing an executing side effect.
        self.authority.authorize_worker_call(
            origin,exec,exec_desc,policy
        ).map_err(|_|CustodyError::NotAuthorized)?;
        self.authority.authorize_worker_call(
            origin,publish,publish_desc,policy
        ).map_err(|_|CustodyError::NotAuthorized)?;
        inspect_host_reviewed_code_application(
            &reviewed,origin,exec,exec_desc,&self.authority,policy
        ).map_err(|_|CustodyError::BinaryChanged)?;
        match self.effects.prepare(exec)
            .map_err(|_|CustodyError::JournalUnavailable)?{
            PrepareOutcome::New=>{},
            PrepareOutcome::Existing(_)=>return Err(CustodyError::DuplicateEffect),
        }
        self.effects.mark_executing(&exec.call_id)
            .map_err(|_|CustodyError::JournalUnavailable)?;
        let plan=TrustedWorkerPlan{
            task_id:exec.call_id.clone(),
            workspace_id:reviewed.workspace_id,
            executable:reviewed.executable,
            expected_sha256:reviewed.expected_sha256,
            args:vec![
                OsString::from(WORKER_MODE),
                OsString::from(approval.kind.ext())
            ],
            progress_protocol:true,
            max_runtime_seconds:Some(15),
        };
        if self.workers.start_host_approved(plan).is_err(){
            let _=self.effects.mark_uncertain(&exec.call_id);
            return Err(CustodyError::WorkerFailed)
        }
        active.insert(origin.task_id.clone(),Pending{
            origin:origin.clone(),
            execute:exec.clone(),
            publish:publish.clone(),
            publish_desc:publish_desc.clone(),
            approval,revoked:false
        });
        Ok(())
    }

    fn finalize(
        &self,p:&Pending,artifact:&CapturedArtifact,policy:&dyn ToolAuthorizer
    )->Result<ArtifactCustodyReceipt,CustodyError>{
        if p.revoked{return Err(CustodyError::NotAuthorized)}
        if artifact.format!=p.approval.kind.ext()
            ||artifact.bytes.len()>1024*1024
            ||format!("{:x}",Sha256::digest(&artifact.bytes))!=artifact.sha256{
            return Err(CustodyError::OutputMismatch)
        }
        let expected=format!("{:x}",Sha256::digest(E1_VERSION_MARKER_BYTES));
        let proof=self.tasks.verified_result(
            &p.execute.call_id,p.execute.workspace_id.as_deref().unwrap_or("")
        ).map_err(|_|CustodyError::StoreUnavailable)?
            .ok_or(CustodyError::OutputMissing)?;
        if proof.kind!="sha256_scan"||proof.sha256!=expected
            ||proof.bytes!=E1_VERSION_MARKER_BYTES.len() as u64{
            return Err(CustodyError::OutputMismatch)
        }
        let sha=independently_validate_office(
            p.approval.kind,&artifact.bytes,&p.approval.expected_semantics
        )?;
        // This short, bounded local operation is held inside the live
        // original human-turn WriteWorkspace grant until journal publication.
        self.authority.with_authorized_worker_effect(
            &p.origin,&p.publish,&p.publish_desc,policy,||{
                match self.effects.prepare(&p.publish)
                    .map_err(|_|CustodyError::JournalUnavailable)?{
                    PrepareOutcome::New=>{},
                    PrepareOutcome::Existing(_)=>return Err(CustodyError::DuplicateEffect),
                }
                self.effects.mark_executing(&p.publish.call_id)
                    .map_err(|_|CustodyError::JournalUnavailable)?;
                let written:WriteReceipt=match create_new_atomic_in_dir(
                    &self.root,&p.approval.target_name,&p.approval.stage_id,
                    &artifact.bytes
                ){
                    Ok(r)=>r,
                    Err(_)=>{
                        let _=self.effects.mark_uncertain(&p.publish.call_id);
                        return Err(CustodyError::PublishUncertain)
                    }
                };
                if written.sha256!=sha||
                    written.bytes_written!=artifact.bytes.len(){
                    let _=self.effects.mark_uncertain(&p.publish.call_id);
                    return Err(CustodyError::PublishUncertain)
                }
                // Re-read from the opened, host-owned directory capability.
                // Do not rely on a worker-provided hash or a pathname witness.
                let mut published=self.root.open(&p.approval.target_name)
                    .map_err(|_|CustodyError::PublishUncertain)?;
                let mut actual=Vec::new();
                use std::io::Read;
                published.take(1024*1024+1).read_to_end(&mut actual)
                    .map_err(|_|CustodyError::PublishUncertain)?;
                if actual!=artifact.bytes{
                    let _=self.effects.mark_uncertain(&p.publish.call_id);
                    return Err(CustodyError::PublishUncertain)
                }
                let receipt=ArtifactCustodyReceipt{
                    process_call_id:p.execute.call_id.clone(),
                    publish_call_id:p.publish.call_id.clone(),
                    format:p.approval.kind.ext().into(),
                    target:written.target,
                    bytes:written.bytes_written as u64,
                    sha256:sha.clone(),
                    producer_attributed_to_reviewed_worker_pipe:true,
                    independent_semantics_verified:true,
                    worker_process_reaped:true,
                };
                let raw=serde_json::to_vec(&json!({
                    "schema":"lu-e1-artifact-publish-receipt-v1",
                    "processCallId":receipt.process_call_id,
                    "publishCallId":receipt.publish_call_id,
                    "sha256":receipt.sha256,
                    "bytes":receipt.bytes,
                    "format":receipt.format,
                    "producerPipeOwned":true,
                    "semanticContentVerified":true,
                    "actualExternalThirdPartyApplication":false,
                })).map_err(|_|CustodyError::JournalUnavailable)?;
                self.effects.mark_applied(&p.publish.call_id,&raw)
                    .map_err(|_|CustodyError::PublishUncertain)?;
                Ok(receipt)
            }
        ).map_err(|_|CustodyError::NotAuthorized)?
    }

    pub fn poll(&self,policy:&dyn ToolAuthorizer)
        ->Result<usize,CustodyError>{
        let mut active=self.active.lock()
            .map_err(|_|CustodyError::LockUnavailable)?;
        let finished=self.workers.poll()
            .map_err(|_|CustodyError::WorkerUncertain)?;
        let mut retired=Vec::new();
        for (task_id,p) in active.iter(){
            let record=self.tasks.get(&p.execute.call_id)
                .map_err(|_|CustodyError::StoreUnavailable)?
                .ok_or(CustodyError::StoreUnavailable)?;
            if matches!(record.state,TaskState::Pending|TaskState::Running|
                TaskState::Paused){continue}
            let scope=p.execute.workspace_id.as_deref().unwrap_or("");
            if p.revoked{
                let _=self.effects.mark_uncertain(&p.execute.call_id);
                let _=self.workers.discard_local_output_artifact(
                    &p.execute.call_id,scope
                );
                retired.push(task_id.clone());
                continue
            }
            if record.state==TaskState::Completed{
                let artifact=self.workers.take_local_output_artifact(
                    &p.execute.call_id,scope
                ).map_err(|_|CustodyError::WorkerUncertain)?;
                let result=artifact.as_ref().ok_or(CustodyError::OutputMissing)
                    .and_then(|a|self.finalize(p,a,policy));
                match result{
                    Ok(receipt)=>{
                        let raw=serde_json::to_vec(&json!({
                            "schema":"lu-e1-artifact-worker-custody-v1",
                            "processCallId":receipt.process_call_id,
                            "publishCallId":receipt.publish_call_id,
                            "format":receipt.format,
                            "sha256":receipt.sha256,
                            "bytes":receipt.bytes,
                            "reviewedWorkerStdoutPipe":true,
                            "workerProcessReaped":true,
                            "artifactProducerAttributed":true,
                            "independentSemanticsVerified":true,
                            "actualExternalThirdPartyApplication":false,
                        })).map_err(|_|CustodyError::JournalUnavailable)?;
                        self.effects.mark_applied(&p.execute.call_id,&raw)
                            .map_err(|_|CustodyError::JournalUnavailable)?;
                    },
                    Err(_)=>{
                        let _=self.effects.mark_uncertain(&p.execute.call_id);
                    }
                }
            }else{
                let _=self.effects.mark_uncertain(&p.execute.call_id);
                let _=self.workers.discard_local_output_artifact(
                    &p.execute.call_id,scope
                );
            }
            retired.push(task_id.clone());
        }
        for id in retired{active.remove(&id);}
        Ok(finished)
    }

    pub fn active_count(&self)->Result<usize,CustodyError>{
        Ok(self.active.lock()
            .map_err(|_|CustodyError::LockUnavailable)?.len())
    }

    pub fn cancel(&self,origin:&BoundCallOrigin)->Result<(),CustodyError>{
        let mut active=self.active.lock()
            .map_err(|_|CustodyError::LockUnavailable)?;
        let p=active.get_mut(&origin.task_id)
            .ok_or(CustodyError::WrongOrigin)?;
        if p.origin!=*origin{return Err(CustodyError::WrongOrigin)}
        p.revoked=true;
        let _=self.effects.mark_uncertain(&p.execute.call_id);
        let result=self.workers.cancel_owned(
            &p.execute.call_id,p.execute.workspace_id.as_deref().unwrap_or("")
        );
        if result.is_err(){return Err(CustodyError::WorkerUncertain)}
        active.remove(&origin.task_id);
        Ok(())
    }
}
impl Drop for E1ArtifactCustodyBroker{
    fn drop(&mut self){
        if let Ok(mut active)=self.active.lock(){
            for (_,p) in std::mem::take(&mut *active){
                let scope=p.execute.workspace_id.as_deref().unwrap_or("");
                let _=self.effects.mark_uncertain(&p.execute.call_id);
                let _=self.workers.cancel_owned(&p.execute.call_id,scope);
                let _=self.workers.discard_local_output_artifact(
                    &p.execute.call_id,scope
                );
            }
        }
    }
}
