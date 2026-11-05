//! TOOLS-11: scoped, read-only status of durable background tasks.
//! This is not a process launcher, cancellation grant, or task scheduler.
//! Original Jan tool domain: background (start/status/stop); the first
//! activation is status only, with independent host authorization.
use std::sync::Arc;
use prost::Message;
use crate::{
    contracts::{ModuleKind,ModuleManifest,NativeTool,ProtoPayload,ToolDescriptor,
        ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,CORE_CONTRACT_VERSION},
    registry::{NativeToolRegistry,RegistryError},
    tasks::{TaskState,TaskStore},
};
pub const TOOL_ID:&str="lu.background.status";
pub const INPUT_TYPE:&str="type.googleapis.com/lu.v1.BackgroundTaskStatusRequest";
pub const OUTPUT_TYPE:&str="type.googleapis.com/lu.v1.BackgroundTaskStatusReply";
#[derive(Clone,PartialEq,Message)]
pub struct StatusRequest{
    #[prost(string,tag="1")]
    pub task_id:String,
}
#[derive(Clone,PartialEq,Message)]
pub struct StatusReply{
    #[prost(string,tag="1")]
    pub task_id:String,
    #[prost(string,tag="2")]
    pub state:String,
    #[prost(uint32,tag="3")]
    pub progress:u32,
    #[prost(string,tag="4")]
    pub detail:String,
    #[prost(bool,tag="5")]
    pub interrupted:bool,
}
fn state_name(state:TaskState)->&'static str{
    match state{
        TaskState::Pending=>"PENDING",TaskState::Running=>"RUNNING",
        TaskState::Paused=>"PAUSED",TaskState::Completed=>"COMPLETED",
        TaskState::Failed=>"FAILED",TaskState::Interrupted=>"INTERRUPTED",
    }
}
fn valid_id(id:&str)->bool{
    !id.is_empty()&&id.len()<=128
      &&id.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
}
pub struct BackgroundStatusTool{
    descriptor:ToolDescriptor,
    store:Arc<TaskStore>,
}
impl BackgroundStatusTool{
    pub fn new(store:Arc<TaskStore>)->Self{
        Self{store,descriptor:ToolDescriptor{
            id:TOOL_ID.into(),display_name:"Inspect authorized background task".into(),
            effect:ToolEffect::ReadWorkspace,
            input_type_url:INPUT_TYPE.into(),output_type_url:OUTPUT_TYPE.into(),
        }}
    }
    fn inspect(&self,invocation:&ToolInvocation)->Result<StatusReply,ToolFailure>{
        if invocation.tool_id!=TOOL_ID||invocation.input.type_url!=INPUT_TYPE
           ||invocation.input.bytes.len()>256{
            return Err(ToolFailure::new("INVALID_TASK_STATUS_REQUEST",
                "unexpected task status envelope",false));
        }
        let requested=StatusRequest::decode(invocation.input.bytes.as_slice())
            .map_err(|_|ToolFailure::new("INVALID_TASK_STATUS_REQUEST","invalid protobuf",false))?;
        if requested.encode_to_vec()!=invocation.input.bytes
           ||!valid_id(&requested.task_id){
            return Err(ToolFailure::new("INVALID_TASK_STATUS_REQUEST",
                "noncanonical or invalid task ID",false));
        }
        // Do not let a model query every host task. The registry authorizer
        // checks ReadWorkspace; this is a second, independent ownership fence.
        let workspace=invocation.workspace_id.as_deref()
            .filter(|s|!s.is_empty()&&s.len()<=128)
            .ok_or_else(||ToolFailure::new("TASK_SCOPE_DENIED",
                "explicit workspace scope required",false))?;
        let record=self.store.get(&requested.task_id)
            .map_err(|_|ToolFailure::new("TASK_STORE_UNAVAILABLE",
                "task status unavailable",true))?
            .ok_or_else(||ToolFailure::new("TASK_NOT_FOUND","task not visible",false))?;
        if record.workspace_id.as_deref()!=Some(workspace){
            return Err(ToolFailure::new("TASK_NOT_FOUND","task not visible",false));
        }
        // Checkpoint data and host paths are deliberately never returned.
        Ok(StatusReply{task_id:record.task_id,state:state_name(record.state).into(),
            progress:record.progress.into(),detail:record.detail.chars().take(2048).collect(),
            interrupted:record.state==TaskState::Interrupted})
    }
}
impl NativeTool for BackgroundStatusTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,invocation:ToolInvocation)->ToolFuture<'_>{
        Box::pin(async move{
            let status=self.inspect(&invocation)?;
            Ok(ToolOutput{payload:ProtoPayload{
                type_url:OUTPUT_TYPE.into(),bytes:status.encode_to_vec()
            }})
        })
    }
}
pub fn register_background_status(
    registry:&NativeToolRegistry,store:Arc<TaskStore>
)->Result<(),RegistryError>{
    let implementation=Arc::new(BackgroundStatusTool::new(store));
    registry.register(ModuleManifest{
        id:"lu.background".into(),display_name:"Lu background task inspection".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![implementation.descriptor.clone()],panels:vec![],workflows:vec![],
    },vec![implementation as Arc<dyn NativeTool>])
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::contracts::{AuthorizationError,ToolAuthorizer};
    struct Authorize;
    impl ToolAuthorizer for Authorize{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{
            Ok(())
        }
    }
    struct Deny;
    impl ToolAuthorizer for Deny{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{
            Err(AuthorizationError::new("not authorized"))
        }
    }
    fn call(id:&str,workspace:Option<&str>)->ToolInvocation{
        ToolInvocation{call_id:"host-call-1".into(),tool_id:TOOL_ID.into(),
            workspace_id:workspace.map(str::to_owned),input:ProtoPayload{
                type_url:INPUT_TYPE.into(),
                bytes:StatusRequest{task_id:id.into()}.encode_to_vec(),
            }}
    }
    #[tokio::test]
    async fn scoped_status_is_typed_and_keeps_checkpoint_private(){
        let store=Arc::new(TaskStore::open(std::path::Path::new(":memory:")).unwrap());
        store.create("task-1",Some("owner"),Some("/sensitive/path")).unwrap();
        store.start("task-1").unwrap();
        store.checkpoint("task-1",37,"Rendering image",&[77,88,99]).unwrap();
        let registry=NativeToolRegistry::new();
        register_background_status(&registry,Arc::clone(&store)).unwrap();
        let result=registry.dispatch(call("task-1",Some("owner")),&Authorize).await.unwrap();
        assert_eq!(result.payload.type_url,OUTPUT_TYPE);
        let status=StatusReply::decode(result.payload.bytes.as_slice()).unwrap();
        assert_eq!(status.task_id,"task-1");
        assert_eq!(status.state,"RUNNING");
        assert_eq!(status.progress,37);
        assert_eq!(status.detail,"Rendering image");
        assert!(!status.interrupted);
        assert!(!String::from_utf8_lossy(&result.payload.bytes).contains("/sensitive/path"));
        assert!(registry.dispatch(call("task-1",Some("wrong")),&Authorize).await.is_err());
        assert!(registry.dispatch(call("task-1",None),&Authorize).await.is_err());
        assert!(registry.dispatch(call("task-1",Some("owner")),&Deny).await.is_err());
    }
    #[tokio::test]
    async fn malformed_messages_and_cross_scope_are_rejected(){
        let store=Arc::new(TaskStore::open(std::path::Path::new(":memory:")).unwrap());
        store.create("t2",Some("owner"),None).unwrap();
        let registry=NativeToolRegistry::new();
        register_background_status(&registry,store).unwrap();
        let mut request=call("t2",Some("owner"));
        request.input.bytes.push(0);
        assert!(registry.dispatch(request,&Authorize).await.is_err());
        let mut request=call("t2",Some("owner"));
        request.input.type_url="type.googleapis.com/lu.v1.ForgedTask".into();
        assert!(registry.dispatch(request,&Authorize).await.is_err());
        assert!(registry.dispatch(call("missing",Some("owner")),&Authorize).await.is_err());
        assert!(registry.dispatch(call("t2",Some("other")),&Authorize).await.is_err());
    }
    #[test]
    fn interrupted_state_remains_observation_not_auto_restart(){
        let store=Arc::new(TaskStore::open(std::path::Path::new(":memory:")).unwrap());
        store.create("job1",Some("owner"),None).unwrap();
        store.start("job1").unwrap();
        assert_eq!(store.reconcile_interrupted().unwrap(),1);
        let tool=BackgroundStatusTool::new(store);
        let state=tool.inspect(&call("job1",Some("owner"))).unwrap();
        assert_eq!(state.state,"INTERRUPTED");
        assert!(state.interrupted);
    }
}
