//! TOOLS-10D: browser tools registered in Lu's native tool registry.
//! A registration is scoped to a trusted user task and approved origins.
//! The runtime adapter cannot claim new authority by naming a tool: each call
//! rechecks the actual ConversationAuthority, effect and typed payload.
use crate::{
    browser_authorized_execution::{
      authorize_browser_operation,browser_descriptor,decode_browser_request,
      BrowserOriginScope,BROWSER_READ_TOOL,BROWSER_INTERACT_TOOL,
    },
    browser_model_codec::{BrowserModelCodec,BrowserModelKind},
    browser_worker_supervisor::BrowserOperation,
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    contracts::{
      ModuleKind,ModuleManifest,NativeTool,ProtoPayload,ToolAuthorizer,ToolDescriptor,
      ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,CORE_CONTRACT_VERSION,
    },
    model::{ModelToolCatalog,ModelToolDefinition,ModelToolCodec},
    registry::NativeToolRegistry,
    wire::proto,
};
use prost::Message;
use serde_json::Value;
use std::sync::Arc;

/// Host-owned bridge to an already running browser. Never implemented by a
/// webpage or model-controlled worker. It receives no arbitrary executable,
/// PID or profile directory and cannot expand its configured origin grant.
pub trait BrowserTaskRunner:Send+Sync {
    fn run(&self,operation:BrowserOperation,arguments:Value)
        ->Result<Value,String>;
}
pub struct ScopedBrowserTask {
    pub authority:ConversationAuthority,
    pub origin:BoundCallOrigin,
    pub scope:BrowserOriginScope,
    pub policy:Arc<dyn ToolAuthorizer>,
    pub runner:Arc<dyn BrowserTaskRunner>,
}
struct BrowserNativeTool {
    descriptor:ToolDescriptor,
    task:Arc<ScopedBrowserTask>,
}
impl NativeTool for BrowserNativeTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,invocation:ToolInvocation)->ToolFuture<'_>{
        let task=Arc::clone(&self.task);
        Box::pin(async move {
            tokio::task::spawn_blocking(move||{
                let (operation,args)=decode_browser_request(&invocation)
                    .map_err(|_|ToolFailure::new("INVALID_BROWSER_REQUEST",
                        "untrusted model browser payload rejected",false))?;
                authorize_browser_operation(
                    &task.authority,&task.origin,&invocation,task.policy.as_ref(),
                    &task.scope,operation,&args
                ).map_err(|_|ToolFailure::new("BROWSER_NOT_AUTHORIZED",
                    "browser task or requested origin is not authorized",false))?;
                let result=task.runner.run(operation,args)
                    .map_err(|_|ToolFailure::new("BROWSER_OPERATION_FAILED",
                        "native browser operation failed",false))?;
                let bytes=serde_json::to_vec(&result)
                    .map_err(|_|ToolFailure::new("INVALID_BROWSER_RESULT",
                        "browser result cannot be encoded",false))?;
                if !result.is_object()||bytes.len()>2_900_000 {
                    return Err(ToolFailure::new("INVALID_BROWSER_RESULT",
                        "browser output exceeds approved result contract",false));
                }
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:"type.googleapis.com/lu.v1.BrowserOperationResult".into(),
                    bytes:proto::BrowserOperationResult{result_json:bytes}.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new("BROWSER_WORKER_FAILED",
                "browser worker unexpectedly stopped",false))?
        })
    }
}
/// Only the host calls this after establishing a persisted authenticated USER
/// task. Inserting a codec into the catalog is discovery, not authorization:
/// ConversationAuthority and ToolAuthorizer are rechecked on every dispatch.
pub fn register_scoped_browser_tools(
    registry:&NativeToolRegistry,
    catalog:&mut ModelToolCatalog,
    task:ScopedBrowserTask,
)->Result<(),String>{
    let task=Arc::new(task);
    let read=browser_descriptor(ToolEffect::NetworkRead)
        .map_err(|_|"browser read descriptor unavailable")?;
    let write=browser_descriptor(ToolEffect::NetworkWrite)
        .map_err(|_|"browser interaction descriptor unavailable")?;
    registry.register(ModuleManifest{
        id:"lu.web.browser".into(),display_name:"Lu owned research browser".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![read.clone(),write.clone()],
        panels:vec![],workflows:vec![],
    },vec![
        Arc::new(BrowserNativeTool{descriptor:read,task:Arc::clone(&task)}) as Arc<dyn NativeTool>,
        Arc::new(BrowserNativeTool{descriptor:write,task}) as Arc<dyn NativeTool>,
    ]).map_err(|e|format!("browser registry declined registration: {e:?}"))?;
    for (name,id,kind,description) in [
        ("lu_browser_read",BROWSER_READ_TOOL,BrowserModelKind::Read,
            "Read and navigate pages on user-approved HTTPS origins."),
        ("lu_browser_interact",BROWSER_INTERACT_TOOL,BrowserModelKind::Interact,
            "Interact with an approved browser page only with a separate NetworkWrite grant."),
    ]{
        let codec=Arc::new(BrowserModelCodec(kind));
        let schema=codec.schema();
        catalog.insert(registry,ModelToolDefinition{
            name:name.into(),native_tool_id:id.into(),description:description.into(),
            parameters:schema,codec:codec as Arc<dyn ModelToolCodec>,
        }).map_err(|e|format!("browser model catalog refused registration: {e:?}"))?;
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{
        conversation_authority::{AuthorizedCapability,TaskAuthority,TrustedUserTurnVerifier},
        contracts::AuthorizationError,
    };
    use serde_json::json;
    use std::{collections::BTreeSet,time::{Duration,SystemTime}};
    struct TestVerifier;
    impl TrustedUserTurnVerifier for TestVerifier{
        fn verify_user_turn(&self,c:&str,u:&str)->bool{
            c=="chat"&&u=="user"
        }
    }
    struct HostPolicy;
    impl ToolAuthorizer for HostPolicy{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
          ->Result<(),AuthorizationError>{Ok(())}
    }
    struct CountingRunner(std::sync::Mutex<usize>);
    impl BrowserTaskRunner for CountingRunner{
        fn run(&self,_:BrowserOperation,_:Value)->Result<Value,String>{
            *self.0.lock().unwrap()+=1;
            Ok(json!({"status":"READ","text":"untrusted test page"}))
        }
    }
    #[tokio::test]
    async fn real_registry_dispatch_rechecks_read_grant_before_worker_execution(){
        let authority=ConversationAuthority::new();
        authority.open_user_task(TaskAuthority{
            conversation_id:"chat".into(),user_turn_id:"user".into(),
            task_id:"task".into(),execution_scope_id:"scope".into(),
            capabilities:vec![AuthorizedCapability{
                tool_id:BROWSER_READ_TOOL.into(),effect:ToolEffect::NetworkRead,
            }],expires_at:Some(SystemTime::now()+Duration::from_secs(90)),
        },&TestVerifier).unwrap();
        let runner=Arc::new(CountingRunner(std::sync::Mutex::new(0)));
        let registry=NativeToolRegistry::new();
        let mut catalog=ModelToolCatalog::new();
        register_scoped_browser_tools(&registry,&mut catalog,ScopedBrowserTask{
            authority:authority.clone(),
            origin:BoundCallOrigin{
                conversation_id:"chat".into(),user_turn_id:"user".into(),task_id:"task".into(),
            },
            scope:BrowserOriginScope::from_host_grant(vec!["https://example.com".into()]).unwrap(),
            policy:Arc::new(HostPolicy),runner:runner.clone(),
        }).unwrap();
        let mut advertised=BTreeSet::new();
        advertised.insert("lu_browser_read".into());
        let read=catalog.encode_call(&advertised,"lu_browser_read","read1",Some("scope"),
            r#"{"operation":"navigate","url":"https://example.com/article"}"#).unwrap();
        let response=registry.dispatch(read,&HostPolicy).await.unwrap();
        assert_eq!(response.payload.type_url,
            "type.googleapis.com/lu.v1.BrowserOperationResult");
        assert_eq!(*runner.0.lock().unwrap(),1);
        let untrusted=catalog.encode_call(&advertised,"lu_browser_read","read2",Some("scope"),
            r#"{"operation":"navigate","url":"https://unapproved.example"}"#).unwrap();
        assert!(registry.dispatch(untrusted,&HostPolicy).await.is_err());
        assert_eq!(*runner.0.lock().unwrap(),1);
        authority.revoke_task("task").unwrap();
        let later=catalog.encode_call(&advertised,"lu_browser_read","read3",Some("scope"),
            r#"{"operation":"read"}"#).unwrap();
        assert!(registry.dispatch(later,&HostPolicy).await.is_err());
        assert_eq!(*runner.0.lock().unwrap(),1);
    }
}
