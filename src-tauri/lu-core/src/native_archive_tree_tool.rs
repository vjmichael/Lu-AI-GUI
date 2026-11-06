//! Model-visible new-folder archive extraction guarded by a persisted human
//! turn, exact source manifest, trusted directory handles and durable journal.
//! A partial extraction returns WRITE_UNCERTAIN; NEVER auto-retry.
use std::sync::Arc;
use cap_std::fs::Dir;
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{AuthorizationError,ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,
        ToolAuthorizer,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    native_archive_tree::{
        preview_archive_tree,extract_archive_tree_new_cancellable,
        ArchiveCancellation,TreeExtractError,
    },
    native_linux_archive::detect_linux_archive,
    registry::NativeToolRegistry,wire::proto,
};
pub const TOOL_ID:&str="lu.archive.tree.extract";
pub const MODEL_NAME:&str="lu_archive_extract_folder";
pub const INPUT:&str="type.googleapis.com/lu.v1.ArchiveTreeExtractRequest";
pub const OUTPUT:&str="type.googleapis.com/lu.v1.ArchiveTreeExtractResult";
fn file_name(s:&str)->bool{
    !s.is_empty()&&s.len()<=200&&s!="."&&s!=".."
       &&!s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
       &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn archive_name(s:&str)->bool{
    file_name(s)&&(s.to_ascii_lowercase().ends_with(".zip")||
        detect_linux_archive(s).is_some())
}
fn hash_ok(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
pub struct ExactArchiveTreePolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub trusted_source:&'a Dir,
    pub source_name:&'a str,
    pub destination_folder:&'a str,
    pub manifest_sha256:&'a str,
}
impl ToolAuthorizer for ExactArchiveTreePolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
      ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("Archive folder extraction not approved");
        if desc.id!=TOOL_ID||call.tool_id!=TOOL_ID
            ||desc.effect!=ToolEffect::WriteWorkspace||call.input.type_url!=INPUT
            ||!archive_name(self.source_name)||!file_name(self.destination_folder)
            ||!hash_ok(self.manifest_sha256) {
            return Err(deny())
        }
        let r=proto::ArchiveTreeExtractRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if r.encode_to_vec()!=call.input.bytes||r.source_name!=self.source_name
            ||r.target_folder!=self.destination_folder
            ||r.approved_manifest_sha256!=self.manifest_sha256{
            return Err(deny())
        }
        if preview_archive_tree(self.trusted_source,&r.source_name)
            .map_err(|_|deny())?.sha256!=self.manifest_sha256{
            return Err(deny())
        }
        self.host_policy.authorize(call,desc)
    }
}
fn failure(e:TreeExtractError)->ToolFailure{
    let code=match e{
        TreeExtractError::TargetExists=>"TARGET_EXISTS",
        TreeExtractError::Cancelled=>"OPERATION_CANCELLED",
        TreeExtractError::PublishedNeedsReconciliation=>"WRITE_UNCERTAIN",
        TreeExtractError::ManifestChanged=>"SOURCE_CHANGED",
        TreeExtractError::TooLarge=>"TOO_LARGE",
        _=>"ARCHIVE_TREE_REJECTED",
    };
    ToolFailure::new(code,"Archive extraction rejected or requires recovery review",false)
}
struct Tool{descriptor:ToolDescriptor,source:Arc<Dir>,destination:Arc<Dir>,cancel:ArchiveCancellation}
impl NativeTool for Tool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let source=Arc::clone(&self.source);
        let destination=Arc::clone(&self.destination);
        let cancel=self.cancel.clone();
        Box::pin(async move{
            if call.input.type_url!=INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Wrong tree extraction type",false))
            }
            let r=proto::ArchiveTreeExtractRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Invalid tree extraction",false))?;
            if r.encode_to_vec()!=call.input.bytes||!archive_name(&r.source_name)
                ||!file_name(&r.target_folder)||!hash_ok(&r.approved_manifest_sha256){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Invalid tree extraction scope",false))
            }
            tokio::task::spawn_blocking(move||{
                let receipt=extract_archive_tree_new_cancellable(
                    &source,&r.source_name,&destination,&r.target_folder,
                    &r.approved_manifest_sha256,&cancel
                ).map_err(failure)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),
                    bytes:proto::ArchiveTreeExtractResult{
                        target_folder:receipt.destination_folder,
                        files_extracted:receipt.files_extracted as u32,
                        bytes_extracted:receipt.bytes_extracted,
                        manifest_sha256:receipt.manifest_sha256,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","Archive extraction worker outcome unknown",false))?
        })
    }
}
struct Codec;
impl ModelToolCodec for Codec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let fields=&["source_name","target_folder","approved_manifest_sha256"];
        expect_fields(args,fields,fields)?;
        let get=|key:&str|args.get(key).and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments(format!("{key} must be text")));
        let source=get("source_name")?;
        let target=get("target_folder")?;
        let digest=get("approved_manifest_sha256")?;
        if !archive_name(source)||!file_name(target)||!hash_ok(digest){
            return Err(ModelToolError::InvalidArguments("Invalid archive folder proposal".into()))
        }
        Ok(ProtoPayload{
            type_url:INPUT.into(),
            bytes:proto::ArchiveTreeExtractRequest{
                source_name:source.into(),target_folder:target.into(),
                approved_manifest_sha256:digest.into(),
            }.encode_to_vec(),
        })
    }
    fn decode(&self,output:&ProtoPayload)->Result<Value,ModelToolError>{
        let r=proto::ArchiveTreeExtractResult::decode(output.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Invalid extraction receipt".into()))?;
        if !file_name(&r.target_folder)||!hash_ok(&r.manifest_sha256)
            ||r.files_extracted>4096||r.bytes_extracted>256*1024*1024{
            return Err(ModelToolError::InvalidResult("Unsafe extraction receipt".into()))
        }
        Ok(json!({"target_folder":r.target_folder,"files_extracted":r.files_extracted,
            "bytes_extracted":r.bytes_extracted,"manifest_sha256":r.manifest_sha256,
            "created":true}))
    }
}
pub fn register_archive_tree_extractor(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    source:Arc<Dir>,destination:Arc<Dir>,
)->Result<(),String>{
    register_archive_tree_extractor_with_cancel(
        registry,catalog,source,destination,ArchiveCancellation::default()
    )
}
/// The host retains the cancellation handle. No caller-supplied model
/// arguments can cancel unrelated conversations or grant new authority.
pub fn register_archive_tree_extractor_with_cancel(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    source:Arc<Dir>,destination:Arc<Dir>,cancel:ArchiveCancellation,
)->Result<(),String>{
    let desc=ToolDescriptor{
        id:TOOL_ID.into(),display_name:"Extract archive into new folder".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.archive.tree".into(),
        display_name:"Lu new-folder archive extraction".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![desc.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(Tool{descriptor:desc,source,destination,cancel})])
        .map_err(|e|format!("Archive tree registration rejected: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:MODEL_NAME.into(),native_tool_id:TOOL_ID.into(),
        description:"Propose extracting an approved ZIP or TAR archive into a new folder. Requires exact human grant and entire manifest SHA; never overwrites.".into(),
        parameters:json!({"type":"object","properties":{
            "source_name":{"type":"string"},
            "target_folder":{"type":"string"},
            "approved_manifest_sha256":{"type":"string"}
        },"required":["source_name","target_folder","approved_manifest_sha256"],
        "additionalProperties":false}),
        codec:Arc::new(Codec),
    }).map_err(|e|format!("Archive tree codec rejected: {e:?}"))?;
    Ok(())
}
