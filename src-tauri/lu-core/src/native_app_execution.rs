//! E1-P03: exact reviewed, one-shot application worker execution.
//!
//! This is a REAL host-supervised process, not an unrestricted shell, an
//! application API bridge, or a model-supplied executable/argv. The ONLY
//! admitted initial operation is Lu's reviewed, read-only version-protocol
//! worker with fixed argv, no stdin/environment or writable output and the
//! existing Windows Job Object / Linux process-group supervisor.
//!
//! We require a separate live ExecuteCode capability and host policy even
//! if E1-P02 advertised an app under a ReadWorkspace discovery permission.
//! Journal EXECUTING is durable before OS launch. Cancellations, revoked
//! grants, missing/failed protocol receipts and crashes are UNCERTAIN, never
//! replayed or falsely reported as verified application output.
//!
//! A child that emits the known protocol SHA confirms only protocol delivery
//! and process exit. It does NOT prove genuine external application features,
//! network isolation, version-signature trust, or user-requested artifact
//! correctness (E1-P04). External arbitrary CLI actions remain DISABLED.
use std::{
    collections::BTreeMap,ffi::OsString,sync::{Arc,Mutex},
};
use serde_json::json;
use sha2::{Digest,Sha256};
use crate::{
    contracts::{ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    journal::{EffectJournal,EffectState,PrepareOutcome},
    native_app_admission::inspect_host_reviewed_code_application,
    native_app_catalog::HostReviewedApplicationCatalog,
    native_background_process::{BackgroundProcessSupervisor,TrustedWorkerPlan},
    tasks::{TaskState,TaskStore},
};

pub const E1_REVIEWED_VERSION_APP_ID:&str="lu-e1-version-worker";
pub const E1_VERSION_EXECUTE_TOOL_ID:&str="lu.e1.version.execute";
pub const E1_VERSION_REQUEST_TYPE:&str="type.googleapis.com/lu.e1.ReviewedVersionRequest";
pub const E1_VERSION_RESULT_TYPE:&str="type.googleapis.com/lu.e1.VersionOperationReceipt";
pub const E1_VERSION_REQUEST_BYTES:&[u8]=b"lu.e1.one_shot.version.v1";
pub const E1_WORKER_MODE:&str="--lu-e1-version-one-shot";
/// The fixed host-owned marker is not secret and cannot prove semantic
/// application results. It merely pins the worker progress-protocol receipt.
pub const E1_VERSION_MARKER_BYTES:&[u8]=b"Lu-E1-reviewed-version-worker-v1";

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum E1ExecutionError{
    NotReviewed,
    InvalidOperation,
    Unauthorized,
    BinaryChanged,
    DuplicateOrAmbiguousEffect,
    TaskAlreadyRunning,
    LockUnavailable,
    JournalUnavailable,
    WorkerStartFailed,
    WorkerStatusUnavailable,
    WrongCancellationOrigin,
    WorkerCancellationUncertain,
}

/// State is retained ONLY within the host owner; no model or WebView may
/// supply a PID, operation ID, argv, cwd, environment or resource authority.
#[derive(Debug,Clone)]
struct RunningOperation{
    call_id:String,
    workspace_id:String,
    origin:BoundCallOrigin,
}
#[derive(Default)]
struct ExecutionGate{
    by_origin_task:BTreeMap<String,RunningOperation>,
}

pub struct E1ReviewedApplicationExecutor{
    authority:ConversationAuthority,
    workers:Arc<BackgroundProcessSupervisor>,
    tasks:Arc<TaskStore>,
    effects:Arc<EffectJournal>,
    gate:Arc<Mutex<ExecutionGate>>,
}
fn valid_worker_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
}
fn reviewed_payload(call:&ToolInvocation,descriptor:&ToolDescriptor)->bool{
    descriptor.id==E1_VERSION_EXECUTE_TOOL_ID
       &&descriptor.effect==ToolEffect::ExecuteCode
       &&descriptor.input_type_url==E1_VERSION_REQUEST_TYPE
       &&descriptor.output_type_url==E1_VERSION_RESULT_TYPE
       &&call.tool_id==E1_VERSION_EXECUTE_TOOL_ID
       &&call.input.type_url==E1_VERSION_REQUEST_TYPE
       &&call.input.bytes.as_slice()==E1_VERSION_REQUEST_BYTES
       &&valid_worker_id(&call.call_id)
       &&call.workspace_id.as_deref().is_some_and(valid_worker_id)
}

