//! TOOLS-11: one-shot, host-prepared model start/cancel for approved jobs.
//! The model never chooses executable, argv, host paths, PID or work scope.
//! This is an ExecuteCode capability only after current-turn host authority.
//! Registration does not advertise the capability to any model by itself.

use std::sync::{Arc,Mutex};
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{ModuleKind,ModuleManifest,NativeTool,ProtoPayload,ToolDescriptor,
        ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    native_background_process::{BackgroundProcessSupervisor,TrustedWorkerPlan},
    native_background_result::register_model_background_observation,
    registry::NativeToolRegistry,
    tasks::TaskStore,
};

const START:&str="lu.bgstart.run";
const CANCEL:&str="lu.bgcancel.run";
const START_IN:&str="type.googleapis.com/lu.v1.BoundJobStart";
const START_OUT:&str="type.googleapis.com/lu.v1.BoundJobStarted";
const CANCEL_IN:&str="type.googleapis.com/lu.v1.BoundJobCancel";
const CANCEL_OUT:&str="type.googleapis.com/lu.v1.BoundJobCancelled";
const QUEUED_CANCEL_OUT:&str="type.googleapis.com/lu.v1.ReviewedScanQueueCancelled";
const QUEUED_CANCEL_IN:&str="type.googleapis.com/lu.v1.ReviewedScanQueueCancel";

