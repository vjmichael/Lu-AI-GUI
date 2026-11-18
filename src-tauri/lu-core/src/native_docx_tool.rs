//! Wave 2 structured DOCX create tool: exact host-approved target + content.
//! Model JSON is an untrusted proposal; registration does not grant authority.
//! Execute only via ConversationAuthority, PolicyEngine and EffectJournal.
use std::sync::Arc;
use cap_std::fs::Dir;
use prost::Message;
use serde_json::{json,Map,Value};
use sha2::{Digest,Sha256};
use crate::{
    contracts::{AuthorizationError,ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,
        ToolAuthorizer,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    native_docx_create::{
        render_docx,create_docx_new,DocxBlock,DocxBlockKind,DocxCreateError,
    },
    native_atomic_write::AtomicWriteError,
    registry::NativeToolRegistry,
    wire::proto,
};
pub const TOOL_ID:&str="lu.docx.create";
pub const MODEL_NAME:&str="lu_docx_create";
pub const INPUT:&str="type.googleapis.com/lu.v1.DocxCreateRequest";
pub const OUTPUT:&str="type.googleapis.com/lu.v1.DocxCreateResult";
const MAX_MODEL_BLOCKS:usize=512;
fn valid_name(s:&str)->bool{
    !s.is_empty()&&s.len()<=200&&s!="."&&s!=".."
       && !s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
       && !s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
       && s.to_ascii_lowercase().ends_with(".docx")
}
fn valid_sha(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn blocks(r:&proto::DocxCreateRequest)->Result<Vec<DocxBlock>,DocxCreateError>{
    if r.blocks.is_empty(){return Err(DocxCreateError::Empty)}
    if r.blocks.len()>MAX_MODEL_BLOCKS{return Err(DocxCreateError::TooLarge)}
    r.blocks.iter().map(|b|{
        let kind=match b.kind.as_str(){
            "title"=>DocxBlockKind::Title,
            "heading1"=>DocxBlockKind::Heading1,
            "heading2"=>DocxBlockKind::Heading2,
            "paragraph"=>DocxBlockKind::Paragraph,
            _=>return Err(DocxCreateError::InvalidText),
        };
        Ok(DocxBlock{kind,text:b.text.clone()})
    }).collect()
}
fn digest(r:&proto::DocxCreateRequest)->Result<String,DocxCreateError>{
    let bytes=render_docx(&blocks(r)?)?;
    Ok(format!("{:x}",Sha256::digest(&bytes)))
}
pub struct ExactDocxCreatePolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub file_name:&'a str,
    pub expected_sha256:&'a str,
}
impl ToolAuthorizer for ExactDocxCreatePolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("Word creation not approved");
        if desc.id!=TOOL_ID||call.tool_id!=TOOL_ID
            ||desc.effect!=ToolEffect::WriteWorkspace||call.input.type_url!=INPUT
            ||!valid_name(self.file_name)||!valid_sha(self.expected_sha256){
            return Err(deny())
        }
        let r=proto::DocxCreateRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if r.encode_to_vec()!=call.input.bytes ||r.file_name!=self.file_name
            ||digest(&r).map_err(|_|deny())?!=self.expected_sha256{
            return Err(deny())
        }
        self.host_policy.authorize(call,desc)
    }
}
fn failure(e:DocxCreateError)->ToolFailure{
    let code=match e{
        DocxCreateError::Publish(AtomicWriteError::ExistingTarget)=>"TARGET_EXISTS",
        DocxCreateError::Publish(AtomicWriteError::PublishedNeedsReconciliation)=>"WRITE_UNCERTAIN",
        DocxCreateError::TooLarge=>"TOO_LARGE",
        _=>"DOCX_WRITE_REJECTED",
    };
    ToolFailure::new(code,"Word creation failed or requires reconciliation",false)
}
struct Creator{descriptor:ToolDescriptor,root:Arc<Dir>}
impl NativeTool for Creator{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let root=Arc::clone(&self.root);
        Box::pin(async move{
            if call.input.type_url!=INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Wrong DOCX request type",false))
            }
            let r=proto::DocxCreateRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Invalid DOCX creation request",false))?;
            if r.encode_to_vec()!=call.input.bytes ||!valid_name(&r.file_name){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Invalid DOCX target",false))
            }
            let selected=blocks(&r).map_err(failure)?;
            tokio::task::spawn_blocking(move||{
                let receipt=create_docx_new(&root,&r.file_name,&call.call_id,&selected)
                    .map_err(failure)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),bytes:proto::DocxCreateResult{
                        file_name:receipt.target,
                        bytes_written:receipt.bytes_written as u64,
                        sha256:receipt.sha256,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","Word creation worker completion unknown",false))?
        })
    }
}
struct Codec;
impl ModelToolCodec for Codec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["file_name","blocks"],&["file_name","blocks"])?;
        let file_name=args.get("file_name").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("DOCX filename required".into()))?;
        let values=args.get("blocks").and_then(Value::as_array)
            .filter(|v|!v.is_empty()&&v.len()<=MAX_MODEL_BLOCKS)
            .ok_or_else(||ModelToolError::InvalidArguments("Document blocks required".into()))?;
        if !valid_name(file_name){
            return Err(ModelToolError::InvalidArguments("Invalid DOCX filename".into()))
        }
        let mut encoded=Vec::new();
        for item in values{
            let obj=item.as_object()
                .ok_or_else(||ModelToolError::InvalidArguments("Block must be object".into()))?;
            expect_fields(obj,&["kind","text"],&["kind","text"])?;
            let kind=obj.get("kind").and_then(Value::as_str)
                .ok_or_else(||ModelToolError::InvalidArguments("Block kind required".into()))?;
            let text=obj.get("text").and_then(Value::as_str)
                .ok_or_else(||ModelToolError::InvalidArguments("Block text required".into()))?;
            encoded.push(proto::DocxCreateBlock{kind:kind.into(),text:text.into()});
        }
        let r=proto::DocxCreateRequest{file_name:file_name.into(),blocks:encoded};
        digest(&r).map_err(|_|ModelToolError::InvalidArguments("Invalid Word document".into()))?;
        Ok(ProtoPayload{type_url:INPUT.into(),bytes:r.encode_to_vec()})
    }
    fn decode(&self,out:&ProtoPayload)->Result<Value,ModelToolError>{
        let r=proto::DocxCreateResult::decode(out.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Invalid DOCX receipt".into()))?;
        if !valid_name(&r.file_name)||!valid_sha(&r.sha256)||r.bytes_written==0{
            return Err(ModelToolError::InvalidResult("Invalid DOCX receipt".into()))
        }
        Ok(json!({"file_name":r.file_name,"bytes_written":r.bytes_written,
            "sha256":r.sha256,"created":true}))
    }
}
pub fn register_docx_create(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,root:Arc<Dir>
)->Result<(),String>{
    let desc=ToolDescriptor{
        id:TOOL_ID.into(),display_name:"Create structured Word document".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.docx".into(),display_name:"Lu structured Word creation".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![desc.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(Creator{descriptor:desc,root})])
        .map_err(|e|format!("DOCX tool registration rejected: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:MODEL_NAME.into(),native_tool_id:TOOL_ID.into(),
        description:"Propose a new Word DOCX document. User must approve exact file and content hash. Never overwrites.".into(),
        parameters:json!({"type":"object","properties":{
            "file_name":{"type":"string"},
            "blocks":{"type":"array","maxItems":MAX_MODEL_BLOCKS,"items":{
                "type":"object","properties":{
                    "kind":{"type":"string","enum":["title","heading1","heading2","paragraph"]},
                    "text":{"type":"string"}
                },"required":["kind","text"],"additionalProperties":false
            }}
        },"required":["file_name","blocks"],"additionalProperties":false}),
        codec:Arc::new(Codec),
    }).map_err(|e|format!("DOCX tool schema rejected: {e:?}"))?;
    Ok(())
}