impl E1ReviewedApplicationExecutor{
    /// Register one synchronous revocation fence before any work begins.
    /// A revocation blocks until its actual owned process cancellation was
    /// attempted. If the OS cannot prove reaping, the journal remains
    /// UNCERTAIN and cannot be retried automatically.
    pub fn new(
        authority:ConversationAuthority,
        workers:Arc<BackgroundProcessSupervisor>,
        tasks:Arc<TaskStore>,
        effects:Arc<EffectJournal>,
    )->Result<Self,E1ExecutionError>{
        let gate=Arc::new(Mutex::new(ExecutionGate::default()));
        let gate_on_revoke=Arc::clone(&gate);
        let workers_on_revoke=Arc::clone(&workers);
        let journal_on_revoke=Arc::clone(&effects);
        authority.on_task_revoked(Arc::new(move |task_id|{
            if let Ok(mut state)=gate_on_revoke.lock(){
                if let Some(entry)=state.by_origin_task.remove(task_id){
                    let _=workers_on_revoke
                        .cancel_owned(&entry.call_id,&entry.workspace_id);
                    let _=journal_on_revoke.mark_uncertain(&entry.call_id);
                }
            }
        })).map_err(|_|E1ExecutionError::LockUnavailable)?;
        Ok(Self{authority,workers,tasks,effects,gate})
    }

    /// Launch ONE fixed-operation trusted worker. Trusted host only.
    /// This requires a separate ExecuteCode grant (not the E1-P02 read grant)
    /// and a reviewed host-selected app executable. No arbitrary argv passed.
    ///
    /// The gate is shared with synchronous authority-revocation callbacks:
    /// if a revoke races admission/launch, it terminates the newly owned
    /// process before the revocation notification finishes.
    pub fn launch_version_probe(
        &self,
        catalog:&HostReviewedApplicationCatalog,
        origin:&BoundCallOrigin,
        call:&ToolInvocation,
        descriptor:&ToolDescriptor,
        host_policy:&dyn ToolAuthorizer,
    )->Result<(),E1ExecutionError>{
        if !reviewed_payload(call,descriptor){
            return Err(E1ExecutionError::InvalidOperation)
        }
        let mut guard=self.gate.lock()
            .map_err(|_|E1ExecutionError::LockUnavailable)?;
        if guard.by_origin_task.contains_key(&origin.task_id){
            return Err(E1ExecutionError::TaskAlreadyRunning)
        }
        let mut reviewed=catalog.host_reviewed_entry(E1_REVIEWED_VERSION_APP_ID)
            .ok_or(E1ExecutionError::NotReviewed)?;
        // The read-only discovery tool and this ExecuteCode action have
        // DIFFERENT grants; inherit binary review identity, not permission.
        if reviewed.workspace_id!=call.workspace_id.as_deref()
            .unwrap_or_default(){
            return Err(E1ExecutionError::InvalidOperation)
        }
        reviewed.tool_id=E1_VERSION_EXECUTE_TOOL_ID.into();
        self.authority.authorize_worker_call(
            origin,call,descriptor,host_policy
        ).map_err(|_|E1ExecutionError::Unauthorized)?;
        inspect_host_reviewed_code_application(
            &reviewed,origin,call,descriptor,&self.authority,host_policy
        ).map_err(|_|E1ExecutionError::BinaryChanged)?;

        // No duplicate ID is ever a retry, even when it was already
        // PREPARED or reported FAILED before an operator intervenes.
        match self.effects.prepare(call)
            .map_err(|_|E1ExecutionError::JournalUnavailable)?{
            PrepareOutcome::New=>{},
            PrepareOutcome::Existing(_)=>return Err(
                E1ExecutionError::DuplicateOrAmbiguousEffect
            )
        }
        self.effects.mark_executing(&call.call_id)
            .map_err(|_|E1ExecutionError::JournalUnavailable)?;

        let plan=TrustedWorkerPlan{
            task_id:call.call_id.clone(),
            workspace_id:reviewed.workspace_id.clone(),
            executable:reviewed.executable,
            expected_sha256:reviewed.expected_sha256,
            args:vec![OsString::from(E1_WORKER_MODE)],
            progress_protocol:true,
            max_runtime_seconds:Some(15),
        };
        if self.workers.start_host_approved(plan).is_err(){
            // Spawn failure may be ambiguous, never turn it into success or
            // blindly retry a prepared/executing call ID.
            let _=self.effects.mark_uncertain(&call.call_id);
            return Err(E1ExecutionError::WorkerStartFailed)
        }
        guard.by_origin_task.insert(origin.task_id.clone(),RunningOperation{
            call_id:call.call_id.clone(),workspace_id:reviewed.workspace_id,
            origin:origin.clone()
        });
        Ok(())
    }

