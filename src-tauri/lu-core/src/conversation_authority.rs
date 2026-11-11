//! Conversation-bound authority for Lu agent actions.
//!
//! A model can REQUEST a tool call but cannot CREATE a capability. The trusted
//! host opens a task only after verifying a real user-authored conversation
//! turn. Untrusted documents, retrieved pages, model output, subagents, and
//! tool responses are task data, never authorities.
//!
//! Deliberately not wired into Jan's desktop or Cowork yet: the desktop must
//! implement a real conversation ownership/role verifier, attach immutable
//! origin context on the Rust side, and constrain argument-level resource
//! scope before this can be a production authorization boundary.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock},
    time::SystemTime,
};

use crate::{
    contracts::{AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation},
    durable::DurableDispatcher,
    journal::EffectJournal,
    privilege_guard::{enforce_model_ceiling, ProtectedEffect},
    registry::{NativeToolRegistry, ToolResponse},
};

/// MUST be implemented with host-owned conversation records. In particular,
/// check the authenticated account owns this conversation, the referenced
/// message exists, and its persisted role is USER (not system, model, tool,
/// quoted text or a retrieved document). Never trust an LLM-reported role.
pub trait TrustedUserTurnVerifier: Send + Sync {
    fn verify_user_turn(&self, conversation_id: &str, user_turn_id: &str) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedCapability {
    pub tool_id: String,
    pub effect: ToolEffect,
}

/// Immutable, exact origin of a task and its delegated capabilities.
/// The host, not the model or webview, chooses these identifiers.
#[derive(Debug, Clone)]
pub struct TaskAuthority {
    pub conversation_id: String,
    pub user_turn_id: String,
    pub task_id: String,
    /// Exact workspace/session scope bound by the host. Do not accept this
    /// value directly from a model or arbitrary webview invocation.
    pub execution_scope_id: String,
    pub capabilities: Vec<AuthorizedCapability>,
    pub expires_at: Option<SystemTime>,
}

/// Out-of-band provenance attached by Lu's trusted task scheduler. It is NOT
/// a token or permission claim that should be serialized into model prompts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCallOrigin {
    pub conversation_id: String,
    pub user_turn_id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityError {
    InvalidIdentifier,
    InvalidGrant,
    UnverifiedUserTurn,
    DuplicateTask,
    UnknownTask,
    ProtectedEffect(ProtectedEffect),
    Unavailable,
}

#[derive(Clone)]
struct Record {
    authority: TaskAuthority,
    active: bool,
    allowed: BTreeMap<String, ToolEffect>,
}

type TaskRevocationListener = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Clone, Default)]
pub struct ConversationAuthority {
    tasks: Arc<RwLock<BTreeMap<String, Record>>>,
    revoked_listeners: Arc<RwLock<Vec<TaskRevocationListener>>>,
}

fn valid_identity(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 128 && !value.contains('\0')
}

impl ConversationAuthority {
    pub fn new() -> Self {
        Self::default()
    }