#[derive(Clone,PartialEq,Message)]
struct StartRequest {#[prost(string,tag="1")] kind:String}
#[derive(Clone,PartialEq,Message)]
struct StartReply {#[prost(string,tag="1")] id:String}
#[derive(Clone,PartialEq,Message)]
struct CancelRequest {#[prost(string,tag="1")] id:String,#[prost(string,tag="2")] expected:String}
#[derive(Clone,PartialEq,Message)]
struct CancelReply {#[prost(string,tag="1")] id:String,#[prost(string,tag="2")] previous:String}
fn failure(code:&str)->ToolFailure{ToolFailure::new(code,code,false)}
fn valid(s:&str)->bool{
    !s.is_empty()&&s.len()<=128
        &&s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
}
fn typed_request<T:Message+Default>(call:&ToolInvocation,id:&str,kind:&str)
    ->Result<T,ToolFailure>{
    if call.tool_id!=id||call.input.type_url!=kind||call.input.bytes.len()>256{
        return Err(failure("INVALID_BACKGROUND_CONTROL"))
    }
    let request=T::decode(call.input.bytes.as_slice())
        .map_err(|_|failure("INVALID_BACKGROUND_CONTROL"))?;
    if request.encode_to_vec()!=call.input.bytes{
        return Err(failure("INVALID_BACKGROUND_CONTROL"))
    }
    Ok(request)
}

struct StartTool{
    descriptor:ToolDescriptor,workspace:String,kind:String,
    pending:Mutex<Option<TrustedWorkerPlan>>,
    supervisor:Arc<BackgroundProcessSupervisor>,
}
impl NativeTool for StartTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        Box::pin(async move{
            let request:StartRequest=typed_request(&call,START,START_IN)?;
            if call.workspace_id.as_deref()!=Some(self.workspace.as_str())
                ||request.kind!=self.kind{
                return Err(failure("BACKGROUND_START_NOT_AUTHORIZED"))
            }
            // Consume prior to the OS operation. Never replay after failure.
            let plan=self.pending.lock()
                .map_err(|_|failure("BACKGROUND_START_UNCERTAIN"))?
                .take().ok_or_else(||failure("BACKGROUND_START_ALREADY_USED"))?;
            let id=plan.task_id.clone();
            let manager=Arc::clone(&self.supervisor);
            tokio::task::spawn_blocking(move||manager.start_host_approved(plan))
                .await.map_err(|_|failure("BACKGROUND_START_UNCERTAIN"))?
                .map_err(|_|failure("BACKGROUND_START_FAILED"))?;
            Ok(ToolOutput{payload:ProtoPayload{
                type_url:START_OUT.into(),
                bytes:StartReply{id}.encode_to_vec(),
            }})
        })
    }
}

struct CancelTool{
    descriptor:ToolDescriptor,workspace:String,target:String,used:Mutex<bool>,
    store:Arc<TaskStore>,supervisor:Arc<BackgroundProcessSupervisor>,
    protected:bool,
    queued:Option<(Arc<crate::native_background_scheduler::BackgroundScheduler>,String)>,
}
impl NativeTool for CancelTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        Box::pin(async move{
            let request:CancelRequest=typed_request(&call,CANCEL,&self.descriptor.input_type_url)?;
            if call.workspace_id.as_deref()!=Some(self.workspace.as_str())
                ||request.id!=self.target||request.expected!=self.queued.as_ref().map_or("",|(_,v)|v.as_str()){
                return Err(failure("BACKGROUND_CANCEL_NOT_AUTHORIZED"))
            }
            {
                let mut used=self.used.lock()
                    .map_err(|_|failure("BACKGROUND_CANCEL_UNCERTAIN"))?;
                if *used{return Err(failure("BACKGROUND_CANCEL_ALREADY_USED"))}
                *used=true;
            }
            if let Some((scheduler,expected))=&self.queued{
                let scheduler=Arc::clone(scheduler);let expected=expected.clone();
                let id=self.target.clone();let workspace=self.workspace.clone();
                tokio::task::spawn_blocking(move||scheduler.cancel_reviewed_unstarted(&id,&workspace,&expected))
                    .await.map_err(|_|failure("BACKGROUND_CANCEL_UNCERTAIN"))?
                    .map_err(|_|failure("BACKGROUND_CANCEL_UNCERTAIN"))?;
                if self.store.get(&request.id).map_err(|_|failure("BACKGROUND_CANCEL_UNCERTAIN"))?.is_some()
                    ||self.supervisor.can_cancel(&request.id){
                    return Err(failure("BACKGROUND_CANCEL_UNCERTAIN"))
                }
                return Ok(ToolOutput{payload:ProtoPayload{type_url:QUEUED_CANCEL_OUT.into(),
                    bytes:CancelReply{id:request.id,previous:request.expected}.encode_to_vec()}})
            }
            let manager=Arc::clone(&self.supervisor);
            let id=self.target.clone();
            let workspace=self.workspace.clone();
            let protected=self.protected;
            tokio::task::spawn_blocking(move||if protected{manager.cancel_owned_protected(&id,&workspace)}
                else{manager.cancel_owned(&id,&workspace)})
                .await.map_err(|_|failure("BACKGROUND_CANCEL_UNCERTAIN"))?
                .map_err(|_|failure("BACKGROUND_CANCEL_UNCERTAIN"))?;
            // Native supervisor has verified termination. Independently read
            // the exact durable owner and terminal state before model feedback.
            let record=self.store.get(&request.id)
                .map_err(|_|failure("BACKGROUND_CANCEL_UNCERTAIN"))?
                .ok_or_else(||failure("BACKGROUND_CANCEL_UNCERTAIN"))?;
            if record.workspace_id.as_deref()!=Some(self.workspace.as_str())
                ||record.state.as_str()!="CANCELLED"||self.supervisor.can_cancel(&request.id){
                return Err(failure("BACKGROUND_CANCEL_UNCERTAIN"))
            }
            Ok(ToolOutput{payload:ProtoPayload{
                type_url:CANCEL_OUT.into(),
                bytes:CancelReply{id:request.id,previous:String::new()}.encode_to_vec(),
            }})
        })
    }
}
fn strict(args:&Map<String,Value>,key:&str,expected:&str)
    ->Result<String,ModelToolError>{
    expect_fields(args,&[key],&[key])?;
    let value=args.get(key).and_then(Value::as_str)
        .filter(|s|*s==expected)
        .ok_or_else(||ModelToolError::InvalidArguments(
            "action is not the exact host-prepared task".into()))?;
    Ok(value.into())
}
struct StartCodec {kind:String}
impl ModelToolCodec for StartCodec{
    fn input_type_url(&self)->&'static str{START_IN}
    fn output_type_url(&self)->&'static str{START_OUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let kind=strict(args,"job_kind",&self.kind)?;
        Ok(ProtoPayload{type_url:START_IN.into(),
            bytes:StartRequest{kind}.encode_to_vec()})
    }
    fn decode(&self,reply:&ProtoPayload)->Result<Value,ModelToolError>{
        if reply.type_url!=START_OUT{return Err(ModelToolError::InvalidResult(
            "wrong bound start response".into()))}
        let v=StartReply::decode(reply.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("bad start response".into()))?;
        if !valid(&v.id){return Err(ModelToolError::InvalidResult(
            "invalid started task".into()))}
        Ok(json!({"task_id":v.id,"started":true,
            "notice":"Lu supervises the task and sends completion; do not poll."}))
    }
}
struct CancelCodec {target:String,queued_state:Option<String>}
impl ModelToolCodec for CancelCodec{
    fn input_type_url(&self)->&'static str{if self.queued_state.is_some(){QUEUED_CANCEL_IN}else{CANCEL_IN}}
    fn output_type_url(&self)->&'static str{if self.queued_state.is_some(){QUEUED_CANCEL_OUT}else{CANCEL_OUT}}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let id=strict(args,"task_id",&self.target)?;
        Ok(ProtoPayload{type_url:self.input_type_url().into(),
            bytes:CancelRequest{id,expected:self.queued_state.clone().unwrap_or_default()}.encode_to_vec()})
    }
    fn decode(&self,reply:&ProtoPayload)->Result<Value,ModelToolError>{
        if reply.type_url!=self.output_type_url(){return Err(ModelToolError::InvalidResult(
            "wrong bound cancel response".into()))}
        let v=CancelReply::decode(reply.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("bad cancel response".into()))?;
        if v.id!=self.target||v.previous!=self.queued_state.as_deref().unwrap_or("")||v.encode_to_vec()!=reply.bytes{return Err(ModelToolError::InvalidResult(
            "wrong cancelled task".into()))}
        if let Some(previous)=&self.queued_state{
            return Ok(json!({"task_id":v.id,"state":"CANCELLED","previous_state":previous,"started":false}))
        }
        Ok(json!({"task_id":v.id,"state":"CANCELLED",
            "verified_by":"Lu-owned worker supervisor"}))
    }
}