    /// The process supervisor is authoritative for exit/timeout/reaping.
    /// A matching protocol receipt confirms completion of THIS fixed worker
    /// action, never correctness of an external document/API/GUI output.
    pub fn poll(&self)->Result<usize,E1ExecutionError>{
        let mut guard=self.gate.lock()
            .map_err(|_|E1ExecutionError::LockUnavailable)?;
        let finished=self.workers.poll()
            .map_err(|_|E1ExecutionError::WorkerStatusUnavailable)?;
        let mut done=Vec::new();
        for (task,entry) in &guard.by_origin_task{
            let record=self.tasks.get(&entry.call_id)
                .map_err(|_|E1ExecutionError::WorkerStatusUnavailable)?
                .ok_or(E1ExecutionError::WorkerStatusUnavailable)?;
            if matches!(record.state,TaskState::Running|TaskState::Pending|
                TaskState::Paused){continue}
            if record.state==TaskState::Completed{
                let proof=self.tasks.verified_result(
                    &entry.call_id,&entry.workspace_id
                ).map_err(|_|E1ExecutionError::WorkerStatusUnavailable)?;
                let expected=format!("{:x}",Sha256::digest(E1_VERSION_MARKER_BYTES));
                if proof.as_ref().is_some_and(|p|
                    p.kind=="sha256_scan"&&p.sha256==expected
                    &&p.bytes==E1_VERSION_MARKER_BYTES.len() as u64){
                    let receipt=json!({
                        "schema":"lu-e1-reviewed-worker-receipt-v1",
                        "callId":entry.call_id,
                        "workerProcessReaped":true,
                        "fixedMode":E1_WORKER_MODE,
                        "protocolDigestMatched":true,
                        "independentApplicationOutputVerified":false,
                        "reviewedOperation":"read_only_version_marker",
                    });
                    let bytes=serde_json::to_vec(&receipt)
                        .map_err(|_|E1ExecutionError::JournalUnavailable)?;
                    self.effects.mark_applied(&entry.call_id,&bytes)
                        .map_err(|_|E1ExecutionError::JournalUnavailable)?;
                }else{
                    self.effects.mark_uncertain(&entry.call_id)
                        .map_err(|_|E1ExecutionError::JournalUnavailable)?;
                }
            }else{
                self.effects.mark_uncertain(&entry.call_id)
                    .map_err(|_|E1ExecutionError::JournalUnavailable)?;
            }
            done.push(task.clone());
        }
        for task in done{guard.by_origin_task.remove(&task);}
        Ok(finished)
    }

