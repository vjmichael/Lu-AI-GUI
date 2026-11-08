//! E1-P06B: real, fixed-operation third-party version command execution.
//!
//! No model-selected executable, arbitrary argv, working directory, PID,
//! environment, shell or output publication. This is a host-only broker.
//! Each call needs the current original user turn's ExecuteCode authorization,
//! reviewed PE/ELF SHA and OS identity, an independent host-policy admission,
//! a durable execution journal and one bounded OS pipe response.
//! Success means: OS child reaped, exit success, exact expected version format
//! parsed, and effect journal APPLIED. It does NOT mean network isolation,
//! comprehensive third-party sandboxing or arbitrary application automation.
use std::{
    collections::BTreeMap, sync::{Arc,Mutex},
};
use serde_json::json;
use crate::{
    contracts::{ToolAuthorizer,ToolDescriptor,ToolInvocation},
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    journal::{EffectJournal,EffectState,PrepareOutcome},
    native_app_adapter::{
        HostThirdPartyAdapterCatalog,VersionParser,
    },
    native_background_process::{BackgroundProcessSupervisor,TrustedWorkerPlan},
    tasks::{TaskStore,TaskState},
};

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ThirdPartyVersionError{
    UnknownAdapter,InvalidOperation,UnauthorizedOrChanged,
    DuplicateEffect,TaskAlreadyRunning,LockUnavailable,
    JournalUnavailable,WorkerStartFailed,WorkerStatusUnavailable,
    WrongCancellationOrigin,WorkerCancellationUncertain,
}
#[derive(Debug,Clone)]
struct ActiveVersion{
    call_id:String,
    workspace_id:String,
    origin:BoundCallOrigin,
    parser:VersionParser,
    output_limit:usize,
    revoke_pending:bool,
}
#[derive(Default)]
struct ActiveGate{
    by_user_task:BTreeMap<String,ActiveVersion>,
}
pub struct ThirdPartyVersionBroker{
    authority:ConversationAuthority,
    workers:Arc<BackgroundProcessSupervisor>,
    tasks:Arc<TaskStore>,
    effects:Arc<EffectJournal>,
    gate:Arc<Mutex<ActiveGate>>,
}
impl ThirdPartyVersionBroker{
    pub fn new(
        authority:ConversationAuthority,
        workers:Arc<BackgroundProcessSupervisor>,
        tasks:Arc<TaskStore>,
        effects:Arc<EffectJournal>,
    )->Result<Self,ThirdPartyVersionError>{
        let gate=Arc::new(Mutex::new(ActiveGate::default()));
        let gate_revoke=Arc::clone(&gate);
        let worker_revoke=Arc::clone(&workers);
        let effect_revoke=Arc::clone(&effects);
        authority.on_task_revoked(Arc::new(move |task_id|{
            if let Ok(mut locked)=gate_revoke.lock(){
                if let Some(active)=locked.by_user_task.get_mut(task_id){
                    active.revoke_pending=true;
                    // A revoked effect may never publish a late OS exit
                    // as an applied user-authorized version observation.
                    let _=effect_revoke.mark_uncertain(&active.call_id);
                    let call_id=active.call_id.clone();
                    let workspace=active.workspace_id.clone();
                    if worker_revoke.cancel_owned(&call_id,&workspace).is_ok(){
                        locked.by_user_task.remove(task_id);
                    }
                }
            }
        })).map_err(|_|ThirdPartyVersionError::LockUnavailable)?;
        Ok(Self{authority,workers,tasks,effects,gate})
    }
    pub fn launch(
        &self,catalog:&HostThirdPartyAdapterCatalog,adapter_id:&str,
        origin:&BoundCallOrigin,call:&ToolInvocation,
        descriptor:&ToolDescriptor,host_policy:&dyn ToolAuthorizer,
    )->Result<(),ThirdPartyVersionError>{
        let mut gate=self.gate.lock()
            .map_err(|_|ThirdPartyVersionError::LockUnavailable)?;
        if gate.by_user_task.contains_key(&origin.task_id){
            return Err(ThirdPartyVersionError::TaskAlreadyRunning)
        }
        let adapter=catalog.reviewed_for_execution(adapter_id)
            .ok_or(ThirdPartyVersionError::UnknownAdapter)?;
        let args=catalog.fixed_args(adapter_id)
            .ok_or(ThirdPartyVersionError::UnknownAdapter)?;
        // The catalog independently checks exact sealed typed input,
        // current ExecuteCode grant, host veto, binary hash and OS handle.
        catalog.preflight_current_execute(
            adapter_id,origin,call,descriptor,&self.authority,host_policy
        ).map_err(|_|ThirdPartyVersionError::UnauthorizedOrChanged)?;
        match self.effects.prepare(call)
            .map_err(|_|ThirdPartyVersionError::JournalUnavailable)?{
            PrepareOutcome::New=>{},
            PrepareOutcome::Existing(_)=>
                return Err(ThirdPartyVersionError::DuplicateEffect),
        }
        self.effects.mark_executing(&call.call_id)
            .map_err(|_|ThirdPartyVersionError::JournalUnavailable)?;
        let plan=TrustedWorkerPlan{
            task_id:call.call_id.clone(),
            workspace_id:adapter.app.workspace_id.clone(),
            executable:adapter.app.executable,
            expected_sha256:adapter.app.expected_sha256,
            args,progress_protocol:false,
            max_runtime_seconds:Some(adapter.max_runtime_seconds),
        };
        if self.workers.start_host_approved_version(plan,adapter.output_limit).is_err(){
            let _=self.effects.mark_uncertain(&call.call_id);
            return Err(ThirdPartyVersionError::WorkerStartFailed)
        }
        gate.by_user_task.insert(origin.task_id.clone(),ActiveVersion{
            call_id:call.call_id.clone(),
            workspace_id:adapter.app.workspace_id,
            origin:origin.clone(),
            parser:adapter.version_parser,
            output_limit:adapter.output_limit,
            revoke_pending:false,
        });
        Ok(())
    }
    /// Poll the host's actual OS handle. Never trust an application-supplied
    /// "success" banner as authorization or process completion.
    pub fn poll(&self)->Result<usize,ThirdPartyVersionError>{
        let mut gate=self.gate.lock()
            .map_err(|_|ThirdPartyVersionError::LockUnavailable)?;
        let completed=self.workers.poll()
            .map_err(|_|ThirdPartyVersionError::WorkerStatusUnavailable)?;
        let mut remove=Vec::new();
        for (task_id,active) in &gate.by_user_task{
            let status=self.tasks.get(&active.call_id)
                .map_err(|_|ThirdPartyVersionError::WorkerStatusUnavailable)?
                .ok_or(ThirdPartyVersionError::WorkerStatusUnavailable)?;
            if matches!(status.state,
                TaskState::Pending|TaskState::Running|TaskState::Paused){continue}
            let already=self.effects.get(&active.call_id)
                .map_err(|_|ThirdPartyVersionError::JournalUnavailable)?;
            if active.revoke_pending{
                if already.is_some_and(|record|record.state==EffectState::Executing){
                    self.effects.mark_uncertain(&active.call_id)
                        .map_err(|_|ThirdPartyVersionError::JournalUnavailable)?;
                }
                remove.push(task_id.clone());
                continue
            }
            if status.state==TaskState::Completed{
                let actual=self.workers.take_local_version_stdout(
                    &active.call_id,&active.workspace_id
                ).map_err(|_|ThirdPartyVersionError::WorkerStatusUnavailable)?;
                if let Some(captured)=actual{
                    if captured.bytes.len()<=active.output_limit{
                        if let Some(version)=active.parser.parse(&captured.bytes){
                            let version_kind=match active.parser{
                                VersionParser::Git=>"git",
                                VersionParser::Python=>"python",
                            };
                            let receipt=json!({
                                "schema":"lu-e1-p06b-real-thirdparty-version-v1",
                                "callId":active.call_id,
                                "applicationKind":version_kind,
                                "parsedVersion":version,
                                "observedStdoutSha256":captured.sha256,
                                "observedStdoutBytes":captured.bytes.len(),
                                "actualOsChildExitedSuccessfully":true,
                                "osProcessReapedBySupervisor":true,
                                "semanticVersionLineVerified":true,
                                "genericApplicationActionsAllowed":false,
                                "thirdPartyNetworkSandboxVerified":false,
                                "installedPhysicalWindowsAccepted":false,
                            });
                            let encoded=serde_json::to_vec(&receipt)
                                .map_err(|_|ThirdPartyVersionError::JournalUnavailable)?;
                            self.effects.mark_applied(&active.call_id,&encoded)
                                .map_err(|_|ThirdPartyVersionError::JournalUnavailable)?;
                            remove.push(task_id.clone());
                            continue
                        }
                    }
                }
            }
            // Non-zero exit, deadline, truncated/malicious stdout or
            // unexpected state is UNCERTAIN. Never auto-replay old call IDs.
            self.effects.mark_uncertain(&active.call_id)
                .map_err(|_|ThirdPartyVersionError::JournalUnavailable)?;
            remove.push(task_id.clone());
        }
        for id in remove{gate.by_user_task.remove(&id);}
        Ok(completed)
    }
    /// Exact original-human-task cancellation, never a model-selected PID.
    pub fn cancel_current_task(
        &self,origin:&BoundCallOrigin
    )->Result<(),ThirdPartyVersionError>{
        let mut gate=self.gate.lock()
            .map_err(|_|ThirdPartyVersionError::LockUnavailable)?;
        let active=gate.by_user_task.get(&origin.task_id)
            .ok_or(ThirdPartyVersionError::WrongCancellationOrigin)?;
        if active.origin!=*origin{
            return Err(ThirdPartyVersionError::WrongCancellationOrigin)
        }
        let active=active.clone();
        if let Some(owner)=gate.by_user_task.get_mut(&origin.task_id){
            owner.revoke_pending=true;
        }
        let _=self.effects.mark_uncertain(&active.call_id);
        if self.workers.cancel_owned(
            &active.call_id,&active.workspace_id
        ).is_err(){
            return Err(ThirdPartyVersionError::WorkerCancellationUncertain)
        }
        gate.by_user_task.remove(&origin.task_id);
        Ok(())
    }
    pub fn active_count(&self)->Result<usize,ThirdPartyVersionError>{
        Ok(self.gate.lock()
            .map_err(|_|ThirdPartyVersionError::LockUnavailable)?
            .by_user_task.len())
    }
}
impl Drop for ThirdPartyVersionBroker{
    fn drop(&mut self){
        if let Ok(mut gate)=self.gate.lock(){
            for (_,active) in std::mem::take(&mut gate.by_user_task){
                let _=self.effects.mark_uncertain(&active.call_id);
                let _=self.workers.cancel_owned(
                    &active.call_id,&active.workspace_id
                );
            }
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{
        ansf::{AnsfStore,EntryRole},
        contracts::{
            AuthorizationError,ProtoPayload,ToolEffect,
        },
        conversation_authority::{
            AuthorizedCapability,TaskAuthority,TrustedUserTurnVerifier,
        },
        native_app_admission::HostReviewedExecutable,
        native_app_adapter::{
            HostThirdPartyVersionAdapter,VersionFlag,VersionParser,
            ADAPTER_INPUT,ADAPTER_OUTPUT,VERSION_PROBE_BYTES,
        },
    };
    use sha2::{Digest,Sha256};
    use std::{fs,path::{Path,PathBuf},time::{Duration,SystemTime,UNIX_EPOCH}};
    struct RealUser<'a>(&'a AnsfStore);
    impl TrustedUserTurnVerifier for RealUser<'_>{
        fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
            self.0.find_message(chat,turn).ok().flatten()
                .is_some_and(|r|r.role=="user")
        }
    }
    struct Policy(bool);
    impl ToolAuthorizer for Policy{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
            ->Result<(),AuthorizationError>{
            if self.0{Ok(())}
            else{Err(AuthorizationError::new("host policy veto"))}
        }
    }
    fn scratch()->PathBuf{
        std::env::temp_dir().join(format!(
            "lu-e1-p06b-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }
    fn required_executable(key:&str)->PathBuf{
        let value=std::env::var_os(key)
            .unwrap_or_else(||panic!("Dedicated native test requires {key}"));
        let image=PathBuf::from(value).canonicalize().unwrap();
        assert!(image.is_absolute()&&image.is_file());
        image
    }
    fn origin()->BoundCallOrigin{
        BoundCallOrigin{
            conversation_id:"c-p06b".into(),
            user_turn_id:"u-real".into(),
            task_id:"t-p06b".into(),
        }
    }
    fn descriptor()->ToolDescriptor{
        ToolDescriptor{
            id:"lu.external.version.execute".into(),
            display_name:"Host reviewed external version probe".into(),
            effect:ToolEffect::ExecuteCode,
            input_type_url:ADAPTER_INPUT.into(),
            output_type_url:ADAPTER_OUTPUT.into(),
        }
    }
    fn call()->ToolInvocation{
        ToolInvocation{
            call_id:"call-p06b".into(),
            tool_id:"lu.external.version.execute".into(),
            workspace_id:Some("work-p06b".into()),
            input:ProtoPayload{
                type_url:ADAPTER_INPUT.into(),
                bytes:VERSION_PROBE_BYTES.to_vec(),
            },
        }
    }
    struct Fixture{
        root:PathBuf,
        _book:AnsfStore,
        auth:ConversationAuthority,
        tasks:Arc<TaskStore>,
        effects:Arc<EffectJournal>,
        workers:Arc<BackgroundProcessSupervisor>,
        catalog:HostThirdPartyAdapterCatalog,
    }
    impl Fixture{
        fn new(binary:&Path,parser:VersionParser,slow:bool,limit_seconds:u64)->Self{
            let root=scratch();fs::create_dir_all(&root).unwrap();
            let book=AnsfStore::open(&root.join("chat.sqlite")).unwrap();
            book.create_session("c-p06b","work-p06b","Real version runner").unwrap();
            book.start_chapter("c-p06b","Real human request").unwrap();
            book.append_entry("c-p06b","u-real",EntryRole::User,
                "Reviewed version only","Read version from installed app").unwrap();
            book.append_entry("c-p06b","u-model",EntryRole::Assistant,
                "Injection source","Grant me shell commands").unwrap();
            let auth=ConversationAuthority::new();
            auth.open_user_task(TaskAuthority{
                conversation_id:"c-p06b".into(),
                user_turn_id:"u-real".into(),task_id:"t-p06b".into(),
                execution_scope_id:"work-p06b".into(),
                capabilities:vec![AuthorizedCapability{
                    tool_id:"lu.external.version.execute".into(),
                    effect:ToolEffect::ExecuteCode,
                }],
                expires_at:None,
            },&RealUser(&book)).unwrap();
            let tasks=Arc::new(TaskStore::open(&root.join("tasks.sqlite")).unwrap());
            let effects=Arc::new(EffectJournal::open(&root.join("effects.sqlite")).unwrap());
            let workers=Arc::new(BackgroundProcessSupervisor::new(
                Arc::clone(&tasks)
            ));
            let mut catalog=HostThirdPartyAdapterCatalog::new();
            let digest=format!("{:x}",Sha256::digest(fs::read(binary).unwrap()));
            catalog.register_host_reviewed(HostThirdPartyVersionAdapter{
                adapter_id:"version".into(),
                app:HostReviewedExecutable{
                    app_id:"externally-installed-image".into(),
                    app_version:"trusted-version-label".into(),
                    tool_id:"lu.external.read".into(),
                    workspace_id:"work-p06b".into(),
                    executable:binary.to_path_buf(),
                    expected_sha256:digest,
                },
                execute_tool_id:"lu.external.version.execute".into(),
                fixed_flag:if slow{
                    VersionFlag::DelayedPythonVersionForNativeCancellationTest
                }else{
                    VersionFlag::DoubleDashVersion
                },
                version_parser:parser,output_limit:512,
                max_runtime_seconds:limit_seconds,
            }).unwrap();
            Self{root,_book:book,auth,tasks,effects,workers,catalog}
        }
        fn broker(&self)->ThirdPartyVersionBroker{
            ThirdPartyVersionBroker::new(
                self.auth.clone(),Arc::clone(&self.workers),
                Arc::clone(&self.tasks),Arc::clone(&self.effects)
            ).unwrap()
        }
        fn effect_state(&self)->EffectState{
            self.effects.get(&call().call_id).unwrap().unwrap().state
        }
    }
    #[test]
    #[ignore="requires real installed Git binary; dedicated Windows/Linux native CI"]
    fn actual_installed_git_stdout_is_parsed_after_os_reap_and_persisted(){
        let git=required_executable("LU_E1_P06_GIT");
        let fixture=Fixture::new(&git,VersionParser::Git,false,15);
        let broker=fixture.broker();
        assert_eq!(broker.launch(
            &fixture.catalog,"version",&origin(),&call(),
            &descriptor(),&Policy(false)
        ),Err(ThirdPartyVersionError::UnauthorizedOrChanged));
        assert!(fixture.effects.get(&call().call_id).unwrap().is_none());
        let mut fake_origin=origin();
        fake_origin.user_turn_id="u-model".into();
        assert_eq!(broker.launch(
            &fixture.catalog,"version",&fake_origin,&call(),
            &descriptor(),&Policy(true)
        ),Err(ThirdPartyVersionError::UnauthorizedOrChanged));
        assert!(fixture.effects.get(&call().call_id).unwrap().is_none());
        broker.launch(&fixture.catalog,"version",&origin(),&call(),
                      &descriptor(),&Policy(true)).unwrap();
        let mut done=false;
        for _ in 0..750{
            broker.poll().unwrap();
            if broker.active_count().unwrap()==0{done=true;break}
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(done,"Real installed Git must exit and be reaped");
        assert!(!fixture.workers.can_cancel(&call().call_id));
        assert_eq!(fixture.tasks.get(&call().call_id).unwrap().unwrap().state,
            TaskState::Completed);
        assert_eq!(fixture.effect_state(),EffectState::Applied);
        let receipt=fixture.effects.get(&call().call_id).unwrap().unwrap().receipt.unwrap();
        let value:serde_json::Value=serde_json::from_slice(&receipt).unwrap();
        assert_eq!(value["schema"],"lu-e1-p06b-real-thirdparty-version-v1");
        assert_eq!(value["applicationKind"],"git");
        println!("E1_P06B_REAL_GIT_VERSION parsed={} stdoutSha256={}",
            value["parsedVersion"].as_str().unwrap_or("unavailable"),
            value["observedStdoutSha256"].as_str().unwrap_or("unavailable"));
        assert_eq!(value["osProcessReapedBySupervisor"],true);
        assert_eq!(value["semanticVersionLineVerified"],true);
        assert!(value["parsedVersion"].as_str().is_some_and(|s|
            s.len()>2&&s.as_bytes()[0].is_ascii_digit()));
        assert_eq!(broker.launch(&fixture.catalog,"version",&origin(),&call(),
                     &descriptor(),&Policy(true)),
            Err(ThirdPartyVersionError::DuplicateEffect));
        let dir=fixture.root.clone();
        drop(broker);drop(fixture);
        let reopened=EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
        assert_eq!(reopened.get(&call().call_id).unwrap().unwrap().state,
            EffectState::Applied);
        drop(reopened);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore="requires real third-party Python sleeping child; dedicated OS native CI"]
    fn real_python_process_cancel_origin_and_revocation_reap_before_uncertain(){
        let py=required_executable("LU_E1_P06_PYTHON");
        let fixture=Fixture::new(&py,VersionParser::Python,true,15);
        let broker=fixture.broker();
        broker.launch(&fixture.catalog,"version",&origin(),&call(),
                      &descriptor(),&Policy(true)).unwrap();
        assert!(fixture.workers.can_cancel(&call().call_id));
        let mut fake=origin();fake.user_turn_id="u-model".into();
        assert_eq!(broker.cancel_current_task(&fake),
            Err(ThirdPartyVersionError::WrongCancellationOrigin));
        assert!(fixture.workers.can_cancel(&call().call_id));
        broker.cancel_current_task(&origin()).unwrap();
        assert_eq!(broker.active_count().unwrap(),0);
        assert!(!fixture.workers.can_cancel(&call().call_id));
        assert_eq!(fixture.tasks.get(&call().call_id).unwrap().unwrap().state,
            TaskState::Cancelled);
        assert_eq!(fixture.effect_state(),EffectState::Uncertain);
        assert!(fixture.workers.take_local_version_stdout(
            &call().call_id,"work-p06b"
        ).unwrap().is_none());
        assert_eq!(broker.launch(&fixture.catalog,"version",&origin(),&call(),
                     &descriptor(),&Policy(true)),
            Err(ThirdPartyVersionError::DuplicateEffect));
        let dir=fixture.root.clone();
        drop(broker);drop(fixture);
        let reopened=EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
        assert_eq!(reopened.get(&call().call_id).unwrap().unwrap().state,
            EffectState::Uncertain);
        drop(reopened);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore="requires real installed Python process revocation; dedicated OS native CI"]
    fn revoking_original_user_task_kills_real_python_and_denies_late_receipt(){
        let py=required_executable("LU_E1_P06_PYTHON");
        let fixture=Fixture::new(&py,VersionParser::Python,true,15);
        let broker=fixture.broker();
        broker.launch(&fixture.catalog,"version",&origin(),&call(),
                      &descriptor(),&Policy(true)).unwrap();
        fixture.auth.revoke_task("t-p06b").unwrap();
        assert!(!fixture.workers.can_cancel(&call().call_id));
        assert_eq!(broker.active_count().unwrap(),0);
        assert_eq!(fixture.effect_state(),EffectState::Uncertain);
        assert_eq!(fixture.tasks.get(&call().call_id).unwrap().unwrap().state,
            TaskState::Cancelled);
        let mut next=call();next.call_id="call-revoked-new".into();
        assert_eq!(broker.launch(&fixture.catalog,"version",&origin(),&next,
                     &descriptor(),&Policy(true)),
            Err(ThirdPartyVersionError::UnauthorizedOrChanged));
        assert!(fixture.effects.get(&next.call_id).unwrap().is_none());
        let dir=fixture.root.clone();
        drop(broker);drop(fixture);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore="requires actual installed Python producing oversized stdout"]
    fn actual_python_excess_stdout_is_rejected_and_never_receipted(){
        let py=required_executable("LU_E1_P06_PYTHON");
        let mut fixture=Fixture::new(&py,VersionParser::Python,true,15);
        let mut profile=fixture.catalog.reviewed_for_execution("version").unwrap();
        fixture.catalog.delist("version").unwrap();
        profile.fixed_flag=VersionFlag::OverlongPythonVersionForNativeOutputTest;
        fixture.catalog.register_host_reviewed(profile).unwrap();
        let broker=fixture.broker();
        broker.launch(&fixture.catalog,"version",&origin(),&call(),
                      &descriptor(),&Policy(true)).unwrap();
        let mut finished=false;
        for _ in 0..750{
            broker.poll().unwrap();
            if broker.active_count().unwrap()==0{
                finished=true;break
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(finished);
        assert!(!fixture.workers.can_cancel(&call().call_id));
        assert_eq!(fixture.effect_state(),EffectState::Uncertain);
        assert!(fixture.workers.take_local_version_stdout(
            &call().call_id,"work-p06b"
        ).unwrap().is_none());
        assert!(matches!(fixture.tasks.get(&call().call_id).unwrap().unwrap().state,
            TaskState::Failed|TaskState::TimedOut));
        assert!(fixture.effects.get(&call().call_id).unwrap().unwrap().receipt.is_none());
        let dir=fixture.root.clone();
        drop(broker);drop(fixture);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore="requires real external Python worker and durable host restart path"]
    fn host_owner_drop_cancels_real_python_and_reopened_journal_never_replays(){
        let py=required_executable("LU_E1_P06_PYTHON");
        let fixture=Fixture::new(&py,VersionParser::Python,true,15);
        let broker=fixture.broker();
        broker.launch(&fixture.catalog,"version",&origin(),&call(),
                      &descriptor(),&Policy(true)).unwrap();
        assert!(fixture.workers.can_cancel(&call().call_id));
        drop(broker); // Host closure must actually kill/reap owned OS child.
        assert!(!fixture.workers.can_cancel(&call().call_id));
        assert_eq!(fixture.effect_state(),EffectState::Uncertain);
        assert_eq!(fixture.tasks.get(&call().call_id).unwrap().unwrap().state,
            TaskState::Cancelled);
        let dir=fixture.root.clone();
        drop(fixture);
        let tasks=TaskStore::open(&dir.join("tasks.sqlite")).unwrap();
        let effects=EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
        let _=crate::native_app_execution::reconcile_after_exclusive_host_restart(
            &tasks,&effects
        ).unwrap();
        assert_eq!(effects.prepare(&call()).unwrap(),
            PrepareOutcome::Existing(EffectState::Uncertain));
        drop(tasks);drop(effects);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore="requires actual OS deadline-killed Python process; dedicated native CI"]
    fn actual_python_timeout_is_reaped_uncertain_and_restart_never_replays(){
        let py=required_executable("LU_E1_P06_PYTHON");
        let fixture=Fixture::new(&py,VersionParser::Python,true,1);
        let broker=fixture.broker();
        broker.launch(&fixture.catalog,"version",&origin(),&call(),
                      &descriptor(),&Policy(true)).unwrap();
        std::thread::sleep(Duration::from_millis(1250));
        broker.poll().unwrap();
        assert!(!fixture.workers.can_cancel(&call().call_id));
        assert_eq!(broker.active_count().unwrap(),0);
        assert_eq!(fixture.tasks.get(&call().call_id).unwrap().unwrap().state,
            TaskState::TimedOut);
        assert_eq!(fixture.effect_state(),EffectState::Uncertain);
        let dir=fixture.root.clone();
        drop(broker);drop(fixture);
        let tasks=TaskStore::open(&dir.join("tasks.sqlite")).unwrap();
        let effects=EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
        let _=crate::native_app_execution::reconcile_after_exclusive_host_restart(
            &tasks,&effects
        ).unwrap();
        assert_eq!(effects.get(&call().call_id).unwrap().unwrap().state,
            EffectState::Uncertain);
        assert_eq!(effects.prepare(&call()).unwrap(),
            PrepareOutcome::Existing(EffectState::Uncertain));
        drop(tasks);drop(effects);fs::remove_dir_all(dir).unwrap();
    }
}