/// Register only a host-reviewed launch in a fresh per-turn registry.
/// The caller still must obtain independent current-turn ExecuteCode authority.
/// This intentionally offers no cancellation tool in the launch-only registry.
pub fn register_bound_background_start(
    registry:&NativeToolRegistry,store:Arc<TaskStore>,
    supervisor:Arc<BackgroundProcessSupervisor>,job_kind:&str,
    plan:TrustedWorkerPlan,
)->Result<ModelToolCatalog,String>{
    validate_bound_plan(job_kind,&plan)?;
    let mut catalog=register_model_background_observation(registry,store)?;
    register_start(registry,&mut catalog,supervisor,job_kind,plan)?;
    Ok(catalog)
}

/// Register cancellation of a task already owned by this host and workspace.
/// No new plan, executable, process identifier or launch capability is accepted.
/// The caller must independently verify original-turn ownership and obtain
/// fresh human confirmation; dispatch checks authority and live custody again.
pub fn register_bound_background_cancel(
    registry:&NativeToolRegistry,store:Arc<TaskStore>,
    supervisor:Arc<BackgroundProcessSupervisor>,workspace:&str,target:&str,
)->Result<ModelToolCatalog,String>{
    if !valid(workspace)||!valid(target){
        return Err("invalid host-prepared cancellation".into())
    }
    let task=store.get(target).map_err(|_|"task store unavailable")?
        .ok_or("task unavailable")?;
    if task.workspace_id.as_deref()!=Some(workspace)
        ||task.state.as_str()!="RUNNING"||!supervisor.can_cancel(target){
        return Err("task not owned or not running".into())
    }
    let mut catalog=register_model_background_observation(registry,Arc::clone(&store))?;
    register_cancel(registry,&mut catalog,store,supervisor,workspace,target,false,None)?;
    Ok(catalog)
}


