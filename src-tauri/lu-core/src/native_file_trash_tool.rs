//! Model-visible reversible file trash and host-receipt restore.
//! Both are WriteWorkspace effects requiring persisted human-turn authority,
//! trusted source/trash handles and a durable journal. No shell/OS recycle bin.
use std::sync::Arc;
use cap_std::fs::Dir;
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{AuthorizationError,ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,
        ToolAuthorizer,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    native_file_transfer::{
        source_digest,trash_checked_in_dirs,restore_from_trash_in_dirs,
        TrashReceipt,TransferError,
    },
    registry::NativeToolRegistry,wire::proto,
};
pub const TRASH_ID:&str="lu.files.trash";
pub const RESTORE_ID:&str="lu.files.restore";
pub const TRASH_MODEL:&str="lu_file_trash";
pub const RESTORE_MODEL:&str="lu_file_restore";
pub const TRASH_INPUT:&str="type.googleapis.com/lu.v1.FileTrashRequest";
pub const TRASH_OUTPUT:&str="type.googleapis.com/lu.v1.FileTrashResult";
pub const RESTORE_INPUT:&str="type.googleapis.com/lu.v1.FileRestoreRequest";
pub const RESTORE_OUTPUT:&str="type.googleapis.com/lu.v1.FileRestoreResult";
fn file_name(s:&str)->bool{
    !s.is_empty()&&s.len()<=240&&s!="."&&s!=".."
        &&!s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn hash_ok(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn id_ok(s:&str)->bool{
    s.starts_with("lu-trash-")&&(16..=128).contains(&(s.len()-9))
        &&s.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'-'||c==b'_')
}
pub struct ExactTrashPolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub trusted_source:&'a Dir,
    pub source_name:&'a str,
    pub source_sha256:&'a str,
}
impl ToolAuthorizer for ExactTrashPolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("Trash operation not approved");
        if desc.id!=TRASH_ID||call.tool_id!=TRASH_ID
            ||desc.effect!=ToolEffect::WriteWorkspace||call.input.type_url!=TRASH_INPUT
            ||!file_name(self.source_name)||!hash_ok(self.source_sha256){
            return Err(deny())
        }
        let r=proto::FileTrashRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if r.encode_to_vec()!=call.input.bytes||r.source_name!=self.source_name
            ||r.expected_sha256!=self.source_sha256{return Err(deny())}
        let (_,observed)=source_digest(self.trusted_source,&r.source_name)
            .map_err(|_|deny())?;
        if observed!=self.source_sha256{return Err(deny())}
        self.host_policy.authorize(call,desc)
    }
}
pub struct ExactRestorePolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub trusted_receipt_id:&'a str,
    pub receipt:&'a TrashReceipt,
}
impl ToolAuthorizer for ExactRestorePolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("Restore receipt not approved");
        if desc.id!=RESTORE_ID||call.tool_id!=RESTORE_ID
            ||desc.effect!=ToolEffect::WriteWorkspace||call.input.type_url!=RESTORE_INPUT
            ||!id_ok(self.trusted_receipt_id)
            ||self.receipt.trash_entry!=self.trusted_receipt_id
            ||!file_name(&self.receipt.original)||!hash_ok(&self.receipt.sha256){
            return Err(deny())
        }
        let r=proto::FileRestoreRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if r.encode_to_vec()!=call.input.bytes
            ||r.trusted_receipt_id!=self.trusted_receipt_id{return Err(deny())}
        self.host_policy.authorize(call,desc)
    }
}
fn failure(e:TransferError)->ToolFailure{
    let code=match e{
        TransferError::TargetExists=>"TARGET_EXISTS",
        TransferError::PublishedNeedsReconciliation=>"WRITE_UNCERTAIN",
        TransferError::SourceChanged=>"SOURCE_CHANGED",
        TransferError::TooLarge=>"TOO_LARGE",
        _=>"FILE_TRASH_REJECTED",
    };
    ToolFailure::new(code,"Reversible file operation rejected or needs recovery",false)
}
struct TrashTool{descriptor:ToolDescriptor,source:Arc<Dir>,trash:Arc<Dir>}
impl NativeTool for TrashTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let source=Arc::clone(&self.source);
        let trash=Arc::clone(&self.trash);
        Box::pin(async move{
            if call.input.type_url!=TRASH_INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Wrong trash request",false))
            }
            let r=proto::FileTrashRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Invalid trash request",false))?;
            if r.encode_to_vec()!=call.input.bytes||!file_name(&r.source_name)
                ||!hash_ok(&r.expected_sha256){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Unsafe trash request",false))
            }
            tokio::task::spawn_blocking(move||{
                let receipt=trash_checked_in_dirs(
                    &source,&trash,&r.source_name,&call.call_id,&r.expected_sha256
                ).map_err(failure)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:TRASH_OUTPUT.into(),
                    bytes:proto::FileTrashResult{
                        original_name:receipt.original,trash_entry:receipt.trash_entry,
                        bytes:receipt.bytes,sha256:receipt.sha256,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","Trash worker outcome unknown",false))?
        })
    }
}
struct RestoreTool{
    descriptor:ToolDescriptor,source:Arc<Dir>,trash:Arc<Dir>,
    receipt:TrashReceipt,
}
impl NativeTool for RestoreTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let source=Arc::clone(&self.source);
        let trash=Arc::clone(&self.trash);
        let receipt=self.receipt.clone();
        Box::pin(async move{
            if call.input.type_url!=RESTORE_INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Wrong restore request",false))
            }
            let r=proto::FileRestoreRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Invalid restore request",false))?;
            if r.encode_to_vec()!=call.input.bytes
                ||r.trusted_receipt_id!=receipt.trash_entry||!id_ok(&r.trusted_receipt_id){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Unknown restore receipt",false))
            }
            tokio::task::spawn_blocking(move||{
                let result=restore_from_trash_in_dirs(&source,&trash,&receipt)
                    .map_err(failure)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:RESTORE_OUTPUT.into(),
                    bytes:proto::FileRestoreResult{
                        restored_name:result.target,bytes:result.bytes,
                        sha256:result.sha256,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","Restore worker outcome unknown",false))?
        })
    }
}
struct TrashCodec;
impl ModelToolCodec for TrashCodec{
    fn input_type_url(&self)->&'static str{TRASH_INPUT}
    fn output_type_url(&self)->&'static str{TRASH_OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["source_name","expected_sha256"],
            &["source_name","expected_sha256"])?;
        let source=args.get("source_name").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("Source required".into()))?;
        let hash=args.get("expected_sha256").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("Source hash required".into()))?;
        if !file_name(source)||!hash_ok(hash){
            return Err(ModelToolError::InvalidArguments("Invalid trash proposal".into()))
        }
        Ok(ProtoPayload{
            type_url:TRASH_INPUT.into(),
            bytes:proto::FileTrashRequest{
                source_name:source.into(),expected_sha256:hash.into(),
            }.encode_to_vec(),
        })
    }
    fn decode(&self,out:&ProtoPayload)->Result<Value,ModelToolError>{
        let r=proto::FileTrashResult::decode(out.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Invalid trash receipt".into()))?;
        if !file_name(&r.original_name)||!id_ok(&r.trash_entry)
            ||!hash_ok(&r.sha256)||r.bytes>32*1024*1024{
            return Err(ModelToolError::InvalidResult("Invalid trash custody".into()))
        }
        Ok(json!({"original_name":r.original_name,"trash_entry":r.trash_entry,
            "bytes":r.bytes,"sha256":r.sha256,"reversible":true}))
    }
}
struct RestoreCodec;
impl ModelToolCodec for RestoreCodec{
    fn input_type_url(&self)->&'static str{RESTORE_INPUT}
    fn output_type_url(&self)->&'static str{RESTORE_OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["trusted_receipt_id"],&["trusted_receipt_id"])?;
        let id=args.get("trusted_receipt_id").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("Receipt ID required".into()))?;
        if !id_ok(id){return Err(ModelToolError::InvalidArguments("Bad receipt ID".into()))}
        Ok(ProtoPayload{
            type_url:RESTORE_INPUT.into(),
            bytes:proto::FileRestoreRequest{
                trusted_receipt_id:id.into(),
            }.encode_to_vec(),
        })
    }
    fn decode(&self,out:&ProtoPayload)->Result<Value,ModelToolError>{
        let r=proto::FileRestoreResult::decode(out.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Invalid restore receipt".into()))?;
        if !file_name(&r.restored_name)||!hash_ok(&r.sha256)||r.bytes>32*1024*1024{
            return Err(ModelToolError::InvalidResult("Invalid restored file receipt".into()))
        }
        Ok(json!({"restored_name":r.restored_name,"bytes":r.bytes,
            "sha256":r.sha256,"restored":true}))
    }
}
/// Restore is advertised only if the host supplies a previously persisted and
/// vetted actual trash receipt from its durable effect record. A model cannot
/// register or forge that receipt or choose arbitrary restored file paths.
pub fn register_trash_restore_tools(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    source:Arc<Dir>,trash:Arc<Dir>,approved_restore:Option<TrashReceipt>,
)->Result<(),String>{
    let trash_desc=ToolDescriptor{
        id:TRASH_ID.into(),display_name:"Reversibly trash approved file".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:TRASH_INPUT.into(),
        output_type_url:TRASH_OUTPUT.into(),
    };
    let mut descriptors=vec![trash_desc.clone()];
    let mut implementations:Vec<Arc<dyn NativeTool>>=vec![Arc::new(TrashTool{
        descriptor:trash_desc,source:Arc::clone(&source),trash:Arc::clone(&trash),
    })];
    if let Some(receipt)=approved_restore{
        if !id_ok(&receipt.trash_entry)||!file_name(&receipt.original)
            ||!hash_ok(&receipt.sha256){
            return Err("Trusted restore receipt was malformed".into())
        }
        let desc=ToolDescriptor{
            id:RESTORE_ID.into(),display_name:"Restore recorded trashed file".into(),
            effect:ToolEffect::WriteWorkspace,input_type_url:RESTORE_INPUT.into(),
            output_type_url:RESTORE_OUTPUT.into(),
        };
        descriptors.push(desc.clone());
        implementations.push(Arc::new(RestoreTool{
            descriptor:desc,source:Arc::clone(&source),
            trash:Arc::clone(&trash),receipt,
        }));
    }
    let restore_allowed=descriptors.iter().any(|d|d.id==RESTORE_ID);
    registry.register(ModuleManifest{
        id:"lu.files.reversible".into(),display_name:"Lu reversible file deletion".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:descriptors,panels:vec![],workflows:vec![],
    },implementations).map_err(|e|format!("Reversible file registry refused: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:TRASH_MODEL.into(),native_tool_id:TRASH_ID.into(),
        description:"Propose moving one existing approved file into Lu-owned reversible trash; no permanent deletion.".into(),
        parameters:json!({"type":"object","properties":{
            "source_name":{"type":"string"},"expected_sha256":{"type":"string"}
        },"required":["source_name","expected_sha256"],"additionalProperties":false}),
        codec:Arc::new(TrashCodec),
    }).map_err(|e|format!("Trash codec refused: {e:?}"))?;
    if restore_allowed{
        catalog.insert(registry,ModelToolDefinition{
            name:RESTORE_MODEL.into(),native_tool_id:RESTORE_ID.into(),
            description:"Propose restoring an approved host-recorded trash receipt; no overwrite.".into(),
            parameters:json!({"type":"object","properties":{
                "trusted_receipt_id":{"type":"string"}
            },"required":["trusted_receipt_id"],"additionalProperties":false}),
            codec:Arc::new(RestoreCodec),
        }).map_err(|e|format!("Restore codec refused: {e:?}"))?;
    }
    Ok(())
}
