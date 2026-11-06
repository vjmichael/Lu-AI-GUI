//! Exact-approved, NON-DESTRUCTIVE existing Office document revision tool.
//! The user grants a new versioned output after inspecting the entire rendered
//! package/diff. The original DOCX/PPTX/XLSX remains untouched. No model
//! request is allowed to mint grant or change original workbook metadata.
use std::{io::Read,sync::Arc};
use cap_std::fs::Dir;
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{AuthorizationError,ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,
        ToolAuthorizer,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    native_office_revision::{
        OfficeRevisionEdit,OfficeRevisionError,render_office_revision,
        publish_office_revision_new,
    },
    native_atomic_write::AtomicWriteError,
    registry::NativeToolRegistry,wire::proto,
};
pub const TOOL_ID:&str="lu.office.revisions.create";
pub const MODEL_NAME:&str="lu_office_create_revision";
pub const INPUT:&str="type.googleapis.com/lu.v1.OfficeRevisionRequest";
pub const OUTPUT:&str="type.googleapis.com/lu.v1.OfficeRevisionResult";
fn file_name(s:&str)->bool{
    !s.is_empty()&&s.len()<=200&&s!="."&&s!=".."
       &&!s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
       &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn same_type(source:&str,target:&str)->bool{
    file_name(source)&&file_name(target)&&source!=target
        &&[".docx",".pptx",".xlsx"].iter().any(|ext|
            source.to_ascii_lowercase().ends_with(ext) &&
            target.to_ascii_lowercase().ends_with(ext))
}
fn sha_ok(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn patch(r:&proto::OfficeRevisionRequest)->Result<OfficeRevisionEdit,OfficeRevisionError>{
    match &r.change{
        Some(proto::office_revision_request::Change::WordText(p))=>{
            Ok(OfficeRevisionEdit::WordRun{
                text_index:p.text_index as usize,
                expected_old:p.expected_old.clone(),new_text:p.new_text.clone(),
            })
        },
        Some(proto::office_revision_request::Change::SlideText(p))=>{
            Ok(OfficeRevisionEdit::SlideRun{
                slide_number:p.slide_number as usize,text_index:p.text_index as usize,
                expected_old:p.expected_old.clone(),new_text:p.new_text.clone(),
            })
        },
        Some(proto::office_revision_request::Change::CellNumber(p))=>{
            if !p.expected_old.is_finite()||!p.new_number.is_finite(){
                return Err(OfficeRevisionError::InvalidContent)
            }
            Ok(OfficeRevisionEdit::CellNumber{
                sheet_number:p.sheet_number as usize,address:p.address.clone(),
                expected_old:p.expected_old,new_number:p.new_number,
            })
        },
        Some(proto::office_revision_request::Change::CellBoolean(p))=>{
            Ok(OfficeRevisionEdit::CellBoolean{
                sheet_number:p.sheet_number as usize,address:p.address.clone(),
                expected_old:p.expected_old,new_boolean:p.new_boolean,
            })
        },
        Some(proto::office_revision_request::Change::CellText(p))=>{
            Ok(OfficeRevisionEdit::CellInlineText{
                sheet_number:p.sheet_number as usize,address:p.address.clone(),
                expected_old:p.expected_old.clone(),new_text:p.new_text.clone(),
            })
        },
        None=>Err(OfficeRevisionError::InvalidTarget),
    }
}
fn valid_kind(r:&proto::OfficeRevisionRequest)->bool{
    let extension=if r.source_name.to_ascii_lowercase().ends_with(".docx"){"docx"}
        else if r.source_name.to_ascii_lowercase().ends_with(".pptx"){"pptx"}
        else if r.source_name.to_ascii_lowercase().ends_with(".xlsx"){"xlsx"}
        else{return false};
    matches!((&r.change,extension),
        (Some(proto::office_revision_request::Change::WordText(_)),"docx")|
        (Some(proto::office_revision_request::Change::SlideText(_)),"pptx")|
        (Some(proto::office_revision_request::Change::CellNumber(_)),"xlsx")|
        (Some(proto::office_revision_request::Change::CellBoolean(_)),"xlsx")|
        (Some(proto::office_revision_request::Change::CellText(_)),"xlsx"))
}
fn preview(
    root:&Dir,r:&proto::OfficeRevisionRequest,
)->Result<String,OfficeRevisionError>{
    if !same_type(&r.source_name,&r.target_new_name)||!valid_kind(r)
        ||!sha_ok(&r.original_sha256)||!sha_ok(&r.approved_revised_sha256){
        return Err(OfficeRevisionError::InvalidName)
    }
    let metadata=root.symlink_metadata(&r.source_name)
        .map_err(|_|OfficeRevisionError::NotFound)?;
    if !metadata.is_file()||metadata.file_type().is_symlink()
        ||metadata.len()>1024*1024{return Err(OfficeRevisionError::UnsafePackage)}
    #[cfg(windows)]{
        use cap_std::fs::MetadataExt;
        if metadata.file_attributes()&0x400!=0{
            return Err(OfficeRevisionError::UnsafePackage)
        }
    }
    let mut handle=root.open(&r.source_name)
        .map_err(|_|OfficeRevisionError::NotFound)?;
    let mut data=Vec::new();
    (&mut handle).take(1024*1024+1).read_to_end(&mut data)
        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
    let revised=render_office_revision(&data,&r.original_sha256,&patch(r)?)?;
    Ok(revised.revised_sha256)
}
pub struct ExactOfficeRevisionPolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub trusted_source:&'a Dir,
    pub approved_request:&'a proto::OfficeRevisionRequest,
}
impl ToolAuthorizer for ExactOfficeRevisionPolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        let deny=||AuthorizationError::new("Document revision not approved");
        if desc.id!=TOOL_ID||call.tool_id!=TOOL_ID
            ||desc.effect!=ToolEffect::WriteWorkspace||call.input.type_url!=INPUT{
            return Err(deny())
        }
        let r=proto::OfficeRevisionRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny())?;
        if r.encode_to_vec()!=call.input.bytes ||
            r.encode_to_vec()!=self.approved_request.encode_to_vec() ||
            preview(self.trusted_source,&r).map_err(|_|deny())?
                !=self.approved_request.approved_revised_sha256 {
            return Err(deny())
        }
        self.host_policy.authorize(call,desc)
    }
}
fn failure(error:OfficeRevisionError)->ToolFailure{
    let code=match error{
        OfficeRevisionError::Publish(AtomicWriteError::ExistingTarget)=>"TARGET_EXISTS",
        OfficeRevisionError::Publish(AtomicWriteError::PublishedNeedsReconciliation)
            =>"WRITE_UNCERTAIN",
        OfficeRevisionError::SourceChanged=>"SOURCE_CHANGED",
        OfficeRevisionError::TooLarge=>"TOO_LARGE",
        OfficeRevisionError::UnsupportedPackage=>"OFFICE_STRUCTURE_UNSUPPORTED",
        _=>"OFFICE_REVISION_REJECTED",
    };
    ToolFailure::new(code,"Office revision rejected or needs recovery review",false)
}
struct Tool{descriptor:ToolDescriptor,source:Arc<Dir>,target:Arc<Dir>}
impl NativeTool for Tool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let source=Arc::clone(&self.source);
        let target=Arc::clone(&self.target);
        Box::pin(async move{
            if call.input.type_url!=INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Wrong document revision type",false))
            }
            let r=proto::OfficeRevisionRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Invalid document revision",false))?;
            if r.encode_to_vec()!=call.input.bytes ||!valid_kind(&r)
                ||!same_type(&r.source_name,&r.target_new_name){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Invalid revision source or target",false))
            }
            let edit=patch(&r).map_err(failure)?;
            tokio::task::spawn_blocking(move||{
                let receipt=publish_office_revision_new(
                    &source,&r.source_name,&target,&r.target_new_name,
                    &call.call_id,&r.original_sha256,&r.approved_revised_sha256,&edit
                ).map_err(failure)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),
                    bytes:proto::OfficeRevisionResult{
                        target_new_name:receipt.target,
                        bytes_written:receipt.bytes_written as u64,
                        revised_sha256:receipt.sha256,
                    }.encode_to_vec(),
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","Office revision worker completion unknown",false))?
        })
    }
}
struct Codec;
impl ModelToolCodec for Codec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let required=&["source_name","target_new_name","original_sha256",
            "approved_revised_sha256","kind"];
        let get=|field:&str|args.get(field).and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments(format!("{field} must be text")));
        let kind=get("kind")?;
        let allowed:Vec<&str>=match kind{
            "word_text"=>required.iter().copied().chain([
                "text_index","expected_old","new_text"
            ]).collect(),
            "slide_text"=>required.iter().copied().chain([
                "slide_number","text_index","expected_old","new_text"
            ]).collect(),
            "cell_number"=>required.iter().copied().chain([
                "sheet_number","address","expected_old_number","new_number"
            ]).collect(),
            "cell_boolean"=>required.iter().copied().chain([
                "sheet_number","address","expected_old_boolean","new_boolean"
            ]).collect(),
            "cell_text"=>required.iter().copied().chain([
                "sheet_number","address","expected_old","new_text"
            ]).collect(),
            _=>return Err(ModelToolError::InvalidArguments("Invalid Office edit kind".into())),
        };
        expect_fields(args,&allowed,&allowed)?;
        let uint=|field:&str|args.get(field).and_then(Value::as_u64)
            .and_then(|v|u32::try_from(v).ok())
            .ok_or_else(||ModelToolError::InvalidArguments(format!("{field} must be integer")));
        let number=|field:&str|args.get(field).and_then(Value::as_f64)
            .filter(|v|v.is_finite())
            .ok_or_else(||ModelToolError::InvalidArguments(format!("{field} must be number")));
        let boolean=|field:&str|args.get(field).and_then(Value::as_bool)
            .ok_or_else(||ModelToolError::InvalidArguments(format!("{field} must be boolean")));
        let change=match kind{
            "word_text"=>proto::office_revision_request::Change::WordText(
                proto::OfficeTextPatch{
                    text_index:uint("text_index")?,slide_number:0,
                    expected_old:get("expected_old")?.into(),
                    new_text:get("new_text")?.into(),
                }),
            "slide_text"=>proto::office_revision_request::Change::SlideText(
                proto::OfficeTextPatch{
                    text_index:uint("text_index")?,slide_number:uint("slide_number")?,
                    expected_old:get("expected_old")?.into(),
                    new_text:get("new_text")?.into(),
                }),
            "cell_number"=>proto::office_revision_request::Change::CellNumber(
                proto::OfficeNumberPatch{
                    sheet_number:uint("sheet_number")?,
                    address:get("address")?.into(),
                    expected_old:number("expected_old_number")?,
                    new_number:number("new_number")?,
                }),
            "cell_boolean"=>proto::office_revision_request::Change::CellBoolean(
                proto::OfficeBooleanPatch{
                    sheet_number:uint("sheet_number")?,
                    address:get("address")?.into(),
                    expected_old:boolean("expected_old_boolean")?,
                    new_boolean:boolean("new_boolean")?,
                }),
            "cell_text"=>proto::office_revision_request::Change::CellText(
                proto::OfficeStringPatch{
                    sheet_number:uint("sheet_number")?,
                    address:get("address")?.into(),
                    expected_old:get("expected_old")?.into(),
                    new_text:get("new_text")?.into(),
                }),
            _=>return Err(ModelToolError::InvalidArguments("Unsupported Office edit".into())),
        };
        let r=proto::OfficeRevisionRequest{
            source_name:get("source_name")?.into(),
            target_new_name:get("target_new_name")?.into(),
            original_sha256:get("original_sha256")?.into(),
            approved_revised_sha256:get("approved_revised_sha256")?.into(),
            change:Some(change),
        };
        if !same_type(&r.source_name,&r.target_new_name)||!valid_kind(&r)
            ||!sha_ok(&r.original_sha256)||!sha_ok(&r.approved_revised_sha256)
            ||patch(&r).is_err(){
            return Err(ModelToolError::InvalidArguments("Unsafe Office revision".into()))
        }
        Ok(ProtoPayload{type_url:INPUT.into(),bytes:r.encode_to_vec()})
    }
    fn decode(&self,output:&ProtoPayload)->Result<Value,ModelToolError>{
        let r=proto::OfficeRevisionResult::decode(output.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Malformed Office revision receipt".into()))?;
        if !file_name(&r.target_new_name)||!sha_ok(&r.revised_sha256)
            ||r.bytes_written==0||r.bytes_written>1024*1024{
            return Err(ModelToolError::InvalidResult("Invalid Office output receipt".into()))
        }
        Ok(json!({"target_new_name":r.target_new_name,
            "bytes_written":r.bytes_written,"revised_sha256":r.revised_sha256,
            "created_revision":true,"original_preserved":true}))
    }
}
pub fn register_office_revision_tool(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    source:Arc<Dir>,target:Arc<Dir>,
)->Result<(),String>{
    let desc=ToolDescriptor{
        id:TOOL_ID.into(),display_name:"Create approved Office document revision".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.office.revisions".into(),
        display_name:"Lu non-destructive Office revisions".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![desc.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(Tool{descriptor:desc,source,target})])
        .map_err(|e|format!("Office revision registry refused: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:MODEL_NAME.into(),native_tool_id:TOOL_ID.into(),
        description:"Propose revising one existing Word text run, PowerPoint run or simple XLSX numeric/boolean cell into a NEW versioned file. User must review full content and approve exact output hash.".into(),
        parameters:json!({"type":"object","properties":{
            "source_name":{"type":"string"},"target_new_name":{"type":"string"},
            "original_sha256":{"type":"string"},
            "approved_revised_sha256":{"type":"string"},
            "kind":{"type":"string","enum":["word_text","slide_text","cell_number","cell_boolean","cell_text"]},
            "text_index":{"type":"integer","minimum":0,"maximum":10000},
            "slide_number":{"type":"integer","minimum":1,"maximum":256},
            "sheet_number":{"type":"integer","minimum":1,"maximum":256},
            "address":{"type":"string"},"expected_old":{"type":"string"},
            "new_text":{"type":"string"},"expected_old_number":{"type":"number"},
            "new_number":{"type":"number"},"expected_old_boolean":{"type":"boolean"},
            "new_boolean":{"type":"boolean"}
        },"required":["source_name","target_new_name","original_sha256",
            "approved_revised_sha256","kind"],"additionalProperties":false}),
        codec:Arc::new(Codec),
    }).map_err(|e|format!("Office revision model codec refused: {e:?}"))?;
    Ok(())
}