/// Production cancellation admits only the exact reviewed queue state or
/// a live kernel-owned worker. Neither grants a launch capability.
pub fn register_reviewed_background_cancel(
    registry:&NativeToolRegistry,store:Arc<TaskStore>,
    supervisor:Arc<BackgroundProcessSupervisor>,
    scheduler:Arc<crate::native_background_scheduler::BackgroundScheduler>,
    workspace:&str,target:&str,expected:&str,
)->Result<ModelToolCatalog,String>{
    let queued=match expected{
        "QUEUED"|"PAUSED"=>{
            if scheduler.reviewed_unstarted_state(target,workspace).map_err(|_|"Queue unavailable")?
                .as_deref()!=Some(expected){return Err("Review target unavailable".into())}
            Some((scheduler,expected.to_owned()))
        },
        "RUNNING"=>{
            let row=store.get(target).map_err(|_|"Task store unavailable")?.ok_or("Task unavailable")?;
            if row.workspace_id.as_deref()!=Some(workspace)||row.state.as_str()!="RUNNING"||
                scheduler.state_for(target,workspace).map_err(|_|"Schedule unavailable")?.as_deref()!=Some("RUNNING")||
                !supervisor.can_cancel_protected(target,workspace){return Err("Protected task unavailable".into())}
            None
        },
        _=>return Err("Invalid reviewed cancellation state".into()),
    };
    let mut catalog=ModelToolCatalog::new();
    register_cancel(registry,&mut catalog,store,supervisor,workspace,target,true,queued)?;
    Ok(catalog)
}

fn validate_bound_plan(job_kind:&str,plan:&TrustedWorkerPlan)->Result<(),String>{
    if !valid(job_kind)||!valid(&plan.task_id)||!valid(&plan.workspace_id)
        ||plan.expected_sha256.len()!=64
        ||!plan.expected_sha256.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err("invalid host-prepared task".into())
    }
    Ok(())
}

fn register_start(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    supervisor:Arc<BackgroundProcessSupervisor>,job_kind:&str,
    plan:TrustedWorkerPlan,
)->Result<(),String>{
    let workspace=plan.workspace_id.clone();
    let start=ToolDescriptor{id:START.into(),
        display_name:"Start host-selected reviewed worker".into(),
        effect:ToolEffect::ExecuteCode,
        input_type_url:START_IN.into(),output_type_url:START_OUT.into()};
    registry.register(ModuleManifest{id:"lu.bgstart".into(),
        display_name:"Lu host-bound task launch".into(),kind:ModuleKind::Core,
        contract_version:CORE_CONTRACT_VERSION,tools:vec![start.clone()],
        panels:vec![],workflows:vec![]},
        vec![Arc::new(StartTool{descriptor:start,
            workspace,kind:job_kind.into(),
            pending:Mutex::new(Some(plan)),supervisor})
            as Arc<dyn NativeTool>]
    ).map_err(|e|format!("start registration failed: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:"lu_background_start".into(),native_tool_id:START.into(),
        description:"Run only the user-approved Lu-host-prepared job; no command, file path or PID parameters.".into(),
        parameters:json!({"type":"object","properties":{
            "job_kind":{"type":"string","enum":[job_kind]}
        },"required":["job_kind"],"additionalProperties":false}),
        codec:Arc::new(StartCodec{kind:job_kind.into()})
    }).map_err(|e|format!("start catalog failed: {e:?}"))?;
    Ok(())
}