    /// A trusted host supervisor registers before it can launch any workers.
    /// Notifications run synchronously after the authority is closed; no
    /// worker can retain a grant by racing an async notification queue.
    pub fn on_task_revoked(
        &self,
        listener: Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Result<(), AuthorityError> {
        self.revoked_listeners
            .write()
            .map_err(|_| AuthorityError::Unavailable)?
            .push(listener);
        Ok(())
    }

    fn notify_revoked(&self, task_ids: &[String]) -> Result<(), AuthorityError> {
        let listeners = self
            .revoked_listeners
            .read()
            .map_err(|_| AuthorityError::Unavailable)?
            .clone();
        for task_id in task_ids {
            for listener in &listeners {
                listener(task_id);
            }
        }
        Ok(())
    }

    /// Called ONLY from a trusted host handler after checking the actual
    /// persisted USER message and ownership. No "model says user approved"
    /// string can satisfy the verifier.
    pub fn open_user_task(
        &self,
        authority: TaskAuthority,
        verifier: &dyn TrustedUserTurnVerifier,
    ) -> Result<(), AuthorityError> {
        for id in [
            &authority.conversation_id,
            &authority.user_turn_id,
            &authority.task_id,
            &authority.execution_scope_id,
        ] {
            if !valid_identity(id) {
                return Err(AuthorityError::InvalidIdentifier);
            }
        }
        if !verifier.verify_user_turn(&authority.conversation_id, &authority.user_turn_id) {
            return Err(AuthorityError::UnverifiedUserTurn);
        }
        if authority.capabilities.is_empty() {
            return Err(AuthorityError::InvalidGrant);
        }
        if authority
            .expires_at
            .is_some_and(|expires| SystemTime::now() >= expires)
        {
            return Err(AuthorityError::InvalidGrant);
        }
        let mut allowed = BTreeMap::new();
        for capability in &authority.capabilities {
            if !valid_identity(&capability.tool_id) {
                return Err(AuthorityError::InvalidGrant);
            }
            enforce_model_ceiling(capability.effect).map_err(AuthorityError::ProtectedEffect)?;
            if allowed
                .insert(capability.tool_id.clone(), capability.effect)
                .is_some()
            {
                return Err(AuthorityError::InvalidGrant);
            }
        }
        let mut tasks = self
            .tasks
            .write()
            .map_err(|_| AuthorityError::Unavailable)?;
        if tasks.contains_key(&authority.task_id) {
            return Err(AuthorityError::DuplicateTask);
        }
        tasks.insert(
            authority.task_id.clone(),
            Record {
                authority,
                active: true,
                allowed,
            },
        );
        Ok(())
    }

    /// An existing task cannot gain new capabilities via a model message.
    /// Changed user intent should be reverified and expressed as a NEW task.
    pub fn revoke_task(&self, task_id: &str) -> Result<(), AuthorityError> {
        let mut tasks = self
            .tasks
            .write()
            .map_err(|_| AuthorityError::Unavailable)?;
        let entry = tasks.get_mut(task_id).ok_or(AuthorityError::UnknownTask)?;
        entry.active = false;
        drop(tasks);
        self.notify_revoked(&[task_id.to_owned()])
    }

    pub fn revoke_conversation(&self, conversation_id: &str) -> Result<usize, AuthorityError> {
        let mut tasks = self
            .tasks
            .write()
            .map_err(|_| AuthorityError::Unavailable)?;
        let mut ids = Vec::new();
        for (id, task) in tasks.iter_mut() {
            if task.authority.conversation_id == conversation_id && task.active {
                task.active = false;
                ids.push(id.clone());
            }
        }
        drop(tasks);
        self.notify_revoked(&ids)?;
        Ok(ids.len())
    }

    /// This is the safe dispatch entry point for future model-originated
    /// calls. It checks the immutable origin, exact tool/effect/scope, and
    /// delegates to existing host policy. It does NOT infer whether a request
    /// is semantically relevant to the user's task: the host must additionally
    /// enforce typed argument scope and high-risk action review.
    pub async fn dispatch_bound(
        &self,
        registry: &NativeToolRegistry,
        invocation: ToolInvocation,
        origin: BoundCallOrigin,
        host_policy: &dyn ToolAuthorizer,
    ) -> ToolResponse {
        let guard = BoundAuthorizer {
            authority: self,
            origin: &origin,
            host_policy,
        };
        registry.dispatch(invocation, &guard).await
    }

    /// Durable model-originated dispatch with the same conversation authority
    /// fence as dispatch_bound. Unlike the legacy in-memory path, repeated
    /// effect call IDs are served from validated receipts or denied on uncertain
    /// state. The host must own journal startup recovery and single-writer
    /// ownership; this does not terminate OS workers or cancel in-flight tools.
    pub async fn dispatch_bound_durable(
        &self,
        registry: &NativeToolRegistry,
        journal: &EffectJournal,
        invocation: ToolInvocation,
        origin: BoundCallOrigin,
        host_policy: &dyn ToolAuthorizer,
    ) -> ToolResponse {
        let guard = BoundAuthorizer {
            authority: self,
            origin: &origin,
            host_policy,
        };
        DurableDispatcher { registry, journal }
            .dispatch(invocation, &guard)
            .await
    }

    /// Host-only admission check for a separately supervised native CE worker.
    /// No worker process is started here. The caller must bind the origin from
    /// trusted task state, not from a model/webview field, and must enforce the
    /// OS sandbox and argument-level resource policy before launching.
    /// Re-check this immediately before each worker launch, never rely on a
    /// previously advertised catalog or a cached approval.
    pub fn authorize_worker_call(
        &self,
        origin: &BoundCallOrigin,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
        host_policy: &dyn ToolAuthorizer,
    ) -> Result<(), AuthorizationError> {
        if invocation.call_id.trim().is_empty()
            || invocation.input.type_url != descriptor.input_type_url
        {
            return Err(AuthorizationError::new("invalid worker call or input type"));
        }
        enforce_model_ceiling(descriptor.effect)
            .map_err(|_| AuthorizationError::new("worker effect exceeds model ceiling"))?;
        self.authorize_internal(origin, invocation, descriptor)?;
        host_policy.authorize(invocation, descriptor)
    }

    /// Fence a short, bounded host-side CE effect against task or conversation
    /// revocation. The authoritative task read lock is held until the effect
    /// finishes; revocation must acquire the write lock before it can return.
    ///
    /// The host policy is rechecked before taking the lock, then immutable
    /// task origin, scope, effect and expiry are rechecked under the lock.
    /// This is ONLY for bounded local broker effects, never worker execution,
    /// network I/O, user interaction, or callbacks that revoke task authority.
    /// The caller must separately fence durable publisher/grant revocation.
    pub fn with_authorized_worker_effect<R>(
        &self,
        origin: &BoundCallOrigin,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
        host_policy: &dyn ToolAuthorizer,
        operation: impl FnOnce() -> R,
    ) -> Result<R, AuthorizationError> {
        self.authorize_worker_call(origin, invocation, descriptor, host_policy)?;
        let tasks = self
            .tasks
            .read()
            .map_err(|_| AuthorizationError::new("conversation authority unavailable"))?;
        Self::check_internal(&tasks, origin, invocation, descriptor)?;
        Ok(operation())
    }

    /// P0-AUTH: atomic host-owned *launch admission* for a separately
    /// approved READ + EXECUTE worker. The two capabilities must come from
    /// the SAME current ANSF user task, scope and policy. The read guard
    /// remains held only while the host starts and registers the OS child,
    /// so revoke_task/revoke_conversation cannot notify observers before
    /// the newly launched child becomes visible to their kill callback.
    ///
    /// The closure MUST be a synchronous, bounded OS launch/registration:
    /// no model inference, network requests, modal UI, waiting for child
    /// exit, callbacks that revoke authority, or arbitrary user code here.
    /// Never interpret a returned closure value as a new permission.
    pub fn with_authorized_worker_launch<R>(
        &self,
        origin: &BoundCallOrigin,
        read: &ToolInvocation,
        read_descriptor: &ToolDescriptor,
        execute: &ToolInvocation,
        execute_descriptor: &ToolDescriptor,
        policy: &dyn ToolAuthorizer,
        launch_and_register: impl FnOnce() -> R,
    ) -> Result<R, AuthorizationError> {
        if read.call_id == execute.call_id
            || read_descriptor.effect != ToolEffect::ReadWorkspace
            || execute_descriptor.effect != ToolEffect::ExecuteCode
            || read.workspace_id != execute.workspace_id
            || read.input.type_url != read_descriptor.input_type_url
            || execute.input.type_url != execute_descriptor.input_type_url
        {
            return Err(AuthorizationError::new(
                "reviewed launch needs distinct exact read and execute authority",
            ));
        }
        // Host policy can call external providers: do this BEFORE locking
        // native authority, then inspect the final immutable task record
        // again once the short launch fence is held.
        self.authorize_worker_call(origin, read, read_descriptor, policy)?;
        self.authorize_worker_call(origin, execute, execute_descriptor, policy)?;
        let tasks = self.tasks.read()
            .map_err(|_| AuthorizationError::new("reviewed launch authority unavailable"))?;
        Self::check_internal(&tasks, origin, read, read_descriptor)?;
        Self::check_internal(&tasks, origin, execute, execute_descriptor)?;
        Ok(launch_and_register())
    }

    /// For a model-facing tool catalog, filter candidate tools by the
    /// currently authenticated task. This MUST NOT be used to approve an
    /// action: dispatch_bound independently re-checks these conditions.
    pub fn permitted_tool_ids(
        &self,
        origin: &BoundCallOrigin,
    ) -> Result<BTreeSet<String>, AuthorityError> {
        let tasks = self.tasks.read().map_err(|_| AuthorityError::Unavailable)?;
        let record = tasks
            .get(&origin.task_id)
            .ok_or(AuthorityError::UnknownTask)?;
        if !record.active
            || record.authority.conversation_id != origin.conversation_id
            || record.authority.user_turn_id != origin.user_turn_id
            || record
                .authority
                .expires_at
                .is_some_and(|expires| SystemTime::now() >= expires)
        {
            return Ok(BTreeSet::new());
        }
        Ok(record.allowed.keys().cloned().collect())
    }

    fn authorize_internal(
        &self,
        origin: &BoundCallOrigin,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
    ) -> Result<(), AuthorizationError> {
        let tasks = self
            .tasks
            .read()
            .map_err(|_| AuthorizationError::new("conversation authority unavailable"))?;
        Self::check_internal(&tasks, origin, invocation, descriptor)
    }

    fn check_internal(
        tasks: &BTreeMap<String, Record>,
        origin: &BoundCallOrigin,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
    ) -> Result<(), AuthorizationError> {
        let task = tasks
            .get(&origin.task_id)
            .ok_or_else(|| AuthorizationError::new("no authorized task for tool call"))?;
        if !task.active
            || task.authority.conversation_id != origin.conversation_id
            || task.authority.user_turn_id != origin.user_turn_id
            || task
                .authority
                .expires_at
                .is_some_and(|expires| SystemTime::now() >= expires)
        {
            return Err(AuthorizationError::new("task authority invalid or revoked"));
        }
        if invocation.workspace_id.as_deref() != Some(task.authority.execution_scope_id.as_str()) {
            return Err(AuthorizationError::new(
                "tool call scope differs from authorized task",
            ));
        }
        if invocation.tool_id != descriptor.id
            || task.allowed.get(&descriptor.id) != Some(&descriptor.effect)
        {
            return Err(AuthorizationError::new(
                "tool action exceeds conversation-delegated capabilities",
            ));
        }
        Ok(())
    }
}

struct BoundAuthorizer<'a> {
    authority: &'a ConversationAuthority,
    origin: &'a BoundCallOrigin,
    host_policy: &'a dyn ToolAuthorizer,
}

