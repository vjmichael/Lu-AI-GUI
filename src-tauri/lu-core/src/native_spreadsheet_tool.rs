//! Authenticated, exact-content spreadsheet Write tool. The model may propose
//! typed cells; only the trusted host can authorize the actual target and
//! final bytes. Always dispatch via ConversationAuthority + EffectJournal.
//! No default all-files write permission or overwrite exists.
use std::sync::Arc;
use cap_std::fs::Dir;
use prost::Message;
use serde_json::{json,Map,Value};
use sha2::{Sha256,Digest};
use crate::{
    contracts::{AuthorizationError,ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,
        ToolAuthorizer,CORE_CONTRACT_VERSION},
    native_spreadsheet_create::{CellValue,SpreadsheetCell,render_csv,render_xlsx,
        SpreadsheetWriteError},
    native_atomic_write::{create_new_atomic_in_dir,AtomicWriteError},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    registry::NativeToolRegistry,
    wire::proto,
};
pub const TOOL_ID:&str="lu.spreadsheet.create";
pub const MODEL_NAME:&str="lu_spreadsheet_create";
pub const INPUT:&str="type.googleapis.com/lu.v1.SpreadsheetCreateRequest";
pub const OUTPUT:&str="type.googleapis.com/lu.v1.SpreadsheetCreateResult";
const MAX_MODEL_CELLS:usize=512;
fn valid_filename(s:&str)->bool{
    !s.is_empty() && s.len()<=200 && s!="." && s!=".."
        && !s.starts_with(".lu-stage-") && !s.ends_with('.') && !s.ends_with(' ')
        && !s.chars().any(|c|c.is_control() || matches!(c,'/'|'\\'|':'))
        && (s.to_ascii_lowercase().ends_with(".xlsx")||s.to_ascii_lowercase().ends_with(".csv"))
}
fn as_cells(input:&proto::SpreadsheetCreateRequest)->Result<Vec<SpreadsheetCell>,SpreadsheetWriteError>{
    if input.cells.len()>MAX_MODEL_CELLS{return Err(SpreadsheetWriteError::TooManyCells)}
    input.cells.iter().map(|c|{
        let value=match &c.value{
            Some(proto::spreadsheet_create_cell::Value::Text(v))=>CellValue::Text(v.clone()),
            Some(proto::spreadsheet_create_cell::Value::Number(v))=>CellValue::Number(*v),
            Some(proto::spreadsheet_create_cell::Value::Boolean(v))=>CellValue::Boolean(*v),
            Some(proto::spreadsheet_create_cell::Value::Formula(v))=>CellValue::Formula(v.clone()),
            None=>return Err(SpreadsheetWriteError::InvalidCell),
        };
        let col=u16::try_from(c.column).map_err(|_|SpreadsheetWriteError::InvalidCell)?;
        Ok(SpreadsheetCell{row:c.row,column:col,value})
    }).collect()
}
fn render(input:&proto::SpreadsheetCreateRequest)->Result<Vec<u8>,SpreadsheetWriteError>{
    let cells=as_cells(input)?;
    if input.file_name.to_ascii_lowercase().ends_with(".xlsx"){
        render_xlsx(&input.sheet_name,&cells)
    } else {render_csv(&cells)}
}
fn deny(message:&str)->AuthorizationError{AuthorizationError::new(message)}
/// This policy is constructed ONLY after an explicit target+content preview
/// grant recorded by the trusted host, never from a model-provided string.
pub struct ExactSpreadsheetCreatePolicy<'a>{
    pub host_policy:&'a dyn ToolAuthorizer,
    pub approved_file_name:&'a str,
    pub approved_sha256:&'a str,
}
impl ToolAuthorizer for ExactSpreadsheetCreatePolicy<'_>{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)->Result<(),AuthorizationError>{
        if desc.id!=TOOL_ID || call.tool_id!=TOOL_ID ||
            desc.effect!=ToolEffect::WriteWorkspace || call.input.type_url!=INPUT ||
            !valid_filename(self.approved_file_name) ||
            self.approved_sha256.len()!=64 {
            return Err(deny("Spreadsheet write capability not approved"));
        }
        let request=proto::SpreadsheetCreateRequest::decode(call.input.bytes.as_slice())
            .map_err(|_|deny("Invalid spreadsheet protobuf"))?;
        if request.encode_to_vec()!=call.input.bytes ||
            request.file_name!=self.approved_file_name || !valid_filename(&request.file_name){
            return Err(deny("Spreadsheet target differs from approval"));
        }
        let bytes=render(&request).map_err(|_|deny("Invalid spreadsheet cell data"))?;
        let digest=format!("{:x}",Sha256::digest(&bytes));
        if digest!=self.approved_sha256 {
            return Err(deny("Spreadsheet bytes differ from approved preview"));
        }
        self.host_policy.authorize(call,desc)
    }
}
fn failure(error:SpreadsheetWriteError)->ToolFailure{
    let code=match error{
        SpreadsheetWriteError::Atomic(AtomicWriteError::ExistingTarget)=>"TARGET_EXISTS",
        SpreadsheetWriteError::Atomic(AtomicWriteError::PublishedNeedsReconciliation)=>"WRITE_UNCERTAIN",
        SpreadsheetWriteError::TooLarge|SpreadsheetWriteError::TooManyCells=>"TOO_LARGE",
        _=>"SPREADSHEET_INVALID",
    };
    ToolFailure::new(code,"Spreadsheet creation rejected or requires reconciliation",false)
}
struct SpreadsheetCreateTool{ descriptor:ToolDescriptor,root:Arc<Dir> }
impl NativeTool for SpreadsheetCreateTool {
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let root=Arc::clone(&self.root);
        Box::pin(async move{
            if call.input.type_url!=INPUT{
                return Err(ToolFailure::new("BAD_PROTOBUF","Invalid spreadsheet request type",false))
            }
            let request=proto::SpreadsheetCreateRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|ToolFailure::new("BAD_PROTOBUF","Invalid spreadsheet request",false))?;
            if request.encode_to_vec()!=call.input.bytes || !valid_filename(&request.file_name){
                return Err(ToolFailure::new("BAD_ARGUMENTS","Invalid spreadsheet target",false))
            }
            let bytes=render(&request).map_err(failure)?;
            tokio::task::spawn_blocking(move||{
                let receipt=create_new_atomic_in_dir(
                    &root,&request.file_name,&call.call_id,&bytes
                ).map_err(|e|failure(SpreadsheetWriteError::Atomic(e)))?;
                let output=proto::SpreadsheetCreateResult{
                    file_name:receipt.target,
                    bytes_written:receipt.bytes_written as u64,
                    sha256:receipt.sha256,
                };
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),bytes:output.encode_to_vec()
                }})
            }).await.map_err(|_|ToolFailure::new(
                "WRITE_UNCERTAIN","Spreadsheet worker outcome unknown",false))?
        })
    }
}
struct Codec;
impl ModelToolCodec for Codec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["file_name","sheet_name","cells"],&["file_name","sheet_name","cells"])?;
        let file_name=args.get("file_name").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("file_name must be text".into()))?;
        let sheet_name=args.get("sheet_name").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("sheet_name must be text".into()))?;
        if !valid_filename(file_name){
            return Err(ModelToolError::InvalidArguments("Invalid spreadsheet name".into()))
        }
        let cells=args.get("cells").and_then(Value::as_array)
            .filter(|v|v.len()<=MAX_MODEL_CELLS)
            .ok_or_else(||ModelToolError::InvalidArguments("Invalid cells array".into()))?;
        let mut encoded=Vec::with_capacity(cells.len());
        for item in cells{
            let object=item.as_object()
                .ok_or_else(||ModelToolError::InvalidArguments("Cell must be an object".into()))?;
            expect_fields(object,&["row","column","type","value"],&["row","column","type","value"])?;
            let row=object.get("row").and_then(Value::as_u64)
                .and_then(|n|u32::try_from(n).ok())
                .ok_or_else(||ModelToolError::InvalidArguments("Invalid row".into()))?;
            let column=object.get("column").and_then(Value::as_u64)
                .and_then(|n|u32::try_from(n).ok())
                .ok_or_else(||ModelToolError::InvalidArguments("Invalid column".into()))?;
            let value=match object.get("type").and_then(Value::as_str){
                Some("text")=>proto::spreadsheet_create_cell::Value::Text(
                    object.get("value").and_then(Value::as_str)
                        .ok_or_else(||ModelToolError::InvalidArguments("Text must be a string".into()))?.into()),
                Some("number")=>proto::spreadsheet_create_cell::Value::Number(
                    object.get("value").and_then(Value::as_f64)
                        .filter(|n|n.is_finite())
                        .ok_or_else(||ModelToolError::InvalidArguments("Number must be finite".into()))?),
                Some("boolean")=>proto::spreadsheet_create_cell::Value::Boolean(
                    object.get("value").and_then(Value::as_bool)
                        .ok_or_else(||ModelToolError::InvalidArguments("Boolean required".into()))?),
                Some("formula")=>proto::spreadsheet_create_cell::Value::Formula(
                    object.get("value").and_then(Value::as_str)
                        .ok_or_else(||ModelToolError::InvalidArguments("Formula must be a string".into()))?.into()),
                _=>return Err(ModelToolError::InvalidArguments("Unknown cell type".into())),
            };
            encoded.push(proto::SpreadsheetCreateCell{row,column,value:Some(value)});
        }
        let request=proto::SpreadsheetCreateRequest{
            file_name:file_name.into(),sheet_name:sheet_name.into(),cells:encoded,
        };
        // Validate before publishing even an inert candidate.
        render(&request).map_err(|_|ModelToolError::InvalidArguments("Invalid spreadsheet data".into()))?;
        Ok(ProtoPayload{type_url:INPUT.into(),bytes:request.encode_to_vec()})
    }
    fn decode(&self,output:&ProtoPayload)->Result<Value,ModelToolError>{
        let result=proto::SpreadsheetCreateResult::decode(output.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Malformed spreadsheet receipt".into()))?;
        if !valid_filename(&result.file_name) || result.sha256.len()!=64 ||
            result.bytes_written==0 {
            return Err(ModelToolError::InvalidResult("Invalid spreadsheet receipt".into()))
        }
        Ok(json!({"file_name":result.file_name,"bytes_written":result.bytes_written,
            "sha256":result.sha256,"created":true}))
    }
}
/// Register on a host-pinned directory only; no automatic model access.
/// Dispatch MUST use ExactSpreadsheetCreatePolicy, durable effect journal,
/// and persisted human-turn authority.
pub fn register_spreadsheet_create(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,root:Arc<Dir>
)->Result<(),String>{
    let descriptor=ToolDescriptor{
        id:TOOL_ID.into(),display_name:"Create a new XLSX or CSV spreadsheet".into(),
        effect:ToolEffect::WriteWorkspace,input_type_url:INPUT.into(),
        output_type_url:OUTPUT.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.spreadsheet".into(),display_name:"Lu structured spreadsheets".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![descriptor.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(SpreadsheetCreateTool{descriptor,root})])
        .map_err(|e|format!("Spreadsheet registry refused: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:MODEL_NAME.into(),native_tool_id:TOOL_ID.into(),
        description:"Propose a new spreadsheet. Write requires the user's exact approval; never overwrites.".into(),
        parameters:json!({"type":"object","properties":{
            "file_name":{"type":"string","description":"New .xlsx or .csv name"},
            "sheet_name":{"type":"string","description":"Worksheet title for XLSX"},
            "cells":{"type":"array","maxItems":MAX_MODEL_CELLS,"items":{
                "type":"object","properties":{
                    "row":{"type":"integer","minimum":1,"maximum":10000},
                    "column":{"type":"integer","minimum":1,"maximum":256},
                    "type":{"type":"string","enum":["text","number","boolean","formula"]},
                    "value":{}
                },"required":["row","column","type","value"],"additionalProperties":false
            }}
        },"required":["file_name","sheet_name","cells"],"additionalProperties":false}),
        codec:Arc::new(Codec),
    }).map_err(|e|format!("Spreadsheet schema refused: {e:?}"))?;
    Ok(())
}