fn register_cancel(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    store:Arc<TaskStore>,supervisor:Arc<BackgroundProcessSupervisor>,
    workspace:&str,target:&str,protected:bool,
    queued:Option<(Arc<crate::native_background_scheduler::BackgroundScheduler>,String)>,
)->Result<(),String>{
    let queued_state=queued.as_ref().map(|(_,s)|s.clone());
    let cancel=ToolDescriptor{id:CANCEL.into(),
        display_name:"Stop exact Lu-owned worker".into(),
        effect:ToolEffect::ExecuteCode,
        input_type_url:if queued_state.is_some(){QUEUED_CANCEL_IN}else{CANCEL_IN}.into(),
        output_type_url:if queued_state.is_some(){QUEUED_CANCEL_OUT}else{CANCEL_OUT}.into()};
    registry.register(ModuleManifest{id:"lu.bgcancel".into(),
        display_name:"Lu host-bound task cancellation".into(),kind:ModuleKind::Core,
        contract_version:CORE_CONTRACT_VERSION,tools:vec![cancel.clone()],
        panels:vec![],workflows:vec![]},
        vec![Arc::new(CancelTool{descriptor:cancel,
            workspace:workspace.into(),target:target.into(),used:Mutex::new(false),
            store,supervisor,protected,queued})
            as Arc<dyn NativeTool>]
    ).map_err(|e|format!("cancel registration failed: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:"lu_background_cancel".into(),native_tool_id:CANCEL.into(),
        description:"Cancel only this exact authorized task; no process IDs.".into(),
        parameters:json!({"type":"object","properties":{
            "task_id":{"type":"string","enum":[target]}
        },"required":["task_id"],"additionalProperties":false}),
        codec:Arc::new(CancelCodec{target:target.into(),queued_state})
    }).map_err(|e|format!("cancel catalog failed: {e:?}"))?;
    Ok(())
}