impl ToolAuthorizer for BoundAuthorizer<'_> {
    fn authorize(
        &self,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
    ) -> Result<(), AuthorizationError> {
        self.authority
            .authorize_internal(self.origin, invocation, descriptor)?;
        self.host_policy.authorize(invocation, descriptor)
    }
}

#[cfg(test)]
mod p0_auth_launch_tests {
    use super::*;
    use crate::contracts::{ProtoPayload,AuthorizationError};
    use std::time::Duration;
    struct OriginalHuman;
    impl TrustedUserTurnVerifier for OriginalHuman{
        fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
            chat=="c-test"&&turn=="g-test"
        }
    }
    struct AllowReview;
    impl ToolAuthorizer for AllowReview{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
            ->Result<(),AuthorizationError>{Ok(())}
    }
    fn original()->(ConversationAuthority,BoundCallOrigin,
        ToolInvocation,ToolDescriptor,ToolInvocation,ToolDescriptor){
        let auth=ConversationAuthority::new();
        let origin=BoundCallOrigin{
            conversation_id:"c-test".into(),user_turn_id:"g-test".into(),
            task_id:"task-one".into()
        };
        let reader=ToolDescriptor{
            id:"lu.reviewed.file".into(),display_name:"Read".into(),
            effect:ToolEffect::ReadWorkspace,
            input_type_url:"lu.auth.read".into(),
            output_type_url:"lu.auth.read.reply".into()
        };
        let execute=ToolDescriptor{
            id:"lu.reviewed.execute".into(),display_name:"Execute".into(),
            effect:ToolEffect::ExecuteCode,
            input_type_url:"lu.auth.exec".into(),
            output_type_url:"lu.auth.exec.reply".into()
        };
        auth.open_user_task(TaskAuthority{
            conversation_id:origin.conversation_id.clone(),
            user_turn_id:origin.user_turn_id.clone(),
            task_id:origin.task_id.clone(),
            execution_scope_id:"host-only-scope".into(),
            capabilities:vec![
                AuthorizedCapability{tool_id:reader.id.clone(),
                    effect:ToolEffect::ReadWorkspace},
                AuthorizedCapability{tool_id:execute.id.clone(),
                    effect:ToolEffect::ExecuteCode}
            ],expires_at:None,
        },&OriginalHuman).unwrap();
        let read=ToolInvocation{
            call_id:"read-one".into(),tool_id:reader.id.clone(),
            workspace_id:Some("host-only-scope".into()),
            input:ProtoPayload{type_url:reader.input_type_url.clone(),
                bytes:vec![1]}
        };
        let run=ToolInvocation{
            call_id:"exec-one".into(),tool_id:execute.id.clone(),
            workspace_id:Some("host-only-scope".into()),
            input:ProtoPayload{type_url:execute.input_type_url.clone(),
                bytes:vec![2]}
        };
        (auth,origin,read,reader,run,execute)
    }
    #[test]
    fn revoked_or_mismatched_authority_cannot_enter_host_os_launch(){
        let (auth,origin,read,read_d,run,exec_d)=original();
        let launched=std::sync::atomic::AtomicBool::new(false);
        let different=ToolInvocation{
            workspace_id:Some("other-chat".into()),..run.clone()
        };
        assert!(auth.with_authorized_worker_launch(
            &origin,&read,&read_d,&different,&exec_d,&AllowReview,
            ||launched.store(true,std::sync::atomic::Ordering::SeqCst)
        ).is_err());
        assert!(!launched.load(std::sync::atomic::Ordering::SeqCst));
        auth.revoke_task(&origin.task_id).unwrap();
        assert!(auth.with_authorized_worker_launch(
            &origin,&read,&read_d,&run,&exec_d,&AllowReview,
            ||launched.store(true,std::sync::atomic::Ordering::SeqCst)
        ).is_err());
        assert!(!launched.load(std::sync::atomic::Ordering::SeqCst));
    }
    #[test]
    fn concurrent_revocation_cannot_finish_before_registered_child_is_visible(){
        let (auth,origin,read,read_d,run,exec_d)=original();
        let (entered_tx,entered_rx)=std::sync::mpsc::channel();
        let (release_tx,release_rx)=std::sync::mpsc::channel();
        let (revoker_tx,revoker_rx)=std::sync::mpsc::channel();
        let (done_tx,done_rx)=std::sync::mpsc::channel();
        std::thread::scope(|scope|{
            let authorized=auth.clone();
            scope.spawn(move||{
                authorized.with_authorized_worker_launch(
                    &origin,&read,&read_d,&run,&exec_d,&AllowReview,||{
                        entered_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                    }
                ).unwrap();
            });
            entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            let revoke=auth.clone();
            scope.spawn(move||{
                revoker_tx.send(()).unwrap();
                revoke.revoke_task("task-one").unwrap();
                done_tx.send(()).unwrap();
            });
            revoker_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert!(done_rx.recv_timeout(Duration::from_millis(30)).is_err(),
                "task revocation cannot finish while short launch is fenced");
            release_tx.send(()).unwrap();
            done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        });
    }
}
