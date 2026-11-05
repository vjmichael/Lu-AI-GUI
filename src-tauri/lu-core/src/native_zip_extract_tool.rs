//! Native scoped ZIP extraction with exact target and content approval.
//! Registering this model tool is inert until a persisted USER turn and
//! durable EffectJournal authorize a specific source/member/output hash.
use std::sync::Arc;
use cap_std::fs::Dir;
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{AuthorizationError,ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,
        ToolAuthorizer,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    native_zip_extract::{
        extract_zip_member_new_checked,zip_member_sha256,ZipExtractError,
    },
    registry::NativeToolRegistry,wire::proto,
};
pub const TOOL_ID:&str="lu.archive.zip.extract";
pub const MODEL_NAME:&str="lu_zip_extract";
pub const INPUT:&str="type.googleapis.com/lu.v1.ZipExtractRequest";
pub const OUTPUT:&str="type.googleapis.com/lu.v1.ZipExtractResult";
fn basename(s:&str)->bool{
    !s.is_empty()&&s.len()<=240&&s!="."&&s!=".."
        && !s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
        && !s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn member(s:&str)->bool{
    !s.is_empty()&&s.len()<=1024&&!s.starts_with('/')
        && !s.chars().any(|c|c.is_control()||matches!(c,'\\'|':'))
        && s.split('/').all(|p|
            !p.is_empty()&&p!="."&&p!=".."&&!p.eq_ignore_ascii_case(".git")
            && !p.starts_with('-')&&!p.ends_with('.')&&!p.ends_with(' '))
}
fn digest(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
pub struct ExactZipExtractPolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub trusted_source:&'a Dir,
    pub archive_name:&'a str,
    pub member_path:&'a str,
    pub target_name:&'a str,
    pub expected_sha256:&'a str,
}
impl ToolAuthorizer for ExactZipExtractPolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("ZIP extraction not approved");
        if desc.id!=TOOL_ID||call.tool_id!=TOOL_ID
            ||desc.effect!=ToolEffect::WriteWorkspace||call.input.type_url!=INPUT
            ||!basename(self.archive_name)
            ||!self.archive_name.to_ascii_lowercase().ends_with(".zip")
            ||!member(self.member_path)||!basename(self.target_name)
            ||!digest(self.expected_sha256){return Err(deny())}
        let request=proto::ZipExtractRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if request.encode_to_vec()!=call.input.bytes
            ||request.archive_name!=self.archive_name
            ||request.member_path!=self.member_path
            ||request.target_name!=self.target_name
            ||request.expected_sha256!=self.expected_sha256{
            return Err(deny())
        }
        let current=zip_member_sha256(
            self.trusted_source,&request.archive_name,&request.member_path
        ).map_err(|_|deny())?;
        if current!=self.expected_sha256{return Err(deny())}
        self.host_policy.authorize(call,desc)
    }
}
fn failure(e:ZipExtractError)->ToolFailure{
    let code=match e{
        ZipExtractError::TargetExists=>"TARGET_EXISTS",
        ZipExtractError::PublishedNeedsReconciliation=>"WRITE_UNCERTAIN",
        ZipExtractError::SourceChanged=>"SOURCE_CHANGED",
        ZipExtractError::TooLarge=>"TOO_LARGE",
        ZipExtractError::UnsupportedMethod=>"ZIP_METHOD_UNSUPPORTED",
        _=>"ZIP_EXTRACT_REJECTED",
    };
    ToolFailure::new(code,"ZIP extraction failed or requires reconciliation",false)
}
struct Extractor{descriptor:ToolDescriptor,source:Arc<Dir>,destination:Arc<Dir>}
impl NativeTool for Extractor{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let source=Arc::clone(&self.source);
        let destination=Arc::clone(&self.destination);
        Box::pin(async move{
            if call.input.type_url!=INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Invalid ZIP extract type",false))
            }
            let r=proto::ZipExtractRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Invalid ZIP extract request",false))?;
            if r.encode_to_vec()!=call.input.bytes || !basename(&r.archive_name)
                ||!r.archive_name.to_ascii_lowercase().ends_with(".zip")
                ||!member(&r.member_path)||!basename(&r.target_name)||!digest(&r.expected_sha256){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Invalid ZIP extraction request",false))
            }
            tokio::task::spawn_blocking(move||{
                let receipt=extract_zip_member_new_checked(
                    &source,&r.archive_name,&r.member_path,
                    &destination,&r.target_name,&call.call_id,&r.expected_sha256
                ).map_err(failure)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),
                    bytes:proto::ZipExtractResult{
                        target_name:receipt.target_name,member_path:receipt.source_member,
                        bytes_written:receipt.bytes_written,sha256:receipt.sha256,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","ZIP extraction worker completion unknown",false))?
        })
    }
}
struct Codec;
impl ModelToolCodec for Codec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let fields=&["archive_name","member_path","target_name","expected_sha256"];
        expect_fields(args,fields,fields)?;
        let get=|k:&str|args.get(k).and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments(format!("{k} must be text")));
        let archive=get("archive_name")?;
        let member_path=get("member_path")?;
        let target=get("target_name")?;
        let hash=get("expected_sha256")?;
        if !basename(archive)||!archive.to_ascii_lowercase().ends_with(".zip")
            ||!member(member_path)||!basename(target)||!digest(hash){
            return Err(ModelToolError::InvalidArguments("Unsafe ZIP extraction".into()))
        }
        Ok(ProtoPayload{
            type_url:INPUT.into(),
            bytes:proto::ZipExtractRequest{
                archive_name:archive.into(),member_path:member_path.into(),
                target_name:target.into(),expected_sha256:hash.into(),
            }.encode_to_vec(),
        })
    }
    fn decode(&self,out:&ProtoPayload)->Result<Value,ModelToolError>{
        let r=proto::ZipExtractResult::decode(out.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Invalid ZIP receipt".into()))?;
        if !basename(&r.target_name)||!member(&r.member_path)||!digest(&r.sha256)
            ||r.bytes_written>64*1024*1024{
            return Err(ModelToolError::InvalidResult("Untrusted ZIP receipt".into()))
        }
        Ok(json!({"target_name":r.target_name,"member_path":r.member_path,
            "bytes_written":r.bytes_written,"sha256":r.sha256,"created":true}))
    }
}
pub fn register_zip_extractor(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    source:Arc<Dir>,destination:Arc<Dir>,
)->Result<(),String>{
    let desc=ToolDescriptor{
        id:TOOL_ID.into(),display_name:"Extract a ZIP member into new file".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.archive.zip.extractor".into(),
        display_name:"Lu ZIP selected-member extraction".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![desc.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(Extractor{descriptor:desc,source,destination})])
        .map_err(|e|format!("ZIP extractor registry rejected: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:MODEL_NAME.into(),native_tool_id:TOOL_ID.into(),
        description:"Propose extracting a selected uncompressed ZIP member into a new approved file; requires exact human grant and digest.".into(),
        parameters:json!({"type":"object","properties":{
            "archive_name":{"type":"string"},
            "member_path":{"type":"string"},
            "target_name":{"type":"string"},
            "expected_sha256":{"type":"string"}
        },"required":["archive_name","member_path","target_name","expected_sha256"],
        "additionalProperties":false}),
        codec:Arc::new(Codec),
    }).map_err(|e|format!("ZIP extractor schema rejected: {e:?}"))?;
    Ok(())
}
