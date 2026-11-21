//! LU-IPC-01D-R4-R2: Lu CORE-side admitted DLC invocation boundary.
//!
//! This is NOT an IDE client, installer, credential issuer, or a second source
//! of user-task authority. A raw Protobuf OriginRefs/session_ref/catalog is
//! never a grant. Only a native, OS/package-attested transport can enter the
//! production Windows path. The host supplies the immutable original USER
//! turn origin, workspace scope, current policy and exact typed-input decoder.
//! All effects still run through ConversationAuthority + durable dispatch.
//!
//! Linux has the source-only inner gate for later separately authenticated
//! Unix-domain peer admission; there is no generic production entry that can
//! claim Linux package custody from a self-reported PID or UID.
use crate::{
    contracts::{ToolAuthorizer, ToolDescriptor, ToolInvocation, ProtoPayload},
    conversation_authority::{BoundCallOrigin, ConversationAuthority},
    journal::EffectJournal,
    registry::{NativeToolRegistry, ToolResponse},
    workspace_ipc::{proto, MAX_TYPED_BYTES},
    workspace_ipc_transport::{TransportError, TransportSession},
};
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
const MAX_PARALLEL:u32=4;
const MAX_DEADLINE_MS:u64=300_000;
const SOURCE_DOMAIN:&[u8]=b"LU_CORE_DLC_EFFECT_CALL_V1";

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CoreDlcGateError {
    SessionMismatch,
    SessionClosed,
    CapacityExhausted,
    InvalidInvoke,
    InvalidOrigin,
    InvalidDeadline,
    UnresolvedObjectRef,
    UnregisteredTool,
    InvalidInput,
    Denied,
    Unavailable,
    Transport,
}
/// The trusted *host* must decode the exact registered Protobuf request and
/// validate its semantics, argument scope, ownership and resource references.
/// Returning true without decoding is a host integration error, not authority.
/// No default verifier is available; bytes from a DLC are always untrusted.
pub trait TrustedCoreInputVerifier: Send + Sync {
    fn verify_exact(&self, descriptor:&ToolDescriptor, encoded:&[u8])->bool;
}
pub struct HostCoreTask<'a>{
    /// Created by the trusted original-user-turn supervisor, NEVER by copying
    /// incoming OriginRefs. Existing ConversationAuthority rechecks this.
    pub origin:&'a BoundCallOrigin,
    pub workspace_scope:&'a str,
    pub policy:&'a dyn ToolAuthorizer,
    pub input_verifier:&'a dyn TrustedCoreInputVerifier,
}
struct GateState{
    closed:AtomicBool,
    in_flight:AtomicUsize,
    bound_task:Mutex<Option<String>>,
}
/// An authenticated native transport's host-owned lifecycle and bounded
/// dispatch credits. It holds no HMAC key and cannot sign an IDE package.
pub struct CoreDlcHostGate {
    session:TransportSession,
    state:Arc<GateState>,
}
struct CallSlot {
    state:Arc<GateState>,
}
impl Drop for CallSlot {
    fn drop(&mut self){
        self.state.in_flight.fetch_sub(1,Ordering::AcqRel);
    }
}
impl CoreDlcHostGate {
    fn from_trusted_session(session:TransportSession)->Self {
        Self {session,state:Arc::new(GateState{
            closed:AtomicBool::new(false),
            in_flight:AtomicUsize::new(0),
            bound_task:Mutex::new(None),
        })}
    }
    /// Only a genuine installed signed Windows named-pipe session may create
    /// a PRODUCTION ingress. The debug signed-pwsh surrogate has no installer
    /// revocation witness and MUST NOT qualify for effects.
    #[cfg(windows)]
    pub fn from_installed_windows(
        installed:&crate::workspace_ipc_transport::windows::SignedWindowsSession,
    )->Result<Self,CoreDlcGateError>{
        if !installed.installed_enrollment_active() ||
            installed.check_alive().is_err(){
            return Err(CoreDlcGateError::Transport);
        }
        Ok(Self::from_trusted_session(installed.session().clone()))
    }
    #[cfg(test)]
    fn test_only(session:TransportSession)->Self{Self::from_trusted_session(session)}

