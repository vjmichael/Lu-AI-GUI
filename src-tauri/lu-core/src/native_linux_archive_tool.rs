//! Native Linux-archive model adapters for TAR, compressed TAR and compressed
//! single-stream files. They require a persisted-human-turn write capability,
 //! exact file/member/output digest host approval, and durable effect journal.
 //! Neither a JSON schema nor a directory registration grants write authority.
use std::{collections::BTreeSet,sync::Arc};
use cap_std::fs::Dir;
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{AuthorizationError,ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,
        ToolAuthorizer,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    native_linux_archive::{
        detect_linux_archive,preview_linux_archive,create_linux_archive_new_checked,
        linux_archive_member_digest,extract_linux_archive_member,LinuxArchiveError,
    },
    registry::NativeToolRegistry,
    wire::proto,
};
pub const CREATE_ID:&str="lu.archive.linux.create";
pub const EXTRACT_ID:&str="lu.archive.linux.extract";
pub const CREATE_MODEL:&str="lu_linux_archive_create";
pub const EXTRACT_MODEL:&str="lu_linux_archive_extract";
pub const CREATE_INPUT:&str="type.googleapis.com/lu.v1.LinuxArchiveCreateRequest";
pub const EXTRACT_INPUT:&str="type.googleapis.com/lu.v1.LinuxArchiveExtractRequest";
pub const OUTPUT:&str="type.googleapis.com/lu.v1.LinuxArchiveWriteResult";
fn file_name(s:&str)->bool{
    !s.is_empty() && s.len()<=240 && s!="." && s!=".."
    && !s.starts_with(".lu-stage-") && !s.ends_with('.') && !s.ends_with(' ')
    && !s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn archive_name(s:&str)->bool{file_name(s)&&detect_linux_archive(s).is_some()}
fn member_name(s:&str)->bool{
    !s.is_empty() && s.len()<=1024&&!s.starts_with('/')
    && !s.chars().any(|c|c.is_control()||matches!(c,'\\'|':'))
    && s.split('/').all(|part|
        !part.is_empty()&&part!="."&&part!=".."&&!part.eq_ignore_ascii_case(".git")
        && !part.starts_with('-')&&!part.ends_with('.')&&!part.ends_with(' '))
}
fn hash_valid(s:&str)->bool{
    s.len()==64 && s.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn inputs_valid(inputs:&[String])->bool{
    !inputs.is_empty()&&inputs.len()<=1024&&inputs.iter().all(|s|file_name(s))
        && inputs.iter().map(|s|s.to_lowercase()).collect::<BTreeSet<_>>().len()==inputs.len()
}
pub struct ExactLinuxExtractPolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub trusted_source_root:&'a Dir,
    pub archive_name:&'a str,
    pub member_path:&'a str,
    pub target_name:&'a str,
    pub expected_sha256:&'a str,
}
impl ToolAuthorizer for ExactLinuxExtractPolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
      ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("Linux archive extraction not approved");
        if desc.id!=EXTRACT_ID||call.tool_id!=EXTRACT_ID
            || desc.effect!=ToolEffect::WriteWorkspace || call.input.type_url!=EXTRACT_INPUT
            || !archive_name(self.archive_name)||!member_name(self.member_path)
            || !file_name(self.target_name)||!hash_valid(self.expected_sha256){
            return Err(deny())
        }
        let r=proto::LinuxArchiveExtractRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if r.encode_to_vec()!=call.input.bytes
            || r.archive_name!=self.archive_name || r.member_path!=self.member_path
            || r.target_name!=self.target_name
            || r.expected_sha256!=self.expected_sha256{
            return Err(deny())
        }
        let observed=linux_archive_member_digest(
            self.trusted_source_root,&r.archive_name,&r.member_path
        ).map_err(|_|deny())?;
        if observed!=self.expected_sha256{return Err(deny())}
        self.host_policy.authorize(call,desc)
    }
}
pub struct ExactLinuxCreatePolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub trusted_source_root:&'a Dir,
    pub archive_name:&'a str,
    pub input_names:&'a [String],
    pub expected_sha256:&'a str,
}
impl ToolAuthorizer for ExactLinuxCreatePolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
      ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("Linux archive creation not approved");
        if desc.id!=CREATE_ID||call.tool_id!=CREATE_ID
            || desc.effect!=ToolEffect::WriteWorkspace || call.input.type_url!=CREATE_INPUT
            || !archive_name(self.archive_name)||!inputs_valid(self.input_names)
            || !hash_valid(self.expected_sha256){
            return Err(deny())
        }
        let r=proto::LinuxArchiveCreateRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if r.encode_to_vec()!=call.input.bytes||r.archive_name!=self.archive_name
            || r.input_names.as_slice()!=self.input_names || r.expected_sha256!=self.expected_sha256 {
            return Err(deny())
        }
        let (_,observed)=preview_linux_archive(
            self.trusted_source_root,&r.archive_name,&r.input_names
        ).map_err(|_|deny())?;
        if observed!=self.expected_sha256{return Err(deny())}
        self.host_policy.authorize(call,desc)
    }
}
fn failure(e:LinuxArchiveError)->ToolFailure{
    let code=match e{
        LinuxArchiveError::TargetExists=>"TARGET_EXISTS",
        LinuxArchiveError::PublishedNeedsReconciliation=>"WRITE_UNCERTAIN",
        LinuxArchiveError::SourceChanged=>"SOURCE_CHANGED",
        LinuxArchiveError::TooLarge=>"TOO_LARGE",
        LinuxArchiveError::BackendUnavailable=>"ARCHIVE_BACKEND_MISSING",
        LinuxArchiveError::UnsupportedPlatform=>"UNSUPPORTED_PLATFORM",
        _=>"ARCHIVE_REJECTED",
    };
    ToolFailure::new(code,"Linux archive operation rejected or requires reconciliation",false)
}
struct Extractor{descriptor:ToolDescriptor,source:Arc<Dir>,destination:Arc<Dir>}
impl NativeTool for Extractor{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let source=Arc::clone(&self.source);
        let destination=Arc::clone(&self.destination);
        Box::pin(async move{
            if call.input.type_url!=EXTRACT_INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Wrong archive extraction type",false))
            }
            let r=proto::LinuxArchiveExtractRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Malformed archive request",false))?;
            if r.encode_to_vec()!=call.input.bytes||!archive_name(&r.archive_name)
                ||!member_name(&r.member_path)||!file_name(&r.target_name)
                ||!hash_valid(&r.expected_sha256){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Invalid archive extraction request",false))
            }
            tokio::task::spawn_blocking(move||{
                let receipt=extract_linux_archive_member(
                    &source,&r.archive_name,&r.member_path,&destination,&r.target_name,
                    &call.call_id,&r.expected_sha256
                ).map_err(failure)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),
                    bytes:proto::LinuxArchiveWriteResult{
                        target_name:receipt.target_name,bytes_written:receipt.bytes_written,
                        sha256:receipt.sha256,member_count:receipt.extracted_files as u32,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","Archive extraction worker completion unknown",false))?
        })
    }
}
struct Creator{descriptor:ToolDescriptor,root:Arc<Dir>}
impl NativeTool for Creator{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let root=Arc::clone(&self.root);
        Box::pin(async move{
            if call.input.type_url!=CREATE_INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Wrong archive creation type",false))
            }
            let r=proto::LinuxArchiveCreateRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Malformed archive request",false))?;
            if r.encode_to_vec()!=call.input.bytes||!archive_name(&r.archive_name)
                ||!inputs_valid(&r.input_names)||!hash_valid(&r.expected_sha256) {
                return Err(ToolFailure::new("BAD_ARGUMENTS","Invalid archive creation request",false))
            }
            tokio::task::spawn_blocking(move||{
                let receipt=create_linux_archive_new_checked(
                    &root,&r.archive_name,&r.input_names,&call.call_id,&r.expected_sha256
                ).map_err(failure)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),
                    bytes:proto::LinuxArchiveWriteResult{
                        target_name:receipt.target_name,bytes_written:receipt.bytes_written,
                        sha256:receipt.sha256,member_count:receipt.extracted_files as u32,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","Archive creation worker completion unknown",false))?
        })
    }
}
struct ExtractCodec;
impl ModelToolCodec for ExtractCodec{
    fn input_type_url(&self)->&'static str{EXTRACT_INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let fields=&["archive_name","member_path","target_name","expected_sha256"];
        expect_fields(args,fields,fields)?;
        let get=|name:&str|args.get(name).and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments(format!("{name} must be text")));
        let archive=get("archive_name")?;
        let member=get("member_path")?;
        let target=get("target_name")?;
        let digest=get("expected_sha256")?;
        if !archive_name(archive)||!member_name(member)||!file_name(target)
            ||!hash_valid(digest){
            return Err(ModelToolError::InvalidArguments("Invalid archive extraction".into()))
        }
        Ok(ProtoPayload{
            type_url:EXTRACT_INPUT.into(),
            bytes:proto::LinuxArchiveExtractRequest{
                archive_name:archive.into(),member_path:member.into(),
                target_name:target.into(),expected_sha256:digest.into(),
            }.encode_to_vec(),
        })
    }
    fn decode(&self,out:&ProtoPayload)->Result<Value,ModelToolError>{
        decode_receipt(out)
    }
}
struct CreateCodec;
impl ModelToolCodec for CreateCodec{
    fn input_type_url(&self)->&'static str{CREATE_INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let fields=&["archive_name","input_names","expected_sha256"];
        expect_fields(args,fields,fields)?;
        let name=args.get("archive_name").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("Archive name required".into()))?;
        let expected=args.get("expected_sha256").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("Content hash required".into()))?;
        let array=args.get("input_names").and_then(Value::as_array)
            .filter(|a|!a.is_empty()&&a.len()<=1024)
            .ok_or_else(||ModelToolError::InvalidArguments("Input names required".into()))?;
        let names=array.iter().map(|v|v.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(||ModelToolError::InvalidArguments("Input names must be text".into()))?;
        if !archive_name(name)||!hash_valid(expected)||!inputs_valid(&names) {
            return Err(ModelToolError::InvalidArguments("Invalid archive creation".into()))
        }
        Ok(ProtoPayload{
            type_url:CREATE_INPUT.into(),
            bytes:proto::LinuxArchiveCreateRequest{
                archive_name:name.into(),input_names:names,expected_sha256:expected.into(),
            }.encode_to_vec(),
        })
    }
    fn decode(&self,out:&ProtoPayload)->Result<Value,ModelToolError>{
        decode_receipt(out)
    }
}
fn decode_receipt(out:&ProtoPayload)->Result<Value,ModelToolError>{
    let r=proto::LinuxArchiveWriteResult::decode(out.bytes.as_slice())
        .map_err(|_|ModelToolError::InvalidResult("Invalid archive receipt".into()))?;
    if !file_name(&r.target_name)||!hash_valid(&r.sha256)
        ||r.bytes_written>128*1024*1024||r.member_count==0{
        return Err(ModelToolError::InvalidResult("Unsafe archive receipt".into()))
    }
    Ok(json!({"target_name":r.target_name,"bytes_written":r.bytes_written,
        "sha256":r.sha256,"member_count":r.member_count,"created":true}))
}
/// Host supplies approved source/destination directory handles; registration
/// does not grant either effect. Caller MUST use exact target/content policy
/// and persisted human user-turn with durable effect journal.
pub fn register_linux_archive_tools(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    source:Arc<Dir>,destination:Arc<Dir>,
)->Result<(),String>{
    let create=ToolDescriptor{
        id:CREATE_ID.into(),display_name:"Create a new Linux TAR/compressed archive".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:CREATE_INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    let extract=ToolDescriptor{
        id:EXTRACT_ID.into(),display_name:"Extract a selected TAR/compressed member".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:EXTRACT_INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.archive.linux".into(),display_name:"Lu Linux Archive operations".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![create.clone(),extract.clone()],panels:vec![],workflows:vec![],
    },vec![
        Arc::new(Creator{descriptor:create,root:Arc::clone(&destination)}),
        Arc::new(Extractor{descriptor:extract,source,destination}),
    ]).map_err(|e|format!("Linux archive registry rejected: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:CREATE_MODEL.into(),native_tool_id:CREATE_ID.into(),
        description:"Propose a new TAR/TAR.GZ/TAR.XZ/TAR.BZ2/TAR.ZST or standalone gzip/xz/bzip2/zstd file. Requires exact approved digest and human grant.".into(),
        parameters:json!({"type":"object","properties":{
            "archive_name":{"type":"string"},
            "input_names":{"type":"array","maxItems":1024,"items":{"type":"string"}},
            "expected_sha256":{"type":"string"}
        },"required":["archive_name","input_names","expected_sha256"],
        "additionalProperties":false}),
        codec:Arc::new(CreateCodec),
    }).map_err(|e|format!("Linux archive create schema rejected: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:EXTRACT_MODEL.into(),native_tool_id:EXTRACT_ID.into(),
        description:"Propose extracting one member from a TAR archive or standalone gzip/xz/bzip2/zstd stream into a NEW approved file. No overwrites.".into(),
        parameters:json!({"type":"object","properties":{
            "archive_name":{"type":"string"},"member_path":{"type":"string"},
            "target_name":{"type":"string"},"expected_sha256":{"type":"string"}
        },"required":["archive_name","member_path","target_name","expected_sha256"],
        "additionalProperties":false}),
        codec:Arc::new(ExtractCodec),
    }).map_err(|e|format!("Linux archive extract schema rejected: {e:?}"))?;
    Ok(())
}