/// Compatibility entrypoint for a pre-reviewed plan and its own task ID.
/// Production original-turn integration should use the separate entrypoints
/// so cancellation never confers a launch grant (or vice versa).
pub fn register_bound_background_controls(
    registry:&NativeToolRegistry,store:Arc<TaskStore>,
    supervisor:Arc<BackgroundProcessSupervisor>,job_kind:&str,
    plan:TrustedWorkerPlan,
)->Result<ModelToolCatalog,String>{
    validate_bound_plan(job_kind,&plan)?;
    let workspace=plan.workspace_id.clone();
    let id=plan.task_id.clone();
    let mut catalog=register_model_background_observation(registry,Arc::clone(&store))?;
    register_start(registry,&mut catalog,Arc::clone(&supervisor),job_kind,plan)?;
    register_cancel(registry,&mut catalog,store,supervisor,&workspace,&id,false,None)?;
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{AuthorizationError,ToolAuthorizer};
    use sha2::{Digest,Sha256};
    use std::{collections::BTreeSet,ffi::OsString,fs,path::{Path,PathBuf}};
    struct Allow;
    impl ToolAuthorizer for Allow{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
            ->Result<(),AuthorizationError>{Ok(())}
    }
    struct Deny;
    impl ToolAuthorizer for Deny{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
            ->Result<(),AuthorizationError>{
            Err(AuthorizationError::new("user did not approve"))
        }
    }
    fn plan()->TrustedWorkerPlan{
        #[cfg(unix)]
        let (exe,args)=(PathBuf::from("/bin/sleep").canonicalize().unwrap(),
            vec![OsString::from("30")]);
        #[cfg(windows)]
        let (exe,args)=(PathBuf::from(std::env::var_os("ComSpec").unwrap_or_else(||
                OsString::from(r"C:\Windows\System32\cmd.exe"))).canonicalize().unwrap(),
            vec![OsString::from("/D"),OsString::from("/C"),
                 OsString::from("ping -n 20 127.0.0.1 >NUL")]);
        let digest=format!("{:x}",Sha256::digest(fs::read(&exe).unwrap()));
        TrustedWorkerPlan{task_id:"worker-bound1".into(),workspace_id:"c-owner".into(),
            executable:exe,expected_sha256:digest,args,progress_protocol:false,
            max_runtime_seconds:Some(60)}
    }
    #[tokio::test]
    async fn reviewed_plan_only_runs_once_then_owned_cancel_is_one_shot(){
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let supervisor=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
        let registry=NativeToolRegistry::new();
        let model=register_bound_background_controls(&registry,
            Arc::clone(&store),Arc::clone(&supervisor),"verified_scan",plan()).unwrap();
        let allowed=BTreeSet::from(["lu_background_start".into(),
            "lu_background_cancel".into()]);
        let request=model.encode_call(&allowed,"lu_background_start","call-1",
            Some("c-owner"),r#"{"job_kind":"verified_scan"}"#).unwrap();
        assert!(registry.dispatch(request.clone(),&Deny).await.is_err());
        assert!(store.get("worker-bound1").unwrap().is_none());
        let wrong=model.encode_call(&allowed,"lu_background_start","call-2",
            Some("another"),r#"{"job_kind":"verified_scan"}"#).unwrap();
        assert!(registry.dispatch(wrong,&Allow).await.is_err());
        assert!(store.get("worker-bound1").unwrap().is_none());
        let response=registry.dispatch(request.clone(),&Allow).await.unwrap();
        assert_eq!(StartReply::decode(response.payload.bytes.as_slice()).unwrap().id,
            "worker-bound1");
        assert!(registry.dispatch(request,&Allow).await.is_err());
        assert!(supervisor.can_cancel("worker-bound1"));
        assert!(model.encode_call(&allowed,"lu_background_cancel","call-3",
            Some("c-owner"),r#"{"task_id":"wrong-worker"}"#).is_err());
        let cancel=model.encode_call(&allowed,"lu_background_cancel","call-4",
            Some("c-owner"),r#"{"task_id":"worker-bound1"}"#).unwrap();
        assert!(registry.dispatch(cancel.clone(),&Deny).await.is_err());
        assert!(registry.dispatch(cancel.clone(),&Allow).await.is_ok());
        assert!(registry.dispatch(cancel,&Allow).await.is_err());
        assert_eq!(store.get("worker-bound1").unwrap().unwrap().state.as_str(),
            "CANCELLED");
    }
    #[tokio::test]
    async fn fresh_turn_cancel_registry_never_grants_launch_or_foreign_task(){
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let supervisor=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
        let launch_registry=NativeToolRegistry::new();
        let launch=register_bound_background_start(&launch_registry,Arc::clone(&store),
            Arc::clone(&supervisor),"verified_scan",plan()).unwrap();
        let allowed=BTreeSet::from(["lu_background_start".into(),"lu_background_cancel".into()]);
        assert!(launch.encode_call(&allowed,"lu_background_cancel","wrong-launch-turn",
            Some("c-owner"),r#"{"task_id":"worker-bound1"}"#).is_err());
        let start=launch.encode_call(&allowed,"lu_background_start","fresh-launch",
            Some("c-owner"),r#"{"job_kind":"verified_scan"}"#).unwrap();
        assert!(launch_registry.dispatch(start.clone(),&Deny).await.is_err());
        assert!(store.get("worker-bound1").unwrap().is_none());
        assert!(launch_registry.dispatch(start,&Allow).await.is_ok());

        for (workspace,target) in [("another","worker-bound1"),("c-owner","absent")]{
            let rejected=NativeToolRegistry::new();
            assert!(register_bound_background_cancel(&rejected,Arc::clone(&store),
                Arc::clone(&supervisor),workspace,target).is_err());
        }
        let stop_registry=NativeToolRegistry::new();
        let stop=register_bound_background_cancel(&stop_registry,Arc::clone(&store),
            Arc::clone(&supervisor),"c-owner","worker-bound1").unwrap();
        assert!(stop.encode_call(&allowed,"lu_background_start","wrong-stop-turn",
            Some("c-owner"),r#"{"job_kind":"verified_scan"}"#).is_err());
        let foreign=stop.encode_call(&allowed,"lu_background_cancel","foreign-stop",
            Some("another"),r#"{"task_id":"worker-bound1"}"#).unwrap();
        assert!(stop_registry.dispatch(foreign,&Allow).await.is_err());
        let cancel=stop.encode_call(&allowed,"lu_background_cancel","fresh-stop",
            Some("c-owner"),r#"{"task_id":"worker-bound1"}"#).unwrap();
        assert!(stop_registry.dispatch(cancel.clone(),&Deny).await.is_err());
        assert!(supervisor.can_cancel("worker-bound1"));
        assert!(stop_registry.dispatch(cancel.clone(),&Allow).await.is_ok());
        assert!(stop_registry.dispatch(cancel,&Allow).await.is_err());
        assert!(!supervisor.can_cancel("worker-bound1"));
        assert_eq!(store.get("worker-bound1").unwrap().unwrap().state.as_str(),"CANCELLED");
        let stale=NativeToolRegistry::new();
        assert!(register_bound_background_cancel(&stale,store,supervisor,
            "c-owner","worker-bound1").is_err());
    }
    #[test]
    fn no_model_supplied_shell_executable_or_args(){
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let supervisor=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
        let registry=NativeToolRegistry::new();
        let model=register_bound_background_controls(&registry,store,
            supervisor,"verified_scan",plan()).unwrap();
        let allowed=BTreeSet::from(["lu_background_start".into()]);
        for bad in [r#"{"job_kind":"shell"}"#,
            r#"{"job_kind":"verified_scan","exe":"/bin/sh"}"#,
            r#"{"job_kind":"verified_scan","args":["-c","rm -rf /"]}"#,
            r#"{"job_kind":"verified_scan","workspace_id":"other"}"#,
            r#"{"job_kind":1}"#]{
            assert!(model.encode_call(&allowed,"lu_background_start",
                "call-test",Some("c-owner"),bad).is_err());
        }
        assert!(model.encode_call(&allowed,"lu_background_cancel",
            "call-test",Some("c-owner"),r#"{"task_id":"worker-bound1"}"#).is_err());
    }
}

// Production Windows start is a queue admission, never a direct process
// start in a foreground model turn. A separate receipt schema distinguishes
// an approved queued job from an already running or completed worker.
#[cfg(any(windows,target_os="linux"))]
mod queued_reviewed_start {
    use super::*;
    use crate::native_background_scheduler::{BackgroundScheduler,ReviewedScan};
    const IN:&str="type.googleapis.com/lu.v1.ReviewedScanQueue";
    const OUT:&str="type.googleapis.com/lu.v1.ReviewedScanQueued";
    const KIND:&str="file_scan";
    #[derive(Clone,PartialEq,Message)]
    struct Request{
        #[prost(string,tag="1")] kind:String,
        #[prost(string,tag="2")] id:String,
        #[prost(string,tag="3")] binding:String,
    }
    #[derive(Clone,PartialEq,Message)]
    struct Reply{
        #[prost(string,tag="1")] id:String,
        #[prost(string,tag="2")] binding:String,
    }
    struct QueueTool{
        descriptor:ToolDescriptor,workspace:String,id:String,binding:String,
        pending:Mutex<Option<ReviewedScan>>,scheduler:Arc<BackgroundScheduler>,
    }
    impl NativeTool for QueueTool{
        fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
        fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
            Box::pin(async move{
                let r:Request=typed_request(&call,START,IN)?;
                if call.workspace_id.as_deref()!=Some(self.workspace.as_str())
                    ||r.kind!=KIND||r.id!=self.id||r.binding!=self.binding{
                    return Err(failure("BACKGROUND_QUEUE_NOT_AUTHORIZED"))
                }
                let plan=self.pending.lock().map_err(|_|failure("BACKGROUND_QUEUE_UNCERTAIN"))?
                    .take().ok_or_else(||failure("BACKGROUND_QUEUE_ALREADY_USED"))?;
                let scheduler=Arc::clone(&self.scheduler);
                tokio::task::spawn_blocking(move||scheduler.enqueue_reviewed_scan(plan))
                    .await.map_err(|_|failure("BACKGROUND_QUEUE_UNCERTAIN"))?
                    .map_err(|_|failure("BACKGROUND_QUEUE_REJECTED"))?;
                // Confirm the owned durable queue row, not an execution claim.
                if self.scheduler.state_for(&self.id,&self.workspace).ok().flatten().as_deref()!=Some("QUEUED"){
                    return Err(failure("BACKGROUND_QUEUE_UNCERTAIN"))
                }
                Ok(ToolOutput{payload:ProtoPayload{type_url:OUT.into(),
                    bytes:Reply{id:self.id.clone(),binding:self.binding.clone()}.encode_to_vec()}})
            })
        }
    }
    struct Codec{id:String,binding:String}
    impl ModelToolCodec for Codec{
        fn input_type_url(&self)->&'static str{IN}
        fn output_type_url(&self)->&'static str{OUT}
        fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
            let kind=strict(args,"job_kind",KIND)?;
            Ok(ProtoPayload{type_url:IN.into(),bytes:Request{kind,id:self.id.clone(),binding:self.binding.clone()}.encode_to_vec()})
        }
        fn decode(&self,reply:&ProtoPayload)->Result<Value,ModelToolError>{
            let invalid=||ModelToolError::InvalidResult("invalid reviewed queue receipt".into());
            if reply.type_url!=OUT{return Err(invalid())}
            let r=Reply::decode(reply.bytes.as_slice()).map_err(|_|invalid())?;
            if r.encode_to_vec()!=reply.bytes||r.id!=self.id||r.binding!=self.binding{return Err(invalid())}
            Ok(json!({"task_id":r.id,"state":"QUEUED","started":false,"review_binding":r.binding}))
        }
    }
    pub fn register_reviewed_background_start(
        registry:&NativeToolRegistry,scheduler:Arc<BackgroundScheduler>,
        reviewed:ReviewedScan,binding:&str,
    )->Result<ModelToolCatalog,String>{
        if binding.len()!=64||!binding.bytes().all(|b|b.is_ascii_digit()||matches!(b,b'a'..=b'f'))
            ||!valid(reviewed.task_id())||!valid(reviewed.workspace()){
            return Err("invalid reviewed queue binding".into())
        }
        let workspace=reviewed.workspace().to_owned();let id=reviewed.task_id().to_owned();
        let descriptor=ToolDescriptor{id:START.into(),display_name:"Queue one approved file scan".into(),
            effect:ToolEffect::ExecuteCode,input_type_url:IN.into(),output_type_url:OUT.into()};
        registry.register(ModuleManifest{id:"lu.bgstart".into(),display_name:"Reviewed file scan queue".into(),
            kind:ModuleKind::Core,contract_version:CORE_CONTRACT_VERSION,tools:vec![descriptor.clone()],
            panels:vec![],workflows:vec![]},vec![Arc::new(QueueTool{descriptor,workspace,id:id.clone(),
                binding:binding.into(),pending:Mutex::new(Some(reviewed)),scheduler}) as Arc<dyn NativeTool>])
            .map_err(|e|format!("reviewed queue registration failed: {e:?}"))?;
        let mut catalog=ModelToolCatalog::new();
        catalog.insert(registry,ModelToolDefinition{name:"lu_background_start".into(),native_tool_id:START.into(),
            description:"Queue only the exact native-reviewed one-shot file scan. No paths, commands or scheduling parameters.".into(),
            parameters:json!({"type":"object","properties":{"job_kind":{"type":"string","enum":[KIND]}},
                "required":["job_kind"],"additionalProperties":false}),
            codec:Arc::new(Codec{id,binding:binding.into()})}).map_err(|e|format!("reviewed queue codec failed: {e:?}"))?;
        Ok(catalog)
    }
}
#[cfg(any(windows,target_os="linux"))]
pub use queued_reviewed_start::register_reviewed_background_start;
