//! Foundational F: bounded independent artifact inspection for one human-approved
//! workspace. A model's "created successfully" sentence is never evidence.
//! No shell, application launch, write, credential, network or user-selected
//! absolute path. This verifies byte/container structure, NOT Office layout,
//! formulas, academic citations, PDF rendering or document semantics.
use std::{collections::BTreeSet,io::{Cursor,Read},path::Path,sync::Arc};
use cap_std::{ambient_authority,fs::Dir};
use prost::Message;
use serde_json::{json,Map,Value};
use sha2::{Digest,Sha256};
use zip::ZipArchive;
use crate::{
    contracts::{ModuleKind,ModuleManifest,NativeTool,ProtoPayload,ToolDescriptor,
        ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    registry::NativeToolRegistry,
};

pub const TOOL_ID:&str="lu.artifact.inspect";
const INPUT:&str="type.googleapis.com/lu.v1.ArtifactInspectRequest";
const OUTPUT:&str="type.googleapis.com/lu.v1.ArtifactInspectResult";
pub const MAX_ARTIFACT_BYTES:usize=8*1024*1024;
const MAX_UNCOMPRESSED:u64=32*1024*1024;
const MAX_MEMBERS:usize=512;

#[derive(Clone,PartialEq,Message)]
pub struct ArtifactInspectRequest {
    #[prost(string,tag="1")]
    pub path:String,
    #[prost(string,tag="2")]
    pub format:String,
    #[prost(string,tag="3")]
    pub expected_sha256:String,
}
#[derive(Clone,PartialEq,Message)]
pub struct ArtifactInspectResult {
    #[prost(string,tag="1")]
    pub format:String,
    #[prost(uint64,tag="2")]
    pub bytes:u64,
    #[prost(string,tag="3")]
    pub sha256:String,
    #[prost(uint32,tag="4")]
    pub package_members:u32,
    #[prost(string,tag="5")]
    pub validation_level:String,
    #[prost(bool,tag="6")]
    pub semantic_verified:bool,
    #[prost(string,tag="7")]
    pub note:String,
}

fn invalid(code:&str)->ToolFailure{ToolFailure::new(code,code,false)}
fn supported(kind:&str)->bool{
    matches!(kind,"docx"|"xlsx"|"pptx"|"zip"|"json"|"csv"|"text"|"pdf"|"png"|"jpeg")
}
fn suffix(kind:&str)->&'static str{
    match kind {
        "docx"=>".docx","xlsx"=>".xlsx","pptx"=>".pptx","zip"=>".zip",
        "json"=>".json","csv"=>".csv","text"=>".txt","pdf"=>".pdf",
        "png"=>".png","jpeg"=>".jpg",_=>"",
    }
}
fn valid_path(path:&str,kind:&str)->bool{
    !path.is_empty()&&path.len()<=4096&&supported(kind)&&
    !path.chars().any(|c|c.is_control()||c=='\\'||c==':')&&
    path.split('/').all(|p|!p.is_empty()&&p!="."&&p!=".."&&!p.starts_with('.'))&&
    (path.to_ascii_lowercase().ends_with(suffix(kind))||
      (kind=="jpeg"&&path.to_ascii_lowercase().ends_with(".jpeg")))
}
fn checked_sha(sha:&str)->bool{
    sha.is_empty()||(sha.len()==64&&sha.bytes().all(|b|b.is_ascii_hexdigit()))
}
fn read_bounded(root:&Dir,path:&str)->Result<Vec<u8>,ToolFailure>{
    let mut prefix=String::new();
    let mut parts=path.split('/').peekable();
    while let Some(part)=parts.next(){
        if !prefix.is_empty(){prefix.push('/');}
        prefix.push_str(part);
        let meta=root.symlink_metadata(&prefix).map_err(|_|invalid("ARTIFACT_UNAVAILABLE"))?;
        if meta.file_type().is_symlink()||
          (parts.peek().is_some()&&!meta.is_dir())||
          (parts.peek().is_none()&&!meta.is_file()){
            return Err(invalid("ARTIFACT_UNSAFE_PATH"))
        }
    }
    let mut file=root.open(path).map_err(|_|invalid("ARTIFACT_UNAVAILABLE"))?;
    let before=file.metadata().map_err(|_|invalid("ARTIFACT_UNAVAILABLE"))?;
    if !before.is_file()||before.len()==0||before.len()>MAX_ARTIFACT_BYTES as u64{
        return Err(invalid("ARTIFACT_SIZE_INVALID"))
    }
    let mut data=Vec::with_capacity(before.len() as usize);
    (&mut file).take(MAX_ARTIFACT_BYTES as u64+1).read_to_end(&mut data)
        .map_err(|_|invalid("ARTIFACT_READ_FAILED"))?;
    let after=file.metadata().map_err(|_|invalid("ARTIFACT_READ_FAILED"))?;
    if data.is_empty()||data.len()>MAX_ARTIFACT_BYTES||
      data.len() as u64!=before.len()||after.len()!=before.len()||
      after.modified().ok()!=before.modified().ok(){
        return Err(invalid("ARTIFACT_CHANGED_OR_EMPTY"))
    }
    Ok(data)
}
fn inspect_zip(bytes:&[u8],kind:&str)->Result<(u32,&'static str),ToolFailure>{
    let mut zip=ZipArchive::new(Cursor::new(bytes))
        .map_err(|_|invalid("ARTIFACT_BAD_ZIP"))?;
    if zip.len()==0||zip.len()>MAX_MEMBERS{return Err(invalid("ARTIFACT_ZIP_BUDGET"));}
    let mut seen=BTreeSet::new();
    let mut total=0u64;
    let mut must_have=BTreeSet::from(["[Content_Types].xml"]);
    match kind{
        "docx"=>{must_have.insert("word/document.xml");},
        "xlsx"=>{must_have.insert("xl/workbook.xml");},
        "pptx"=>{must_have.insert("ppt/presentation.xml");},
        _=>{must_have.clear();},
    }
    for n in 0..zip.len(){
        let mut entry=zip.by_index(n).map_err(|_|invalid("ARTIFACT_BAD_ZIP"))?;
        let name=entry.name().map_err(|_|invalid("ARTIFACT_UNSAFE_PACKAGE"))?.into_owned();
        if name.is_empty()||name.len()>512||name.starts_with('/')||
          name.contains('\\')||name.split('/').any(|c|c=="."||c=="..")||
          !seen.insert(name.clone()){
            return Err(invalid("ARTIFACT_UNSAFE_PACKAGE"))
        }
        if entry.size()>MAX_UNCOMPRESSED||
          total.saturating_add(entry.size())>MAX_UNCOMPRESSED{
            return Err(invalid("ARTIFACT_ZIP_BUDGET"))
        }
        total+=entry.size();
        if must_have.remove(name.as_str()){
            if entry.size()==0||entry.size()>2*1024*1024{
                return Err(invalid("ARTIFACT_BAD_PACKAGE"))
            }
            let mut xml=Vec::new();
            (&mut entry).take(2*1024*1024+1).read_to_end(&mut xml)
                .map_err(|_|invalid("ARTIFACT_BAD_PACKAGE"))?;
            if xml.len()!=entry.size() as usize||
              !xml.iter().skip_while(|c|c.is_ascii_whitespace()||**c==0xef||**c==0xbb||**c==0xbf).any(|c|*c==b'<'){
                return Err(invalid("ARTIFACT_BAD_PACKAGE"))
            }
        }
    }
    if !must_have.is_empty(){return Err(invalid("ARTIFACT_MISSING_PARTS"));}
    Ok((zip.len() as u32,if kind=="zip"{"zip_entry_metadata"}else{"ooxml_required_parts"}))
}
pub fn inspect_bytes(
    bytes:&[u8],kind:&str,expected_sha256:&str
)->Result<ArtifactInspectResult,ToolFailure>{
    if !supported(kind){return Err(invalid("ARTIFACT_FORMAT_INVALID"));}
    if bytes.is_empty()||bytes.len()>MAX_ARTIFACT_BYTES{
        return Err(invalid("ARTIFACT_SIZE_INVALID"))
    }
    if !checked_sha(expected_sha256){return Err(invalid("ARTIFACT_SHA_INVALID"));}
    let hash=format!("{:x}",Sha256::digest(bytes));
    if !expected_sha256.is_empty()&&!expected_sha256.eq_ignore_ascii_case(&hash){
        return Err(invalid("ARTIFACT_SHA_MISMATCH"))
    }
    let (members,level)=match kind{
        "docx"|"xlsx"|"pptx"|"zip"=>{
            let (m,l)=inspect_zip(bytes,kind)?;(m,l)
        },
        "json"=>{serde_json::from_slice::<Value>(bytes)
            .map_err(|_|invalid("ARTIFACT_BAD_JSON"))?;(0,"json_syntax")},
        "csv"|"text"=>{
            let value=std::str::from_utf8(bytes).map_err(|_|invalid("ARTIFACT_BAD_UTF8"))?;
            if value.trim().is_empty(){return Err(invalid("ARTIFACT_EMPTY_TEXT"));}
            (0,"utf8_only")
        },
        "pdf"=>{
            if !bytes.starts_with(b"%PDF-"){return Err(invalid("ARTIFACT_BAD_PDF_HEADER"));}
            (0,"pdf_header_only")
        },
        "png"|"jpeg"=>{
            let magic=if kind=="png"{
                bytes.starts_with(b"\x89PNG\r\n\x1a\n")
            }else{bytes.starts_with(&[0xff,0xd8,0xff])};
            if !magic{return Err(invalid("ARTIFACT_BAD_IMAGE"));}
            let format=if kind=="png"{image::ImageFormat::Png}else{image::ImageFormat::Jpeg};
            let mut reader=image::ImageReader::with_format(Cursor::new(bytes),format);
            let mut limits=image::Limits::default();
            limits.max_image_width=Some(8192);
            limits.max_image_height=Some(8192);
            limits.max_alloc=Some(128*1024*1024);
            reader.limits(limits);
            reader.decode().map_err(|_|invalid("ARTIFACT_BAD_IMAGE"))?;
            (0,"image_decoded")
        },
        _=>return Err(invalid("ARTIFACT_FORMAT_INVALID"))
    };
    Ok(ArtifactInspectResult{
        format:kind.into(),bytes:bytes.len() as u64,sha256:hash,
        package_members:members,validation_level:level.into(),
        semantic_verified:false,
        note:"Structure/bytes checked only. This does NOT verify expected contents, formulas, page layout, citations, factual accuracy, or external application acceptance.".into(),
    })
}