    /// Host-originated cancellation by exact conversation/task, not a
    /// model-provided PID. Cancellation does not certify application output.
    pub fn cancel_current_task(
        &self,origin:&BoundCallOrigin
    )->Result<(),E1ExecutionError>{
        let mut gate=self.gate.lock()
            .map_err(|_|E1ExecutionError::LockUnavailable)?;
        let entry=gate.by_origin_task.get(&origin.task_id)
            .ok_or(E1ExecutionError::WrongCancellationOrigin)?;
        if entry.origin!=*origin{
            return Err(E1ExecutionError::WrongCancellationOrigin)
        }
        let entry=entry.clone();
        let stopped=self.workers.cancel_owned(&entry.call_id,&entry.workspace_id);
        let _=self.effects.mark_uncertain(&entry.call_id);
        if stopped.is_err(){
            return Err(E1ExecutionError::WorkerCancellationUncertain)
        }
        gate.by_origin_task.remove(&origin.task_id);
        Ok(())
    }

    pub fn active_count(&self)->Result<usize,E1ExecutionError>{
        Ok(self.gate.lock()
            .map_err(|_|E1ExecutionError::LockUnavailable)?
            .by_origin_task.len())
    }
}
impl Drop for E1ReviewedApplicationExecutor{
    fn drop(&mut self){
        if let Ok(mut gate)=self.gate.lock(){
            for (_,entry) in std::mem::take(&mut gate.by_origin_task){
                let _=self.workers.cancel_owned(&entry.call_id,&entry.workspace_id);
                let _=self.effects.mark_uncertain(&entry.call_id);
            }
        }
    }
}

/// CALL ONLY AFTER the old Lu owner and every child process are independently
/// known to have stopped (exclusive profile/worker ownership). It cannot be
/// called on a live worker: another process's RUNNING state is not a failure.
/// The journal has no authority to replay an ambiguous effect.
pub fn reconcile_after_exclusive_host_restart(
    tasks:&TaskStore,effects:&EffectJournal
)->Result<(usize,usize),E1ExecutionError>{
    let interrupted=tasks.reconcile_interrupted()
        .map_err(|_|E1ExecutionError::WorkerStatusUnavailable)?;
    let uncertain=effects.reconcile_interrupted()
        .map_err(|_|E1ExecutionError::JournalUnavailable)?;
    Ok((interrupted,uncertain))
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn only_fixed_version_probe_with_execute_grant_schema_is_eligible(){
        let d=ToolDescriptor{
            id:E1_VERSION_EXECUTE_TOOL_ID.into(),display_name:"Version".into(),
            effect:ToolEffect::ExecuteCode,
            input_type_url:E1_VERSION_REQUEST_TYPE.into(),
            output_type_url:E1_VERSION_RESULT_TYPE.into(),
        };
        let c=ToolInvocation{
            call_id:"e1-test-1".into(),tool_id:E1_VERSION_EXECUTE_TOOL_ID.into(),
            workspace_id:Some("w-one".into()),
            input:crate::contracts::ProtoPayload{
                type_url:E1_VERSION_REQUEST_TYPE.into(),
                bytes:E1_VERSION_REQUEST_BYTES.into(),
            }
        };
        assert!(reviewed_payload(&c,&d));
        assert!(!reviewed_payload(&c,&ToolDescriptor{
            effect:ToolEffect::ReadWorkspace,..d.clone()
        }));
        assert!(!reviewed_payload(&ToolInvocation{
            input:crate::contracts::ProtoPayload{
                bytes:b"ls -la /home".to_vec(),..c.input.clone()
            },..c.clone()
        },&d));
        assert!(!reviewed_payload(&ToolInvocation{
            call_id:"../../pid".into(),..c.clone()
        },&d));
        assert!(!reviewed_payload(&ToolInvocation{
            workspace_id:Some("../sensitive".into()),..c.clone()
        },&d));
        assert!(!reviewed_payload(&c,&ToolDescriptor{
            output_type_url:"model-selected-protocol".into(),..d
        }));
    }
}
