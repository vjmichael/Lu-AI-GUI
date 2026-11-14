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

#[derive(Clone,PartialEq,Message)]
struct StartRequest {#[prost(string,tag="1")] kind:String}
#[derive(Clone,PartialEq,Message)]
struct StartReply {#[prost(string,tag="1")] id:String}
#[derive(Clone,PartialEq,Message)]
struct CancelRequest {#[prost(string,tag="1")] id:String}
#[derive(Clone,PartialEq,Message)]
struct CancelReply {#[prost(string,tag="1")] id:String}
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
    supervisor:Arc<BackgroundProcessSupervisor>,
}
impl NativeTool for CancelTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        Box::pin(async move{
            let request:CancelRequest=typed_request(&call,CANCEL,CANCEL_IN)?;
            if call.workspace_id.as_deref()!=Some(self.workspace.as_str())
                ||request.id!=self.target{
                return Err(failure("BACKGROUND_CANCEL_NOT_AUTHORIZED"))
            }
            {
                let mut used=self.used.lock()
                    .map_err(|_|failure("BACKGROUND_CANCEL_UNCERTAIN"))?;
                if *used{return Err(failure("BACKGROUND_CANCEL_ALREADY_USED"))}
                *used=true;
            }
            let manager=Arc::clone(&self.supervisor);
            let id=self.target.clone();
            let workspace=self.workspace.clone();
            tokio::task::spawn_blocking(move||manager.cancel_owned(&id,&workspace))
                .await.map_err(|_|failure("BACKGROUND_CANCEL_UNCERTAIN"))?
                .map_err(|_|failure("BACKGROUND_CANCEL_UNCERTAIN"))?;
            Ok(ToolOutput{payload:ProtoPayload{
                type_url:CANCEL_OUT.into(),
                bytes:CancelReply{id:request.id}.encode_to_vec(),
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
struct CancelCodec {target:String}
impl ModelToolCodec for CancelCodec{
    fn input_type_url(&self)->&'static str{CANCEL_IN}
    fn output_type_url(&self)->&'static str{CANCEL_OUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let id=strict(args,"task_id",&self.target)?;
        Ok(ProtoPayload{type_url:CANCEL_IN.into(),
            bytes:CancelRequest{id}.encode_to_vec()})
    }
    fn decode(&self,reply:&ProtoPayload)->Result<Value,ModelToolError>{
        if reply.type_url!=CANCEL_OUT{return Err(ModelToolError::InvalidResult(
            "wrong bound cancel response".into()))}
        let v=CancelReply::decode(reply.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("bad cancel response".into()))?;
        if v.id!=self.target{return Err(ModelToolError::InvalidResult(
            "wrong cancelled task".into()))}
        Ok(json!({"task_id":v.id,"state":"CANCELLED",
            "verified_by":"Lu-owned worker supervisor"}))
    }
}

/// Host only: plan MUST be built from a reviewed worker registry and approved
/// by a persisted real user turn. A fresh registry/plan is required each turn.
/// No filesystem path or executable ever enters model arguments. ToolEffect
/// ExecuteCode still needs independent per-turn PolicyEngine authorization.
pub fn register_bound_background_controls(
    registry:&NativeToolRegistry,store:Arc<TaskStore>,
    supervisor:Arc<BackgroundProcessSupervisor>,job_kind:&str,
    plan:TrustedWorkerPlan,
)->Result<ModelToolCatalog,String>{
    if !valid(job_kind)||!valid(&plan.task_id)||!valid(&plan.workspace_id)
        ||plan.expected_sha256.len()!=64
        ||!plan.expected_sha256.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err("invalid host-prepared task".into())
    }
    let workspace=plan.workspace_id.clone();
    let id=plan.task_id.clone();
    let mut catalog=register_model_background_observation(registry,store)?;
    let start=ToolDescriptor{id:START.into(),
        display_name:"Start host-selected reviewed worker".into(),
        effect:ToolEffect::ExecuteCode,
        input_type_url:START_IN.into(),output_type_url:START_OUT.into()};
    let cancel=ToolDescriptor{id:CANCEL.into(),
        display_name:"Stop exact Lu-owned worker".into(),
        effect:ToolEffect::ExecuteCode,
        input_type_url:CANCEL_IN.into(),output_type_url:CANCEL_OUT.into()};
    registry.register(ModuleManifest{id:"lu.bgstart".into(),
        display_name:"Lu host-bound task launch".into(),kind:ModuleKind::Core,
        contract_version:CORE_CONTRACT_VERSION,tools:vec![start.clone()],
        panels:vec![],workflows:vec![]},
        vec![Arc::new(StartTool{descriptor:start,
            workspace:workspace.clone(),kind:job_kind.into(),
            pending:Mutex::new(Some(plan)),supervisor:Arc::clone(&supervisor)})
            as Arc<dyn NativeTool>]
    ).map_err(|e|format!("start registration failed: {e:?}"))?;
    registry.register(ModuleManifest{id:"lu.bgcancel".into(),
        display_name:"Lu host-bound task cancellation".into(),kind:ModuleKind::Core,
        contract_version:CORE_CONTRACT_VERSION,tools:vec![cancel.clone()],
        panels:vec![],workflows:vec![]},
        vec![Arc::new(CancelTool{descriptor:cancel,
            workspace,target:id.clone(),used:Mutex::new(false),supervisor})
            as Arc<dyn NativeTool>]
    ).map_err(|e|format!("cancel registration failed: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:"lu_background_start".into(),native_tool_id:START.into(),
        description:"Run only the user-approved Lu-host-prepared job; no command, file path or PID parameters.".into(),
        parameters:json!({"type":"object","properties":{
            "job_kind":{"type":"string","enum":[job_kind]}
        },"required":["job_kind"],"additionalProperties":false}),
        codec:Arc::new(StartCodec{kind:job_kind.into()})
    }).map_err(|e|format!("start catalog failed: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:"lu_background_cancel".into(),native_tool_id:CANCEL.into(),
        description:"Cancel only this exact authorized task; no process IDs.".into(),
        parameters:json!({"type":"object","properties":{
            "task_id":{"type":"string","enum":[id]}
        },"required":["task_id"],"additionalProperties":false}),
        codec:Arc::new(CancelCodec{target:id})
    }).map_err(|e|format!("cancel catalog failed: {e:?}"))?;
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