struct Inspector {descriptor:ToolDescriptor,root:Arc<Dir>}
impl NativeTool for Inspector{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let root=Arc::clone(&self.root);
        Box::pin(async move{
            let req=ArtifactInspectRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|invalid("ARTIFACT_REQUEST_INVALID"))?;
            if req.encode_to_vec()!=call.input.bytes||
                !valid_path(&req.path,&req.format)||!checked_sha(&req.expected_sha256){
                return Err(invalid("ARTIFACT_REQUEST_INVALID"))
            }
            tokio::task::spawn_blocking(move||{
                let bytes=read_bounded(&root,&req.path)?;
                let result=inspect_bytes(&bytes,&req.format,&req.expected_sha256)?;
                Ok(ToolOutput{payload:ProtoPayload{
                    type_url:OUTPUT.into(),bytes:result.encode_to_vec()
                }})
            }).await.map_err(|_|invalid("ARTIFACT_WORKER_FAILED"))?
        })
    }
}
struct Codec;
impl ModelToolCodec for Codec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["path","format","expected_sha256"],&["path","format"])?;
        let path=args.get("path").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("path required".into()))?;
        let format=args.get("format").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("format required".into()))?;
        let sha=args.get("expected_sha256").and_then(Value::as_str).unwrap_or("");
        if !valid_path(path,format)||!checked_sha(sha){
            return Err(ModelToolError::InvalidArguments("unsafe path, format or SHA".into()))
        }
        Ok(ProtoPayload{type_url:INPUT.into(),bytes:ArtifactInspectRequest{
            path:path.into(),format:format.into(),expected_sha256:sha.into()
        }.encode_to_vec()})
    }
    fn decode(&self,output:&ProtoPayload)->Result<Value,ModelToolError>{
        let result=ArtifactInspectResult::decode(output.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("invalid artifact receipt".into()))?;
        Ok(json!({"format":result.format,"bytes":result.bytes,
            "sha256":result.sha256,"package_members":result.package_members,
            "validation_level":result.validation_level,
            "semantic_verified":result.semantic_verified,"note":result.note}))
    }
}
/// Host supplies a human-approved directory. Registration is not an authority
/// grant; the existing conversation/task dispatcher decides each invocation.
pub fn register_artifact_inspection(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,approved_root:&Path
)->Result<(),String>{
    let root=approved_root.canonicalize().map_err(|_|"artifact root unavailable")?;
    if !root.is_dir(){return Err("artifact root unavailable".into());}
    let root=Arc::new(Dir::open_ambient_dir(root,ambient_authority())
        .map_err(|_|"artifact root unavailable")?);
    let descriptor=ToolDescriptor{id:TOOL_ID.into(),
        display_name:"Verify bounded output file structure and digest".into(),
        effect:ToolEffect::ReadWorkspace,input_type_url:INPUT.into(),
        output_type_url:OUTPUT.into()};
    registry.register(ModuleManifest{
        id:"lu.artifact".into(),display_name:"Lu artifact verification".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![descriptor.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(Inspector{descriptor,root})])
        .map_err(|_|"artifact native registration denied")?;
    catalog.insert(registry,ModelToolDefinition{
        name:"lu_artifact_inspect".into(),native_tool_id:TOOL_ID.into(),
        description:"Read-only inspect an EXISTING approved file: byte count, SHA-256, format/container structure. Never certifies file contents, layout, formulas or user task success. For JPG use jpeg format.".into(),
        parameters:json!({"type":"object","properties":{
            "path":{"type":"string","description":"Workspace-relative filename"},
            "format":{"type":"string","enum":["docx","xlsx","pptx","zip","json","csv","text","pdf","png","jpeg"]},
            "expected_sha256":{"type":"string","description":"Optional expected full SHA-256"}
        },"required":["path","format"],"additionalProperties":false}),
        codec:Arc::new(Codec),
    }).map_err(|_|"artifact model schema rejected")?;
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;
    use zip::{ZipWriter,write::SimpleFileOptions};
    use std::io::Write;
    fn package(include_sheet:bool)->Vec<u8>{
        let mut cursor=Cursor::new(Vec::new());
        {
            let mut zip=ZipWriter::new(&mut cursor);
            for (name,body) in [("[Content_Types].xml",&b"<Types/>"[..]),
                ("xl/workbook.xml",b"<workbook/>"),
                ("xl/worksheets/sheet1.xml",b"<worksheet/>")]{
                if !include_sheet&&name=="xl/workbook.xml"{continue;}
                zip.start_file(name,SimpleFileOptions::default()).unwrap();
                zip.write_all(body).unwrap();
            }
            zip.finish().unwrap();
        }
        cursor.into_inner()
    }
    #[test]
    fn zero_byte_and_renamed_text_are_not_xlsx(){
        assert!(inspect_bytes(b"","xlsx","").is_err());
        assert!(inspect_bytes(b"fake xlsx","xlsx","").is_err());
        assert!(inspect_bytes(&package(false),"xlsx","").is_err());
        let evidence=inspect_bytes(&package(true),"xlsx","").unwrap();
        assert_eq!(evidence.validation_level,"ooxml_required_parts");
        assert!(evidence.package_members>=2);
        assert!(!evidence.semantic_verified);
    }
    #[test]
    fn sha_mismatch_and_unparsed_json_fail_closed(){
        assert!(inspect_bytes(b"{broken","json","").is_err());
        assert!(inspect_bytes(br#"{"ok":true}"#,"json",&"0".repeat(64)).is_err());
        let evidence=inspect_bytes(br#"{"ok":true}"#,"json","").unwrap();
        assert_eq!(evidence.validation_level,"json_syntax");
        assert!(!evidence.semantic_verified);
    }
    #[test]
    fn reject_unsafe_paths_and_impossible_formats(){
        assert!(!valid_path("../secret.xlsx","xlsx"));
        assert!(!valid_path(".hidden.xlsx","xlsx"));
        assert!(!valid_path("C:/a.xlsx","xlsx"));
        assert!(!valid_path("a.xls","xlsx"));
        assert!(valid_path("output/new.xlsx","xlsx"));
    }
}