    pub fn close(&self){
        // An in-flight OS worker is NOT killed by closing this admission
        // gate; its trusted supervisor must separately terminate/reap it.
        self.state.closed.store(true,Ordering::Release);
    }
    pub fn is_closed(&self)->bool{
        self.state.closed.load(Ordering::Acquire)
    }
    pub fn active_calls(&self)->usize{
        self.state.in_flight.load(Ordering::Acquire)
    }
    fn acquire(&self)->Result<CallSlot,CoreDlcGateError>{
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        let claimed=self.state.in_flight.fetch_update(
            Ordering::AcqRel,Ordering::Acquire,
            |n| if n<(MAX_PARALLEL as usize){Some(n+1)}else{None},
        );
        if claimed.is_err(){return Err(CoreDlcGateError::CapacityExhausted);}
        let slot=CallSlot{state:Arc::clone(&self.state)};
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        Ok(slot)
    }
    fn bind_host_task(&self,authority:&ConversationAuthority,task_id:&str)
        ->Result<(),CoreDlcGateError>{
        if task_id.is_empty(){return Err(CoreDlcGateError::Denied);}
        let mut guard=self.state.bound_task.lock()
            .map_err(|_|CoreDlcGateError::Unavailable)?;
        match guard.as_deref(){
            Some(current) if current==task_id=>return Ok(()),
            Some(_)=>return Err(CoreDlcGateError::Denied),
            None=>{}
        }
        // Revoking the original task closes this session's new admissions.
        // The original authority still checks permissions on every dispatch.
        let state=Arc::downgrade(&self.state);
        let pinned=task_id.to_owned();
        authority.on_task_revoked(Arc::new(move |revoked|{
            if revoked==pinned {
                if let Some(gate)=state.upgrade(){
                    gate.closed.store(true,Ordering::Release);
                }
            }
        })).map_err(|_|CoreDlcGateError::Unavailable)?;
        *guard=Some(task_id.to_owned());
        Ok(())
    }
    fn require_session(&self,session:&TransportSession)
        ->Result<(),CoreDlcGateError>{
        if session!=&self.session{return Err(CoreDlcGateError::SessionMismatch);}
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        Ok(())
    }
    fn call_id(&self,host:&HostCoreTask<'_>,op:&str)->String{
        let mut hash=Sha256::new();
        for part in [
            SOURCE_DOMAIN,
            self.session.module_id.as_bytes(),
            host.origin.conversation_id.as_bytes(),
            host.origin.user_turn_id.as_bytes(),
            host.origin.task_id.as_bytes(),
            host.workspace_scope.as_bytes(),
            op.as_bytes(),
        ]{
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part);
        }
        format!("{:x}",hash.finalize())
    }
    fn authenticated_invocation(
        &self,frame:&proto::WorkspaceFrame,
        host:&HostCoreTask<'_>,registry:&NativeToolRegistry,
    )->Result<(ToolInvocation,ToolDescriptor),CoreDlcGateError>{
        self.session.check_request(frame)
            .map_err(|_|CoreDlcGateError::SessionMismatch)?;
        let Some(proto::workspace_frame::Body::Invoke(request))=frame.body.as_ref()
            else{return Err(CoreDlcGateError::InvalidInvoke)};
        let Some(origin)=request.origin.as_ref()
            else{return Err(CoreDlcGateError::InvalidOrigin)};
        if origin.workspace_ref!=host.workspace_scope
            ||origin.conversation_ref!=host.origin.conversation_id
            ||origin.user_turn_ref!=host.origin.user_turn_id
            ||origin.task_ref!=host.origin.task_id
            ||origin.operation_ref.is_empty()
            ||host.workspace_scope.is_empty(){
            return Err(CoreDlcGateError::InvalidOrigin);
        }
        if request.capability_version!=1{
            return Err(CoreDlcGateError::UnregisteredTool);
        }
        // Object locators need a separate host-owned capability resolver;
        // never silently forward a DLC-supplied object_ref as a file path.
        if !request.input_refs.is_empty(){
            return Err(CoreDlcGateError::UnresolvedObjectRef);
        }
        let now=SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_|CoreDlcGateError::Unavailable)?
            .as_millis() as u64;
        if request.deadline_unix_ms<=now
            ||request.deadline_unix_ms>now.saturating_add(MAX_DEADLINE_MS){
            return Err(CoreDlcGateError::InvalidDeadline);
        }
        let encoded=request.input.as_ref().ok_or(CoreDlcGateError::InvalidInput)?;
        if encoded.encoded_message.len()>MAX_TYPED_BYTES{
            return Err(CoreDlcGateError::InvalidInput);
        }
        let tool=registry.tools().map_err(|_|CoreDlcGateError::Unavailable)?
            .into_iter().find(|descriptor|descriptor.id==request.capability_id)
            .ok_or(CoreDlcGateError::UnregisteredTool)?;
        if encoded.type_url!=tool.input_type_url ||
           !host.input_verifier.verify_exact(&tool,&encoded.encoded_message){
            return Err(CoreDlcGateError::InvalidInput);
        }
        Ok((ToolInvocation {
            call_id:self.call_id(host,&origin.operation_ref),
            tool_id:tool.id.clone(),
            workspace_id:Some(host.workspace_scope.to_owned()),
            input:ProtoPayload{
                type_url:encoded.type_url.clone(),
                bytes:encoded.encoded_message.clone(),
            },
        },tool))
    }
    /// Invoked ONLY with frames already read using the OS-authenticated
    /// transport's per-frame message-token observer. Cannot be called from
    /// arbitrary external application code or an unverified socket.
    async fn dispatch_verified_frame(
        &self,frame:&proto::WorkspaceFrame,
        host:&HostCoreTask<'_>,
        authority:&ConversationAuthority,
        registry:&NativeToolRegistry,
        journal:&EffectJournal,
    )->Result<ToolResponse,CoreDlcGateError>{
        let _slot=self.acquire()?;
        self.bind_host_task(authority,&host.origin.task_id)?;
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        let (invocation,descriptor)=self.authenticated_invocation(frame,host,registry)?;
        // Preflight with current task and host policies. The durable native
        // dispatcher independently rechecks these *before* journal prepare.
        authority.authorize_worker_call(
            host.origin,&invocation,&descriptor,host.policy
        ).map_err(|_|CoreDlcGateError::Denied)?;
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        let result=authority.dispatch_bound_durable(
            registry,journal,invocation,host.origin.clone(),host.policy
        ).await;
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        Ok(result)
    }
    /// Host production bridge: only the installed, pinned signed native
    /// Windows lease reads frames. The raw frame cannot be submitted here.
    #[cfg(windows)]
    pub async fn receive_authorized_windows(
        &self,
        installed:&crate::workspace_ipc_transport::windows::SignedWindowsSession,
        pipe:&mut tokio::net::windows::named_pipe::NamedPipeServer,
        host:&HostCoreTask<'_>,
        authority:&ConversationAuthority,
        registry:&NativeToolRegistry,
        journal:&EffectJournal,
    )->Result<ToolResponse,CoreDlcGateError>{
        self.require_session(installed.session())?;
        if !installed.installed_enrollment_active(){
            return Err(CoreDlcGateError::Transport);
        }
        let frame=installed.read_verified_frame(pipe).await
            .map_err(|_|CoreDlcGateError::Transport)?;
        self.dispatch_verified_frame(&frame,host,authority,registry,journal).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contracts::{AuthorizationError, ModuleKind, ModuleManifest,
            NativeTool, ToolEffect, ToolFuture, ToolOutput, CORE_CONTRACT_VERSION},
        conversation_authority::{AuthorizedCapability, TaskAuthority,
            TrustedUserTurnVerifier},
        journal::EffectState,
        workspace_ipc::{WIRE_MAJOR, WIRE_MINOR},
    };
    use std::{fs, path::PathBuf, sync::atomic::AtomicUsize, time::Duration};

    const REQ:&str="type.googleapis.com/lu.v1.TestRequest";
    const OUT:&str="type.googleapis.com/lu.v1.TestReply";
    struct RealUser;
    impl TrustedUserTurnVerifier for RealUser {
        fn verify_user_turn(&self,conv:&str,turn:&str)->bool{
            conv=="human-conversation" && turn=="original-user-turn"
        }
    }
    struct HostPolicy;
    impl ToolAuthorizer for HostPolicy {
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
            ->Result<(),AuthorizationError>{Ok(())}
    }
    struct Deny;
    impl ToolAuthorizer for Deny {
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
            ->Result<(),AuthorizationError>{
            Err(AuthorizationError::new("trusted policy declined"))
        }
    }
    struct TypedVerifier;
    impl TrustedCoreInputVerifier for TypedVerifier{
        fn verify_exact(&self,descriptor:&ToolDescriptor,raw:&[u8])->bool{
            descriptor.id=="lu.core.read"&&descriptor.input_type_url==REQ&&raw==[8,1]
        }
    }
    struct TestRead {
        descriptor:ToolDescriptor,
        count:Arc<AtomicUsize>,
        delay:Duration,
    }
    impl NativeTool for TestRead {
        fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
        fn invoke(&self,_:ToolInvocation)->ToolFuture<'_>{
            Box::pin(async move{
                self.count.fetch_add(1,Ordering::SeqCst);
                if self.delay>Duration::ZERO{
                    tokio::time::sleep(self.delay).await;
                }
                Ok(ToolOutput{payload:ProtoPayload{type_url:OUT.into(),bytes:vec![8,1]}})
            })
        }
    }
    fn setup(delay:Duration)->(NativeToolRegistry,Arc<AtomicUsize>){
        let registry=NativeToolRegistry::new();
        let count=Arc::new(AtomicUsize::new(0));
        let descriptor=ToolDescriptor{
            id:"lu.core.read".into(),display_name:"Core verified read".into(),
            effect:ToolEffect::ReadWorkspace,input_type_url:REQ.into(),
            output_type_url:OUT.into(),
        };
        registry.register(ModuleManifest{
            id:"lu.core".into(),display_name:"Lu Core fixture".into(),
            contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
            tools:vec![descriptor.clone()],panels:vec![],workflows:vec![],
        },vec![Arc::new(TestRead{descriptor,count:Arc::clone(&count),delay})]).unwrap();
        (registry,count)
    }
    fn origin()->BoundCallOrigin{
        BoundCallOrigin{conversation_id:"human-conversation".into(),
            user_turn_id:"original-user-turn".into(),task_id:"task-a".into()}
    }
    fn authority()->ConversationAuthority{
        let authority=ConversationAuthority::new();
        authority.open_user_task(TaskAuthority{
            conversation_id:"human-conversation".into(),
            user_turn_id:"original-user-turn".into(),task_id:"task-a".into(),
            execution_scope_id:"workspace-a".into(),
            capabilities:vec![AuthorizedCapability{
                tool_id:"lu.core.read".into(),effect:ToolEffect::ReadWorkspace,
            }],expires_at:None,
        },&RealUser).unwrap();
        authority
    }
    fn gateway()->CoreDlcHostGate{
        CoreDlcHostGate::test_only(TransportSession{
            session_ref:"native-host-session-a".into(),
            module_id:"thirdparty.workspace".into(),
            core_epoch_ref:"native-core-epoch-a".into(),
        })
    }
    fn frame(operation:&str)->proto::WorkspaceFrame{
        let now=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        proto::WorkspaceFrame{
            protocol_major:WIRE_MAJOR,protocol_minor:WIRE_MINOR,
            request_id:"request-a".into(),
            session_ref:"native-host-session-a".into(),sequence:0,
            body:Some(proto::workspace_frame::Body::Invoke(proto::CapabilityInvoke{
                origin:Some(proto::OriginRefs{
                    workspace_ref:"workspace-a".into(),
                    conversation_ref:"human-conversation".into(),
                    user_turn_ref:"original-user-turn".into(),
                    task_ref:"task-a".into(),
                    operation_ref:operation.into(),
                }),
                capability_id:"lu.core.read".into(),
                capability_version:1,
                input:Some(proto::TypedValue{
                    type_url:REQ.into(),encoded_message:vec![8,1]
                }),
                input_refs:vec![],deadline_unix_ms:now+15_000,
            }))
        }
    }
    fn test_path()->PathBuf{
        use std::time::{SystemTime,UNIX_EPOCH};
        let id=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path=std::env::temp_dir().join(format!(
            "lu-core-dlc-r4r2-{}-{id}",std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }
    // Windows SQLite WAL/SHM files can retain a sharing lease briefly
    // after the final connection closes (e.g. a filesystem scanner).
    // Cleanup is not the journal's acceptance criterion: the test above
    // already required persisted APPLIED replay across a true DB reopen.
    fn cleanup_fixture(root:&PathBuf){
        for _ in 0..6{
            if fs::remove_dir_all(root).is_ok() || !root.exists(){return;}
            std::thread::sleep(Duration::from_millis(25));
        }
        // Unprivileged test infrastructure must not turn a delayed Windows
        // file-handle release into a false journal-implementation failure.
    }
    #[tokio::test]
    async fn host_bound_invoke_uses_current_user_turn_and_durable_idempotency(){
        let root=test_path();
        let db=root.join("effects.sqlite");
        let journal=EffectJournal::open(&db).unwrap();
        let (registry,count)=setup(Duration::ZERO);
        let authority=authority();
        let gate=gateway();
        let trusted=origin();
        let host=HostCoreTask{
            origin:&trusted,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier
        };
        let request=frame("operation-a");
        assert!(gate.dispatch_verified_frame(
            &request,&host,&authority,&registry,&journal).await.unwrap().is_ok());
        assert!(gate.dispatch_verified_frame(
            &request,&host,&authority,&registry,&journal).await.unwrap().is_ok());
        assert_eq!(count.load(Ordering::SeqCst),1,
            "no effect may be re-executed with a second frame or retry");
        let id=gate.call_id(&host,"operation-a");
        assert_eq!(journal.get(&id).unwrap().unwrap().state,EffectState::Applied);
        assert_eq!(gate.active_calls(),0);
        drop(journal);
        let journal=EffectJournal::open(&db).unwrap();
        assert!(gate.dispatch_verified_frame(
            &request,&host,&authority,&registry,&journal).await.unwrap().is_ok());
        assert_eq!(count.load(Ordering::SeqCst),1,
            "receipt must survive a durable journal reopen");
        drop(journal);
        cleanup_fixture(&root);
    }
    #[tokio::test]
    async fn claimed_origin_scope_payload_and_policy_cannot_mint_permission(){
        let root=test_path();
        let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
        let (registry,count)=setup(Duration::ZERO);
        let authority=authority();
        let gate=gateway();
        let trusted=origin();
        let host=HostCoreTask{
            origin:&trusted,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier
        };
        let original=frame("op-denied");
        let mut bad=original.clone();
        if let Some(proto::workspace_frame::Body::Invoke(i))=bad.body.as_mut(){
            i.origin.as_mut().unwrap().user_turn_ref="generated-by-model".into();
        }
        assert_eq!(gate.dispatch_verified_frame(
            &bad,&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::InvalidOrigin));
        let mut bad=original.clone();
        if let Some(proto::workspace_frame::Body::Invoke(i))=bad.body.as_mut(){
            i.origin.as_mut().unwrap().workspace_ref="other-workspace".into();
        }
        assert_eq!(gate.dispatch_verified_frame(
            &bad,&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::InvalidOrigin));
        let mut bad=original.clone();
        if let Some(proto::workspace_frame::Body::Invoke(i))=bad.body.as_mut(){
            i.input.as_mut().unwrap().encoded_message=vec![8,2];
        }
        assert_eq!(gate.dispatch_verified_frame(
            &bad,&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::InvalidInput));
        let mut bad=original.clone();
        if let Some(proto::workspace_frame::Body::Invoke(i))=bad.body.as_mut(){
            i.input_refs.push(proto::ObjectRef{
                owner_module_id:"thirdparty.workspace".into(),
                object_ref:"fake-object".into(),sha256:vec![0x44;32],
                size_bytes:4,expiry_unix_ms:4_000_000_000_000,
            });
        }
        assert_eq!(gate.dispatch_verified_frame(
            &bad,&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::UnresolvedObjectRef));
        let mut bad=original.clone();
        bad.session_ref="another-session".into();
        assert_eq!(gate.dispatch_verified_frame(
            &bad,&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::SessionMismatch));
        let deny=HostCoreTask{
            origin:&trusted,workspace_scope:"workspace-a",
            policy:&Deny,input_verifier:&TypedVerifier,
        };
        assert_eq!(gate.dispatch_verified_frame(
            &original,&deny,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::Denied));
        assert_eq!(count.load(Ordering::SeqCst),0);
        drop(journal);
        cleanup_fixture(&root);
    }
    #[tokio::test]
    async fn revoke_original_turn_closes_admission_and_prevents_new_effect(){
        let root=test_path();
        let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
        let (registry,count)=setup(Duration::ZERO);
        let authority=authority();
        let gate=gateway();
        let trusted=origin();
        let host=HostCoreTask{
            origin:&trusted,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier
        };
        assert!(gate.dispatch_verified_frame(
            &frame("before-revoke"),&host,&authority,&registry,&journal)
            .await.unwrap().is_ok());
        authority.revoke_task("task-a").unwrap();
        assert!(gate.is_closed());
        assert_eq!(gate.dispatch_verified_frame(
            &frame("after-revoke"),&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::SessionClosed));
        assert_eq!(count.load(Ordering::SeqCst),1);
        drop(journal);
        cleanup_fixture(&root);
    }
    #[test]
    fn bounded_credits_and_explicit_shutdown_prevent_unbounded_calls(){
        let gate=gateway();
        let slots=(0..MAX_PARALLEL).map(|_|gate.acquire().unwrap()).collect::<Vec<_>>();
        assert_eq!(gate.active_calls(),MAX_PARALLEL as usize);
        assert!(matches!(gate.acquire(),Err(CoreDlcGateError::CapacityExhausted)));
        gate.close();
        assert!(matches!(gate.acquire(),Err(CoreDlcGateError::SessionClosed)));
        drop(slots);
        assert_eq!(gate.active_calls(),0);
    }
    #[tokio::test]
    async fn aborted_future_is_uncertain_and_recovery_refuses_reexecution(){
        let root=test_path();
        let db=root.join("effects.sqlite");
        let journal=EffectJournal::open(&db).unwrap();
        let (registry,count)=setup(Duration::from_secs(5));
        let authority=authority();
        let gate=gateway();
        let trusted=origin();
        let host=HostCoreTask{
            origin:&trusted,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier
        };
        let request=frame("operation-cancelled");
        let call_id=gate.call_id(&host,"operation-cancelled");
        let timed=tokio::time::timeout(Duration::from_millis(80),
            gate.dispatch_verified_frame(&request,&host,&authority,&registry,&journal)).await;
        assert!(timed.is_err(),"the fixture must be interrupted before effects finish");
        assert_eq!(count.load(Ordering::SeqCst),1);
        assert_eq!(gate.active_calls(),0);
        assert_eq!(journal.get(&call_id).unwrap().unwrap().state,EffectState::Uncertain,
            "future cancellation must never leave effect EXECUTING or retryable");
        drop(journal);
        let reopened=EffectJournal::open(&db).unwrap();
        assert_eq!(reopened.reconcile_interrupted().unwrap(),0);
        let retry=gate.dispatch_verified_frame(
            &request,&host,&authority,&registry,&reopened).await.unwrap();
        assert!(retry.is_err(),"an uncertain effect must not be replayed");
        assert_eq!(count.load(Ordering::SeqCst),1);
        drop(reopened);
        cleanup_fixture(&root);
    }
    #[test]
    fn journal_id_is_stable_across_connection_epoch_but_bound_to_original_task(){
        let gate=gateway();
        let origin=origin();
        let host=HostCoreTask{origin:&origin,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier};
        let same=CoreDlcHostGate::test_only(TransportSession{
            session_ref:"different-after-restart".into(),
            module_id:"thirdparty.workspace".into(),core_epoch_ref:"new-core-epoch".into(),
        });
        assert_eq!(gate.call_id(&host,"op-one"),same.call_id(&host,"op-one"));
        assert_ne!(gate.call_id(&host,"op-one"),gate.call_id(&host,"op-two"));
        let mut other=origin.clone();
        other.task_id="another-task".into();
        let another=HostCoreTask{origin:&other,workspace_scope:host.workspace_scope,
            policy:host.policy,input_verifier:host.input_verifier};
        assert_ne!(gate.call_id(&host,"op-one"),gate.call_id(&another,"op-one"));
    }
}
