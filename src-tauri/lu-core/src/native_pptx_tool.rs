//! Exact-grant presentation writer. A model may propose slides, but no
//! permission is minted by the model's request or this module's registration.
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
    native_pptx_create::{
        render_pptx,create_pptx_new,PresentationSlide,PptxCreateError,
    },
    native_atomic_write::AtomicWriteError,
    registry::NativeToolRegistry,wire::proto,
};
pub const TOOL_ID:&str="lu.pptx.create";
pub const MODEL_NAME:&str="lu_pptx_create";
pub const INPUT:&str="type.googleapis.com/lu.v1.PptxCreateRequest";
pub const OUTPUT:&str="type.googleapis.com/lu.v1.PptxCreateResult";
fn filename(s:&str)->bool{
    !s.is_empty()&&s.len()<=200&&s!="."&&s!=".."
        &&!s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
        &&s.to_ascii_lowercase().ends_with(".pptx")
}
fn hash_ok(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn parsed(r:&proto::PptxCreateRequest)->Vec<PresentationSlide>{
    r.slides.iter().map(|s|PresentationSlide{
        title:s.title.clone(),bullet_points:s.bullet_points.clone(),
    }).collect()
}
fn preview_digest(r:&proto::PptxCreateRequest)->Result<String,PptxCreateError>{
    let bytes=render_pptx(&parsed(r))?;
    Ok(format!("{:x}",Sha256::digest(&bytes)))
}
pub struct ExactPptxCreatePolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub approved_filename:&'a str,
    pub approved_sha256:&'a str,
}
impl ToolAuthorizer for ExactPptxCreatePolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("Presentation creation not approved");
        if desc.id!=TOOL_ID||call.tool_id!=TOOL_ID
            ||desc.effect!=ToolEffect::WriteWorkspace||call.input.type_url!=INPUT
            ||!filename(self.approved_filename)||!hash_ok(self.approved_sha256){
            return Err(deny())
        }
        let r=proto::PptxCreateRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if r.encode_to_vec()!=call.input.bytes
            ||r.file_name!=self.approved_filename
            ||preview_digest(&r).map_err(|_|deny())?!=self.approved_sha256{
            return Err(deny())
        }
        self.host_policy.authorize(call,desc)
    }
}
fn failure(e:PptxCreateError)->ToolFailure{
    let code=match e{
        PptxCreateError::Publish(AtomicWriteError::ExistingTarget)=>"TARGET_EXISTS",
        PptxCreateError::Publish(AtomicWriteError::PublishedNeedsReconciliation)=>"WRITE_UNCERTAIN",
        PptxCreateError::TooLarge=>"TOO_LARGE",
        _=>"PPTX_WRITE_REJECTED",
    };
    ToolFailure::new(code,"PowerPoint creation rejected or requires reconciliation",false)
}
struct Creator{descriptor:ToolDescriptor,root:Arc<Dir>}
impl NativeTool for Creator{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let root=Arc::clone(&self.root);
        Box::pin(async move{
            if call.input.type_url!=INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Wrong PPTX type",false))
            }
            let r=proto::PptxCreateRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Invalid presentation",false))?;
            if r.encode_to_vec()!=call.input.bytes||!filename(&r.file_name){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Invalid presentation target",false))
            }
            let slides=parsed(&r);
            render_pptx(&slides).map_err(failure)?;
            tokio::task::spawn_blocking(move||{
                let receipt=create_pptx_new(&root,&r.file_name,&call.call_id,&slides)
                    .map_err(failure)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),bytes:proto::PptxCreateResult{
                        file_name:receipt.target,
                        bytes_written:receipt.bytes_written as u64,
                        sha256:receipt.sha256,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","PowerPoint worker completion unknown",false))?
        })
    }
}
struct Codec;
impl ModelToolCodec for Codec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["file_name","slides"],&["file_name","slides"])?;
        let name=args.get("file_name").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("PPTX filename required".into()))?;
        let values=args.get("slides").and_then(Value::as_array)
            .filter(|v|!v.is_empty()&&v.len()<=64)
            .ok_or_else(||ModelToolError::InvalidArguments("Slides required".into()))?;
        if !filename(name){return Err(ModelToolError::InvalidArguments("Invalid PPTX name".into()))}
        let mut slides=Vec::with_capacity(values.len());
        for v in values{
            let obj=v.as_object()
                .ok_or_else(||ModelToolError::InvalidArguments("Slide must be an object".into()))?;
            expect_fields(obj,&["title","bullet_points"],&["title","bullet_points"])?;
            let title=obj.get("title").and_then(Value::as_str)
                .ok_or_else(||ModelToolError::InvalidArguments("Slide title required".into()))?;
            let points=obj.get("bullet_points").and_then(Value::as_array)
                .filter(|v|v.len()<=20)
                .ok_or_else(||ModelToolError::InvalidArguments("Bullet points required".into()))?;
            let bullets=points.iter().map(|p|p.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(||ModelToolError::InvalidArguments("Bullet text required".into()))?;
            slides.push(proto::PptxCreateSlide{
                title:title.into(),bullet_points:bullets,
            });
        }
        let r=proto::PptxCreateRequest{file_name:name.into(),slides};
        preview_digest(&r)
            .map_err(|_|ModelToolError::InvalidArguments("Invalid slide data".into()))?;
        Ok(ProtoPayload{type_url:INPUT.into(),bytes:r.encode_to_vec()})
    }
    fn decode(&self,out:&ProtoPayload)->Result<Value,ModelToolError>{
        let r=proto::PptxCreateResult::decode(out.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Bad presentation receipt".into()))?;
        if !filename(&r.file_name)||!hash_ok(&r.sha256)||r.bytes_written==0{
            return Err(ModelToolError::InvalidResult("Invalid presentation receipt".into()))
        }
        Ok(json!({"file_name":r.file_name,"bytes_written":r.bytes_written,
            "sha256":r.sha256,"created":true}))
    }
}
pub fn register_pptx_create(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,root:Arc<Dir>
)->Result<(),String>{
    let descriptor=ToolDescriptor{
        id:TOOL_ID.into(),display_name:"Create PowerPoint presentation".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.pptx".into(),display_name:"Lu native presentation writer".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![descriptor.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(Creator{descriptor,root})])
        .map_err(|e|format!("Presentation registration rejected: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:MODEL_NAME.into(),native_tool_id:TOOL_ID.into(),
        description:"Propose a new text-first PowerPoint deck. Requires exact human-approved filename and package digest.".into(),
        parameters:json!({"type":"object","properties":{
            "file_name":{"type":"string"},
            "slides":{"type":"array","maxItems":64,"items":{
                "type":"object","properties":{
                    "title":{"type":"string"},
                    "bullet_points":{"type":"array","maxItems":20,"items":{"type":"string"}}
                },"required":["title","bullet_points"],"additionalProperties":false
            }}
        },"required":["file_name","slides"],"additionalProperties":false}),
        codec:Arc::new(Codec),
    }).map_err(|e|format!("Presentation schema rejected: {e:?}"))?;
    Ok(())
}
