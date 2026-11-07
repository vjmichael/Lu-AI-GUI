//! Foundational I/A: reviewed external-application action adapter.
//! This is the MODEL-TO-HOST INTERFACE ONLY, not a generic terminal, program
//! launcher, ambient OS permission grant, browser automation or installed app.
//!
//! Trusted host code chooses one precise operation, an effect classification,
//! an implementation and its separately admitted resources. The model sees
//! exactly ONE narrow tool name with a bounded text input. There is no
//! executable, argv, cwd, URL, credential or arbitrary command field.
//!
//! A future host worker must separately prove isolation, task-scoped resources,
//! cancellation, journaling and external result verification before release.
//! No external adapter is registered by default or through an add-on manifest.
use std::sync::Arc;
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{ModuleKind,ModuleManifest,NativeTool,ProtoPayload,ToolDescriptor,
        ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    registry::NativeToolRegistry,
};
pub const MAX_INPUT:usize=4096;
pub const MAX_RESULT:usize=16*1024;
const INPUT:&str="type.googleapis.com/lu.v1.ReviewedApplicationActionRequest";
const OUTPUT:&str="type.googleapis.com/lu.v1.ReviewedApplicationActionResult";
#[derive(Clone,PartialEq,Message)]
struct ActionRequest {
    #[prost(string,tag="1")]
    input:String,
}
#[derive(Clone,PartialEq,Message)]
struct ActionResult {
    #[prost(string,tag="1")]
    result:String,
    #[prost(bool,tag="2")]
    independently_verified:bool,
    #[prost(string,tag="3")]
    status:String,
}
/// Only trusted host Rust can supply and install an operation. Treat this as
/// non-authoritative registration; native dispatch still demands a separate
/// per-turn grant, policy and exact effect match.
#[derive(Clone)]
pub struct HostReviewedAction{
    pub module_id:String,
    pub tool_id:String,
    pub public_name:String,
    pub description:String,
    pub effect:ToolEffect,
}
fn identifier(id:&str)->bool{
    !id.is_empty()&&id.len()<=128&&id.bytes().all(|b|
       b.is_ascii_lowercase()||b.is_ascii_digit()||matches!(b,b'.'|b'_'|b'-'))
}
fn public_name(name:&str)->bool{
    !name.is_empty()&&name.len()<=64&&name.bytes().all(|b|
        b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'_')
}
/// This backend must be a trusted host-side implementation with its OWN
/// approved executable/args/profile, network restrictions and effect journal.
/// A plugin script, model output or webpage may not implement this trait.
pub trait ReviewedApplicationBackend:Send+Sync{
    fn perform(&self,input:&str)->Result<String,String>;
}
struct ApplicationAction{
    descriptor:ToolDescriptor,
    backend:Arc<dyn ReviewedApplicationBackend>,
}
fn refused(code:&str)->ToolFailure{ToolFailure::new(code,code,false)}
impl NativeTool for ApplicationAction{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let backend=Arc::clone(&self.backend);
        Box::pin(async move{
            let request=ActionRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|refused("APPLICATION_INPUT_INVALID"))?;
            if request.encode_to_vec()!=call.input.bytes||
               request.input.is_empty()||request.input.len()>MAX_INPUT||
               request.input.chars().any(|c|c=='\0'){
                return Err(refused("APPLICATION_INPUT_INVALID"))
            }
            // The host authorizer in the native registry must approve this
            // effect before invoke. The backend can only be provided by the host.
            let output=tokio::task::spawn_blocking(move||
                backend.perform(&request.input)
            ).await.map_err(|_|refused("APPLICATION_BACKEND_STOPPED"))?
                .map_err(|_|refused("APPLICATION_OPERATION_FAILED"))?;
            if output.len()>MAX_RESULT{
                return Err(refused("APPLICATION_OUTPUT_TOO_LARGE"))
            }
            let result=ActionResult{
                result:output,independently_verified:false,
                status:"unverified_backend_output".into()
            };
            Ok(ToolOutput{payload:ProtoPayload{
                type_url:OUTPUT.into(),bytes:result.encode_to_vec()
            }})
        })
    }
}
struct Codec;
impl ModelToolCodec for Codec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["input"],&["input"])?;
        let value=args.get("input").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("input must be text".into()))?;
        if value.is_empty()||value.len()>MAX_INPUT||value.chars().any(|c|c=='\0'){
            return Err(ModelToolError::InvalidArguments("invalid app input".into()))
        }
        Ok(ProtoPayload{type_url:INPUT.into(),bytes:ActionRequest{
            input:value.into()
        }.encode_to_vec()})
    }
    fn decode(&self,reply:&ProtoPayload)->Result<Value,ModelToolError>{
        let data=ActionResult::decode(reply.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("invalid app reply".into()))?;
        // A compromised backend must not promote its own claim to a
        // verified external effect through the model JSON response.
        if data.independently_verified||data.status!="unverified_backend_output"||
           data.result.len()>MAX_RESULT{
            return Err(ModelToolError::InvalidResult("untrusted app verification".into()))
        }
        Ok(json!({"result":data.result,"status":data.status,
            "independently_verified":false,
            "note":"External app output is unverified until independently inspected."}))
    }
}
pub fn register_host_reviewed_application_action(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    spec:HostReviewedAction,backend:Arc<dyn ReviewedApplicationBackend>
)->Result<(),String>{
    if !identifier(&spec.module_id)||!identifier(&spec.tool_id)||
       !public_name(&spec.public_name)||spec.description.trim().is_empty()||
       spec.description.len()>256{
        return Err("invalid host-reviewed action specification".into())
    }
    let descriptor=ToolDescriptor{id:spec.tool_id.clone(),
        display_name:spec.description.clone(),effect:spec.effect,
        input_type_url:INPUT.into(),output_type_url:OUTPUT.into()};
    registry.register(ModuleManifest{
        id:spec.module_id,display_name:"Host-reviewed external application".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![descriptor.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(ApplicationAction{descriptor,backend})])
        .map_err(|_|"external app native registration denied")?;
    catalog.insert(registry,ModelToolDefinition{
        name:spec.public_name,native_tool_id:spec.tool_id,
        description:spec.description,
        parameters:json!({"type":"object","properties":{
            "input":{"type":"string","maxLength":MAX_INPUT,
                "description":"Bounded input to a HOST-PINNED operation; not a command, path or URL"}
        },"required":["input"],"additionalProperties":false}),
        codec:Arc::new(Codec),
    }).map_err(|_|"external app model schema rejected")?;
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{contracts::{AuthorizationError,ToolAuthorizer},
        registry::DispatchError};
    use std::{collections::BTreeSet,sync::atomic::{AtomicUsize,Ordering}};
    struct Count(AtomicUsize);
    impl ReviewedApplicationBackend for Count{
        fn perform(&self,input:&str)->Result<String,String>{
            self.0.fetch_add(1,Ordering::SeqCst);
            Ok(format!("observed {}",input))
        }
    }
    struct Permit;
    impl ToolAuthorizer for Permit{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
          ->Result<(),AuthorizationError>{Ok(())}
    }
    struct Deny;
    impl ToolAuthorizer for Deny{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
          ->Result<(),AuthorizationError>{Err(AuthorizationError::new("not approved"))}
    }
    #[tokio::test]
    async fn only_host_installed_operation_can_execute_and_never_claims_verified(){
        let reg=NativeToolRegistry::new();let mut cat=ModelToolCatalog::new();
        let calls=Arc::new(Count(AtomicUsize::new(0)));
        register_host_reviewed_application_action(&reg,&mut cat,HostReviewedAction{
            module_id:"lu.reviewed.example".into(),
            tool_id:"lu.reviewed.example.echo".into(),
            public_name:"lu_approved_echo".into(),
            description:"Echo through host-approved synthetic test runner".into(),
            effect:ToolEffect::ExecuteCode,
        },calls.clone()).unwrap();
        let names=BTreeSet::from(["lu_approved_echo".to_owned()]);
        assert_eq!(cat.schemas(&names).unwrap().len(),1);
        for args in [
            r#"{"input":"x","command":"rm -rf /"}"#,
            r#"{"executable":"python"}"#,
            r#"{"input":""}"#,
        ]{assert!(cat.encode_call(&names,"lu_approved_echo","invalid",None,args).is_err());}
        let call=cat.encode_call(&names,"lu_approved_echo","call-1",None,
            r#"{"input":"hello"}"#).unwrap();
        assert!(matches!(reg.dispatch(call.clone(),&Deny).await,
            Err(DispatchError::Denied(_))));
        assert_eq!(calls.0.load(Ordering::SeqCst),0);
        let result=reg.dispatch(call,&Permit).await.unwrap();
        let response=cat.decode_result(&names,"lu_approved_echo",&Ok(result)).unwrap();
        assert_eq!(response["ok"],true);
        assert_eq!(response["result"]["independently_verified"],false);
        assert_eq!(calls.0.load(Ordering::SeqCst),1);
    }
}
