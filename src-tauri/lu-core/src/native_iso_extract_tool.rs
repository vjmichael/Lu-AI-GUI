//! Exact-grant ISO9660 single-file extraction. Model JSON is untrusted;
//! the host binds source, member, destination AND byte digest after a user
//! preview. Always dispatch through ConversationAuthority/EffectJournal.
use std::sync::Arc;
use cap_std::fs::Dir;
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{AuthorizationError,ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,
        ToolAuthorizer,CORE_CONTRACT_VERSION},
    native_iso9660::{iso_member_digest,extract_iso_file_new_checked,IsoError},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    registry::NativeToolRegistry,
    wire::proto,
};
pub const TOOL_ID:&str="lu.archive.iso.extract";
pub const MODEL_NAME:&str="lu_iso_extract";
pub const INPUT:&str="type.googleapis.com/lu.v1.IsoExtractRequest";
pub const OUTPUT:&str="type.googleapis.com/lu.v1.IsoExtractResult";
fn name_ok(s:&str)->bool{
    !s.is_empty() && s.len()<=240 && s!="." && s!=".."
       && !s.starts_with(".lu-stage-") && !s.ends_with('.') && !s.ends_with(' ')
       && !s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn source_ok(s:&str)->bool{name_ok(s)&&s.to_ascii_lowercase().ends_with(".iso")}
fn member_ok(s:&str)->bool{
    !s.is_empty() && s.len()<=1024 && !s.starts_with('/') && !s.contains('\\')
       && !s.chars().any(|c|c.is_control()||c==':')
       && s.split('/').all(|part|name_ok(part))
}
fn hash_ok(s:&str)->bool{
    s.len()==64 && s.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
pub struct ExactIsoExtractPolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub source_name:&'a str,
    pub member_path:&'a str,
    pub target_name:&'a str,
    pub expected_sha256:&'a str,
    pub trusted_source_root:&'a Dir,
}
impl ToolAuthorizer for ExactIsoExtractPolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,descriptor:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("ISO extraction not approved");
        if descriptor.id!=TOOL_ID || call.tool_id!=TOOL_ID
            || descriptor.effect!=ToolEffect::WriteWorkspace
            || call.input.type_url!=INPUT
            || !source_ok(self.source_name) || !member_ok(self.member_path)
            || !name_ok(self.target_name) || !hash_ok(self.expected_sha256){
            return Err(deny())
        }
        let request=proto::IsoExtractRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if request.encode_to_vec()!=call.input.bytes
            || request.source_name!=self.source_name
            || request.member_path!=self.member_path
            || request.target_name!=self.target_name
            || request.expected_sha256!=self.expected_sha256 {
            return Err(deny())
        }
        // A malicious swap between preview and dispatch is not authority.
        let actual=iso_member_digest(
            self.trusted_source_root,&request.source_name,&request.member_path
        ).map_err(|_|deny())?;
        if actual!=self.expected_sha256{return Err(deny())}
        self.host_policy.authorize(call,descriptor)
    }
}
fn failure(e:IsoError)->ToolFailure{
    let code=match e{
        IsoError::Publish(crate::native_atomic_write::AtomicWriteError::ExistingTarget)
            =>"TARGET_EXISTS",
        IsoError::Publish(crate::native_atomic_write::AtomicWriteError::PublishedNeedsReconciliation)
            =>"WRITE_UNCERTAIN",
        IsoError::SourceChanged=>"SOURCE_CHANGED",
        IsoError::TooLarge=>"TOO_LARGE",
        _=>"ISO_EXTRACT_REJECTED",
    };
    ToolFailure::new(code,"ISO extraction failed or needs reconciliation",false)
}
struct Extractor{descriptor:ToolDescriptor,source:Arc<Dir>,destination:Arc<Dir>}
impl NativeTool for Extractor{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let source=Arc::clone(&self.source);
        let destination=Arc::clone(&self.destination);
        Box::pin(async move{
            if call.input.type_url!=INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","ISO extract contract differs",false))
            }
            let request=proto::IsoExtractRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Malformed ISO extract request",false))?;
            if request.encode_to_vec()!=call.input.bytes
                || !source_ok(&request.source_name)
                || !member_ok(&request.member_path)
                || !name_ok(&request.target_name)
                || !hash_ok(&request.expected_sha256){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Invalid ISO extraction arguments",false))
            }
            tokio::task::spawn_blocking(move||{
                let receipt=extract_iso_file_new_checked(
                    &source,&request.source_name,&request.member_path,
                    &destination,&request.target_name,&call.call_id,&request.expected_sha256
                ).map_err(failure)?;
                let output=proto::IsoExtractResult{
                    target_name:receipt.target,
                    bytes_written:receipt.bytes_written as u64,
                    sha256:receipt.sha256,
                };
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),bytes:output.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","ISO worker completion uncertain",false))?
        })
    }
}
struct Codec;
impl ModelToolCodec for Codec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let fields=&["source_name","member_path","target_name","expected_sha256"];
        expect_fields(args,fields,fields)?;
        let get=|field:&str|args.get(field).and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments(format!("{field} must be text")));
        let source=get("source_name")?;
        let member=get("member_path")?;
        let target=get("target_name")?;
        let digest=get("expected_sha256")?;
        if !source_ok(source)||!member_ok(member)||!name_ok(target)||!hash_ok(digest){
            return Err(ModelToolError::InvalidArguments("Unsafe ISO extraction request".into()))
        }
        let input=proto::IsoExtractRequest{
            source_name:source.into(),member_path:member.into(),
            target_name:target.into(),expected_sha256:digest.into(),
        };
        Ok(ProtoPayload{type_url:INPUT.into(),bytes:input.encode_to_vec()})
    }
    fn decode(&self,output:&ProtoPayload)->Result<Value,ModelToolError>{
        let result=proto::IsoExtractResult::decode(output.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Invalid ISO extraction receipt".into()))?;
        if !name_ok(&result.target_name)||!hash_ok(&result.sha256)||
            result.bytes_written>256*1024*1024 {
            return Err(ModelToolError::InvalidResult("Untrusted ISO extraction receipt".into()))
        }
        Ok(json!({"target_name":result.target_name,"bytes_written":result.bytes_written,
            "sha256":result.sha256,"created":true}))
    }
}
/// Must be invoked ONLY with already-open host-owned directory capabilities;
/// catalog registration is inert until a persisted-human-turn Write grant
/// and this exact-data policy are checked by the durable dispatcher.
pub fn register_iso_extractor(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    source:Arc<Dir>,destination:Arc<Dir>,
)->Result<(),String>{
    let descriptor=ToolDescriptor{
        id:TOOL_ID.into(),display_name:"Extract one ISO9660 file to approved target".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.archive.iso".into(),display_name:"Lu ISO9660 approved extraction".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![descriptor.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(Extractor{descriptor,source,destination})])
        .map_err(|e|format!("ISO extraction registration failed: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:MODEL_NAME.into(),native_tool_id:TOOL_ID.into(),
        description:"Propose extracting one ISO9660 regular file. Requires exact human-approved source, member, target, and content digest. Never overwrites.".into(),
        parameters:json!({"type":"object","properties":{
            "source_name":{"type":"string"},"member_path":{"type":"string"},
            "target_name":{"type":"string"},"expected_sha256":{"type":"string"}
        },"required":["source_name","member_path","target_name","expected_sha256"],
        "additionalProperties":false}),
        codec:Arc::new(Codec),
    }).map_err(|e|format!("ISO extraction schema failed: {e:?}"))?;
    Ok(())
}
