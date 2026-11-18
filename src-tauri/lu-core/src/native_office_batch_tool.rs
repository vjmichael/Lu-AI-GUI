//! Host-approved bounded OOXML batch revision native tool. An AI may draft
//! edits but NEVER grant itself an exact revised package, user turn or path.
//! The host re-renders the entire batch before authorization and publication.
use std::{io::Read,sync::Arc};
use cap_std::fs::Dir;
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{AuthorizationError,ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,
        ToolOutput,ToolAuthorizer,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    native_office_revision::{OfficeRevisionEdit,render_office_revision_batch,
        publish_office_revision_batch_new},
    registry::NativeToolRegistry,wire::proto,
};
pub const TOOL_ID:&str="lu.office.batch.revision.create";
pub const MODEL_NAME:&str="lu_office_create_batch_revision";
pub const INPUT:&str="type.googleapis.com/lu.v1.OfficeBatchRevisionRequest";
pub const OUTPUT:&str="type.googleapis.com/lu.v1.OfficeRevisionResult";
fn sha(s:&str)->bool{s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))}
fn name(s:&str)->bool{!s.is_empty()&&s.len()<=200&&s!="."&&s!=".."
    &&!s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
    &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))}
fn kinds_match(source:&str,target:&str)->bool{
    name(source)&&name(target)&&source!=target
    &&[".docx",".pptx",".xlsx"].iter().any(|suffix|
        source.to_ascii_lowercase().ends_with(suffix)&&
        target.to_ascii_lowercase().ends_with(suffix))
}
pub fn edits(req:&proto::OfficeBatchRevisionRequest)
    ->Result<Vec<OfficeRevisionEdit>,String>{
    if req.patches.is_empty()||req.patches.len()>32||
        !kinds_match(&req.source_name,&req.target_new_name)||
        !sha(&req.original_sha256)||!sha(&req.approved_revised_sha256){
        return Err("Invalid batch approval or count".into())
    }
    let ext=if req.source_name.to_lowercase().ends_with(".docx"){"word"}
        else if req.source_name.to_lowercase().ends_with(".pptx"){"slide"}
        else{"cell"};
    let mut out=Vec::new();
    for patch in &req.patches{
        let edit=match &patch.change{
            Some(proto::office_batch_patch::Change::WordText(p)) if ext=="word"=>
                OfficeRevisionEdit::WordRun{
                    text_index:p.text_index as usize,
                    expected_old:p.expected_old.clone(),new_text:p.new_text.clone(),
                },
            Some(proto::office_batch_patch::Change::SlideText(p)) if ext=="slide"=>
                OfficeRevisionEdit::SlideRun{
                    slide_number:p.slide_number as usize,
                    text_index:p.text_index as usize,
                    expected_old:p.expected_old.clone(),new_text:p.new_text.clone(),
                },
            Some(proto::office_batch_patch::Change::CellNumber(p)) if ext=="cell"
                &&p.expected_old.is_finite()&&p.new_number.is_finite()=>
                OfficeRevisionEdit::CellNumber{
                    sheet_number:p.sheet_number as usize,address:p.address.clone(),
                    expected_old:p.expected_old,new_number:p.new_number,
                },
            Some(proto::office_batch_patch::Change::CellBoolean(p)) if ext=="cell"=>
                OfficeRevisionEdit::CellBoolean{
                    sheet_number:p.sheet_number as usize,address:p.address.clone(),
                    expected_old:p.expected_old,new_boolean:p.new_boolean,
                },
            Some(proto::office_batch_patch::Change::CellText(p)) if ext=="cell"=>
                OfficeRevisionEdit::CellInlineText{
                    sheet_number:p.sheet_number as usize,address:p.address.clone(),
                    expected_old:p.expected_old.clone(),new_text:p.new_text.clone(),
                },
            _=>return Err("Unsupported or mismatched Office batch patch".into()),
        };
        out.push(edit);
    }
    Ok(out)
}
pub struct ExactBatchPolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub trusted_source:&'a Dir,
    pub approved:&'a proto::OfficeBatchRevisionRequest,
}
impl ToolAuthorizer for ExactBatchPolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        let denied=||AuthorizationError::new("Office batch revision was not approved");
        if call.tool_id!=TOOL_ID||desc.id!=TOOL_ID
            ||desc.effect!=ToolEffect::WriteWorkspace||call.input.type_url!=INPUT{
            return Err(denied())
        }
        let request=proto::OfficeBatchRevisionRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|denied())?;
        if request.encode_to_vec()!=call.input.bytes ||
            request.encode_to_vec()!=self.approved.encode_to_vec(){
            return Err(denied())
        }
        let edits=edits(&request).map_err(|_|denied())?;
        let metadata=self.trusted_source.symlink_metadata(&request.source_name)
            .map_err(|_|denied())?;
        if !metadata.is_file()||metadata.file_type().is_symlink()
            ||metadata.len()>1024*1024{return Err(denied())}
        #[cfg(windows)]{
            use cap_std::fs::MetadataExt;
            if metadata.file_attributes()&0x400!=0{return Err(denied())}
        }
        let file=self.trusted_source.open(&request.source_name).map_err(|_|denied())?;
        let mut bytes=Vec::new();
        file.take(1024*1024+1).read_to_end(&mut bytes).map_err(|_|denied())?;
        let preview=render_office_revision_batch(
            &bytes,&request.original_sha256,&edits,
        ).map_err(|_|denied())?;
        if preview.revised_sha256!=request.approved_revised_sha256{
            return Err(denied())
        }
        self.host_policy.authorize(call,desc)
    }
}
fn failure()->ToolFailure{
    ToolFailure::new("OFFICE_BATCH_REJECTED",
        "Approved Office revision failed or requires reconciliation",false)
}
struct BatchTool{descriptor:ToolDescriptor,source:Arc<Dir>,target:Arc<Dir>}
impl NativeTool for BatchTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let source=Arc::clone(&self.source);
        let target=Arc::clone(&self.target);
        Box::pin(async move{
            if call.input.type_url!=INPUT{return Err(failure())}
            let request=proto::OfficeBatchRevisionRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|failure())?;
            if request.encode_to_vec()!=call.input.bytes{return Err(failure())}
            let patches=edits(&request).map_err(|_|failure())?;
            tokio::task::spawn_blocking(move||{
                let receipt=publish_office_revision_batch_new(
                    &source,&request.source_name,&target,&request.target_new_name,
                    &call.call_id,&request.original_sha256,
                    &request.approved_revised_sha256,&patches,
                ).map_err(|_|failure())?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),bytes:proto::OfficeRevisionResult{
                        target_new_name:receipt.target,
                        bytes_written:receipt.bytes_written as u64,
                        revised_sha256:receipt.sha256,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|failure())?
        })
    }
}
struct BatchCodec;
impl ModelToolCodec for BatchCodec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&[
            "source_name","target_new_name","original_sha256",
            "approved_revised_sha256","patches",
        ],&[
            "source_name","target_new_name","original_sha256",
            "approved_revised_sha256","patches",
        ])?;
        let str_field=|name:&str|args.get(name).and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments(format!("{name} must be string")));
        let items=args.get("patches").and_then(Value::as_array)
            .ok_or_else(||ModelToolError::InvalidArguments("patches must be array".into()))?;
        if items.is_empty()||items.len()>32{
            return Err(ModelToolError::InvalidArguments("1–32 patches required".into()))
        }
        let mut patches=Vec::new();
        for value in items{
            let obj=value.as_object().ok_or_else(||
                ModelToolError::InvalidArguments("patch must be object".into()))?;
            let required=&["kind","index","sheet_or_slide","address","expected_old","new_value"];
            expect_fields(obj,required,required)?;
            let get=|key:&str|obj.get(key).and_then(Value::as_str)
                .ok_or_else(||ModelToolError::InvalidArguments(format!("{key} must be text")));
            let index=obj.get("index").and_then(Value::as_u64).and_then(|n|u32::try_from(n).ok())
                .ok_or_else(||ModelToolError::InvalidArguments("index must be u32".into()))?;
            let where_at=obj.get("sheet_or_slide").and_then(Value::as_u64)
                .and_then(|n|u32::try_from(n).ok())
                .ok_or_else(||ModelToolError::InvalidArguments("sheet_or_slide must be u32".into()))?;
            let old=get("expected_old")?;
            let new=get("new_value")?;
            let change=match get("kind")?{
                "word_text"=>proto::office_batch_patch::Change::WordText(
                    proto::OfficeTextPatch{
                        text_index:index,slide_number:0,
                        expected_old:old.into(),new_text:new.into(),
                    }),
                "slide_text"=>proto::office_batch_patch::Change::SlideText(
                    proto::OfficeTextPatch{
                        text_index:index,slide_number:where_at,
                        expected_old:old.into(),new_text:new.into(),
                    }),
                "cell_number"=>proto::office_batch_patch::Change::CellNumber(
                    proto::OfficeNumberPatch{
                        sheet_number:where_at,address:get("address")?.into(),
                        expected_old:old.parse().map_err(|_|
                            ModelToolError::InvalidArguments("old number invalid".into()))?,
                        new_number:new.parse().map_err(|_|
                            ModelToolError::InvalidArguments("new number invalid".into()))?,
                    }),
                "cell_boolean"=>proto::office_batch_patch::Change::CellBoolean(
                    proto::OfficeBooleanPatch{
                        sheet_number:where_at,address:get("address")?.into(),
                        expected_old:old.parse().map_err(|_|
                            ModelToolError::InvalidArguments("old boolean invalid".into()))?,
                        new_boolean:new.parse().map_err(|_|
                            ModelToolError::InvalidArguments("new boolean invalid".into()))?,
                    }),
                "cell_text"=>proto::office_batch_patch::Change::CellText(
                    proto::OfficeStringPatch{
                        sheet_number:where_at,address:get("address")?.into(),
                        expected_old:old.into(),new_text:new.into(),
                    }),
                _=>return Err(ModelToolError::InvalidArguments("unknown patch kind".into())),
            };
            patches.push(proto::OfficeBatchPatch{change:Some(change)});
        }
        let request=proto::OfficeBatchRevisionRequest{
            source_name:str_field("source_name")?.into(),
            target_new_name:str_field("target_new_name")?.into(),
            original_sha256:str_field("original_sha256")?.into(),
            approved_revised_sha256:str_field("approved_revised_sha256")?.into(),
            patches,
        };
        edits(&request).map_err(ModelToolError::InvalidArguments)?;
        Ok(ProtoPayload{type_url:INPUT.into(),bytes:request.encode_to_vec()})
    }
    fn decode(&self,output:&ProtoPayload)->Result<Value,ModelToolError>{
        if output.type_url!=OUTPUT{
            return Err(ModelToolError::InvalidResult("Wrong batch receipt type".into()))
        }
        let r=proto::OfficeRevisionResult::decode(output.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Invalid Office batch receipt".into()))?;
        if !name(&r.target_new_name)||r.bytes_written==0
            ||r.bytes_written>1024*1024||!sha(&r.revised_sha256){
            return Err(ModelToolError::InvalidResult("Unsafe batch receipt".into()))
        }
        Ok(json!({"created_new_revision":true,"original_preserved":true,
            "target_new_name":r.target_new_name,
            "bytes_written":r.bytes_written,"revised_sha256":r.revised_sha256}))
    }
}
pub fn register_office_batch_tool(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    source:Arc<Dir>,target:Arc<Dir>,
)->Result<(),String>{
    let desc=ToolDescriptor{id:TOOL_ID.into(),
        display_name:"Create exact-approved multi-edit Office revision".into(),
        effect:ToolEffect::WriteWorkspace,
        input_type_url:INPUT.into(),output_type_url:OUTPUT.into()};
    registry.register(ModuleManifest{
        id:"lu.office.batch".into(),
        display_name:"Lu Office batch revisions".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![desc.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(BatchTool{descriptor:desc,source,target})])
        .map_err(|_|"Batch registry refused tool")?;
    catalog.insert(registry,ModelToolDefinition{
        name:MODEL_NAME.into(),native_tool_id:TOOL_ID.into(),
        description:"Create a NEW, non-overwriting DOCX/PPTX/XLSX version with up to 32 exact approved Word, slide or numeric/boolean cell edits. Requires full final package SHA-256 host approval.".into(),
        parameters:json!({"type":"object","properties":{
            "source_name":{"type":"string"},"target_new_name":{"type":"string"},
            "original_sha256":{"type":"string"},"approved_revised_sha256":{"type":"string"},
            "patches":{"type":"array","minItems":1,"maxItems":32,"items":{
                "type":"object","properties":{
                    "kind":{"type":"string","enum":["word_text","slide_text","cell_number","cell_boolean","cell_text"]},
                    "index":{"type":"integer","minimum":0},
                    "sheet_or_slide":{"type":"integer","minimum":0},
                    "address":{"type":"string"},
                    "expected_old":{"type":"string"},"new_value":{"type":"string"}
                },"required":["kind","index","sheet_or_slide","address","expected_old","new_value"],
                "additionalProperties":false}}
        },"required":["source_name","target_new_name","original_sha256",
            "approved_revised_sha256","patches"],"additionalProperties":false}),
        codec:Arc::new(BatchCodec),
    }).map_err(|_|"Batch model codec refused registration")?;
    Ok(())
}
