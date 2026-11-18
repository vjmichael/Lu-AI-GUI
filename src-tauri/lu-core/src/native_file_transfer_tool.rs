//! Exact-granted, same-directory new-file copy and move. Host must pin a
//! trusted directory handle, saved human turn, destination and source digest.
//! Catalog visibility never authorizes an effect; durable journal owns replay.
use std::sync::Arc;
use cap_std::fs::Dir;
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{AuthorizationError,ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,
        ToolAuthorizer,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    native_file_transfer::{source_digest,copy_new_checked_in_dir,
        move_new_checked_in_dir,TransferError},
    registry::NativeToolRegistry,wire::proto,
};
pub const COPY_ID:&str="lu.files.transfer.copy_new";
pub const MOVE_ID:&str="lu.files.transfer.move_new";
pub const INPUT:&str="type.googleapis.com/lu.v1.FileTransferRequest";
pub const OUTPUT:&str="type.googleapis.com/lu.v1.FileTransferResult";
pub const COPY_MODEL:&str="lu_file_copy";
pub const MOVE_MODEL:&str="lu_file_move";
fn file_name(s:&str)->bool{
    !s.is_empty()&&s.len()<=240&&s!="."&&s!=".."
       && !s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
       && !s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn hash(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
pub struct ExactFileTransferPolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub source_root:&'a Dir,
    pub tool_id:&'a str,
    pub source_name:&'a str,
    pub target_name:&'a str,
    pub source_sha256:&'a str,
}
impl ToolAuthorizer for ExactFileTransferPolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("File transfer not approved");
        if !matches!(self.tool_id,COPY_ID|MOVE_ID)
            ||call.tool_id!=self.tool_id||desc.id!=self.tool_id
            ||desc.effect!=ToolEffect::WriteWorkspace||call.input.type_url!=INPUT
            ||!file_name(self.source_name)||!file_name(self.target_name)
            ||self.source_name.eq_ignore_ascii_case(self.target_name)
            ||!hash(self.source_sha256){
            return Err(deny())
        }
        let r=proto::FileTransferRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if r.encode_to_vec()!=call.input.bytes
            ||r.source_name!=self.source_name||r.target_name!=self.target_name
            ||r.expected_source_sha256!=self.source_sha256{
            return Err(deny())
        }
        let (_,observed)=source_digest(self.source_root,&r.source_name)
            .map_err(|_|deny())?;
        if observed!=self.source_sha256{return Err(deny())}
        self.host_policy.authorize(call,desc)
    }
}
fn failure(e:TransferError)->ToolFailure{
    let code=match e{
        TransferError::TargetExists=>"TARGET_EXISTS",
        TransferError::PublishedNeedsReconciliation=>"WRITE_UNCERTAIN",
        TransferError::SourceChanged=>"SOURCE_CHANGED",
        TransferError::TooLarge=>"TOO_LARGE",
        _=>"FILE_TRANSFER_REJECTED",
    };
    ToolFailure::new(code,"File transfer rejected or requires reconciliation",false)
}
struct Transfer{descriptor:ToolDescriptor,root:Arc<Dir>,move_source:bool}
impl NativeTool for Transfer{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let root=Arc::clone(&self.root);
        let move_source=self.move_source;
        Box::pin(async move{
            if call.input.type_url!=INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Wrong transfer type",false))
            }
            let r=proto::FileTransferRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Bad transfer request",false))?;
            if r.encode_to_vec()!=call.input.bytes||!file_name(&r.source_name)
                ||!file_name(&r.target_name)
                ||r.source_name.eq_ignore_ascii_case(&r.target_name)
                ||!hash(&r.expected_source_sha256){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Invalid transfer request",false))
            }
            tokio::task::spawn_blocking(move||{
                let receipt=if move_source{
                    move_new_checked_in_dir(
                        &root,&r.source_name,&r.target_name,&r.expected_source_sha256
                    )
                }else{
                    copy_new_checked_in_dir(
                        &root,&r.source_name,&r.target_name,&call.call_id,
                        &r.expected_source_sha256
                    )
                }.map_err(failure)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),
                    bytes:proto::FileTransferResult{
                        source_name:receipt.source,target_name:receipt.target,
                        bytes_written:receipt.bytes,sha256:receipt.sha256,
                        source_removed:receipt.source_removed,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","File transfer worker completion unknown",false))?
        })
    }
}
struct Codec;
impl ModelToolCodec for Codec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let fields=&["source_name","target_name","expected_source_sha256"];
        expect_fields(args,fields,fields)?;
        let get=|k:&str|args.get(k).and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments(format!("{k} must be text")));
        let source=get("source_name")?;
        let target=get("target_name")?;
        let digest=get("expected_source_sha256")?;
        if !file_name(source)||!file_name(target)||source.eq_ignore_ascii_case(target)
            ||!hash(digest){
            return Err(ModelToolError::InvalidArguments("Unsafe file transfer request".into()))
        }
        Ok(ProtoPayload{
            type_url:INPUT.into(),bytes:proto::FileTransferRequest{
                source_name:source.into(),target_name:target.into(),
                expected_source_sha256:digest.into(),
            }.encode_to_vec(),
        })
    }
    fn decode(&self,out:&ProtoPayload)->Result<Value,ModelToolError>{
        let r=proto::FileTransferResult::decode(out.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Malformed transfer receipt".into()))?;
        if !file_name(&r.source_name)||!file_name(&r.target_name)||!hash(&r.sha256)
            ||r.bytes_written>32*1024*1024{
            return Err(ModelToolError::InvalidResult("Unsafe transfer receipt".into()))
        }
        Ok(json!({"source_name":r.source_name,"target_name":r.target_name,
            "bytes_written":r.bytes_written,"sha256":r.sha256,
            "source_removed":r.source_removed}))
    }
}
pub fn register_file_transfer_tools(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,root:Arc<Dir>
)->Result<(),String>{
    let copy=ToolDescriptor{
        id:COPY_ID.into(),display_name:"Copy to new file".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    let move_desc=ToolDescriptor{
        id:MOVE_ID.into(),display_name:"Move to new filename".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.files.transfer".into(),display_name:"Lu safe file transfers".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![copy.clone(),move_desc.clone()],panels:vec![],workflows:vec![],
    },vec![
        Arc::new(Transfer{descriptor:copy,root:Arc::clone(&root),move_source:false}),
        Arc::new(Transfer{descriptor:move_desc,root,move_source:true}),
    ]).map_err(|e|format!("File transfer registry rejected: {e:?}"))?;
    for (name,id,description) in [
        (COPY_MODEL,COPY_ID,"Copy an approved regular file to a new filename; requires exact source digest and user grant."),
        (MOVE_MODEL,MOVE_ID,"Move an approved regular file to a new filename; requires exact source digest and user grant."),
    ]{
        catalog.insert(registry,ModelToolDefinition{
            name:name.into(),native_tool_id:id.into(),
            description:description.into(),
            parameters:json!({"type":"object","properties":{
                "source_name":{"type":"string"},"target_name":{"type":"string"},
                "expected_source_sha256":{"type":"string"}
            },"required":["source_name","target_name","expected_source_sha256"],
            "additionalProperties":false}),
            codec:Arc::new(Codec),
        }).map_err(|e|format!("File transfer schema rejected: {e:?}"))?;
    }
    Ok(())
}
