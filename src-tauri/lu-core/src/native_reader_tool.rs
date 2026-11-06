//! A first-party Reader tool for authorized workspace text documents.
//! Unsupported scanned PDF and OCR/image files deliberately return an adapter
//! error until their audited parsers are installed. No MCP or OS shell.
use cap_std::{ambient_authority,fs::Dir};
use prost::Message;
use sha2::{Digest,Sha256};
use serde_json::{json,Map,Value};
use std::{io::Read,path::{Path,PathBuf},sync::Arc};
use crate::{
    contracts::{ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,
        ToolOutput,CORE_CONTRACT_VERSION},
    model::{expect_fields,optional_uint,ModelToolCatalog,ModelToolCodec,
        ModelToolDefinition,ModelToolError},
    native_reader::{read_selected_bytes,read_plain_text_window,ReaderFailure,MAX_READER_INPUT_BYTES},
    registry::{NativeToolRegistry,RegistryError},
};

pub const TOOL_ID:&str="lu.reader.read";
pub const INPUT_TYPE:&str="type.googleapis.com/lu.v1.ReaderRequest";
pub const OUTPUT_TYPE:&str="type.googleapis.com/lu.v1.ReaderResponse";
const MAX_BYTES:u32=8*1024*1024;
const DEFAULT_BYTES:u32=256*1024;
const MAX_CHARS:u32=32_000;
// Source byte admission is capped at 8 MiB. No decoded plain-text file
// under this limit can contain more than 8 MiB Unicode scalar values.
const MAX_OFFSET_CHARS:u32=MAX_BYTES;
const DEFAULT_CHARS:u32=16_000;

#[derive(Clone,PartialEq,Message)]
pub struct ReaderRequest{
    #[prost(string,tag="1")]
    pub path:String,
    #[prost(uint32,optional,tag="2")]
    pub max_bytes:Option<u32>,
    #[prost(uint32,optional,tag="3")]
    pub max_chars:Option<u32>,
    #[prost(uint32,optional,tag="4")]
    pub offset_chars:Option<u32>,
    #[prost(bool,tag="5")]
    pub inspect_only:bool,
    #[prost(string,tag="6")]
    pub selector:String,
    #[prost(string,tag="7")]
    pub expected_source_sha256:String,
}
#[derive(Clone,PartialEq,Message)]
pub struct ReaderResponse{
    #[prost(string,tag="1")]
    pub format:String,
    #[prost(string,tag="2")]
    pub text:String,
    #[prost(bool,tag="3")]
    pub truncated:bool,
    #[prost(uint64,tag="4")]
    pub bytes_read:u64,
    #[prost(uint32,tag="5")]
    pub offset_chars:u32,
    #[prost(uint32,optional,tag="6")]
    pub next_offset_chars:Option<u32>,
    #[prost(bool,tag="7")]
    pub source_truncated:bool,
    #[prost(uint32,tag="8")]
    pub total_extracted_chars:u32,
    #[prost(string,tag="9")]
    pub source_sha256:String,
}
enum ReaderSource{
    Workspace(Arc<Dir>),
    /// Host-copied, immutable bytes. The model sees only a virtual filename,
    /// never the original OS path or an ambient filesystem capability.
    Pinned{ filename:String, bytes:Arc<Vec<u8>> },
}
pub struct ReaderTool{
    descriptor:ToolDescriptor,
    source:Arc<ReaderSource>,
}
fn failure(code:&str)->ToolFailure{
    ToolFailure::new(code,code,false)
}
fn reader_parse_failure(err:ReaderFailure)->ToolFailure{
    match err{
        ReaderFailure::UnsupportedAdapter(_)=>failure("READER_ADAPTER_UNAVAILABLE"),
        ReaderFailure::UnsupportedEncoding=>failure("READER_INVALID_ENCODING"),
        ReaderFailure::InvalidDocument=>failure("READER_INVALID_DOCUMENT"),
        ReaderFailure::SelectorNotFound=>failure("READER_SELECTOR_NOT_FOUND"),
        ReaderFailure::TooLarge=>failure("READER_INPUT_TOO_LARGE"),
        ReaderFailure::Empty=>failure("READER_EMPTY"),
        ReaderFailure::Io=>failure("READER_FILE_UNAVAILABLE"),
    }
}
fn relative_path(raw:&str)->Result<PathBuf,ToolFailure>{
    if raw.is_empty()||raw.len()>4096
        ||raw.chars().any(|c|matches!(c,'\\'|':'|'\0'))
        ||raw.split('/').any(|p|p.is_empty()||p=="."||p=="..")
        ||Path::new(raw).is_absolute(){
        return Err(failure("READER_INVALID_PATH"));
    }
    Ok(PathBuf::from(raw))
}
fn read_authorized(source:&ReaderSource,request:ReaderRequest)
    ->Result<ReaderResponse,ToolFailure>{
    // In-memory bytes were individually admitted by a native operator.
    // Workspace directory reads retain the conservative 256 KiB default;
    // omitting max_bytes must not make approved larger documents unreadable.
    let max_bytes=request.max_bytes.unwrap_or(match source{
        ReaderSource::Workspace(_)=>DEFAULT_BYTES,
        ReaderSource::Pinned{bytes,..}=>bytes.len() as u32,
    });
    let max_chars=request.max_chars.unwrap_or(DEFAULT_CHARS);
    let offset=request.offset_chars.unwrap_or(0);
    if request.selector.len()>160 || request.selector.chars().any(|ch|ch=='\0'){
        return Err(failure("READER_INVALID_SELECTOR"));
    }
    if request.inspect_only&&offset!=0{return Err(failure("READER_INVALID_INSPECT_REQUEST"));}
    if offset>MAX_OFFSET_CHARS{return Err(failure("READER_INVALID_OFFSET"));}
    if max_bytes==0||max_bytes>MAX_BYTES
        ||max_chars==0||max_chars>MAX_CHARS
        ||u64::from(max_bytes)>MAX_READER_INPUT_BYTES{
        return Err(failure("READER_INVALID_LIMIT"));
    }
    let path=relative_path(&request.path)?;
    let bytes=match source{
        ReaderSource::Workspace(root)=>{
            let meta=root.symlink_metadata(&path)
                .map_err(|_|failure("READER_FILE_UNAVAILABLE"))?;
            if meta.file_type().is_symlink()||!meta.is_file(){
                return Err(failure("READER_UNSAFE_FILE"));
            }
            let mut file=root.open(&path)
                .map_err(|_|failure("READER_FILE_UNAVAILABLE"))?;
            if !file.metadata().map_err(|_|failure("READER_FILE_UNAVAILABLE"))?
                .is_file(){
                return Err(failure("READER_UNSAFE_FILE"));
            }
            let mut bytes=Vec::new();
            file.by_ref().take(u64::from(max_bytes)+1).read_to_end(&mut bytes)
                .map_err(|_|failure("READER_FILE_UNAVAILABLE"))?;
            if bytes.len()>max_bytes as usize{
                return Err(failure("READER_INPUT_TOO_LARGE"));
            }
            bytes
        },
        ReaderSource::Pinned{filename,bytes}=>{
            if &request.path!=filename{
                return Err(failure("READER_FILE_NOT_GRANTED"));
            }
            if bytes.len()>max_bytes as usize{
                return Err(failure("READER_INPUT_TOO_LARGE"));
            }
            (**bytes).clone()
        },
    };
    let source_sha256=format!("{:x}",Sha256::digest(&bytes));
    if !request.expected_source_sha256.is_empty() &&
       request.expected_source_sha256!=source_sha256{
        return Err(failure("READER_SOURCE_CHANGED"));
    }
    // For ordinary source/text files, page the entire UTF-8/UTF-16 input
    // instead of always truncating the extraction at character 200,000.
    // Office and PDF retain their separately bounded extraction adapters.
    let plain=if request.selector.is_empty(){
        read_plain_text_window(
            &request.path,&bytes,offset as usize,max_chars as usize,
            request.inspect_only,
        ).map_err(reader_parse_failure)?
    }else{None};
    let (format,chunk,all_chars,source_truncated)=if let Some(page)=plain{
        if offset as usize>page.total_chars{
            return Err(failure("READER_OFFSET_BEYOND_EXTRACTED_TEXT"));
        }
        (format!("{:?}",page.format),page.text,page.total_chars,false)
    }else{
        let parsed=read_selected_bytes(
            &request.path,&bytes,200_000,&request.selector,
        ).map_err(reader_parse_failure)?;
        let count=parsed.text.chars().count();
        if offset as usize>count{
            return Err(failure("READER_OFFSET_BEYOND_EXTRACTED_TEXT"));
        }
        let end=(offset as usize).saturating_add(max_chars as usize).min(count);
        let chunk=if request.inspect_only{String::new()}else{
            parsed.text.chars().skip(offset as usize)
                .take(end-offset as usize).collect()
        };
        (format!("{:?}",parsed.format),chunk,count,parsed.truncated)
    };
    let end=(offset as usize).saturating_add(max_chars as usize).min(all_chars);
    let has_next=!request.inspect_only&&end<all_chars;
    Ok(ReaderResponse{
        format,text:chunk,truncated:has_next||source_truncated,
        bytes_read:bytes.len() as u64,
        offset_chars:offset,next_offset_chars:if has_next{Some(end as u32)}else{None},
        source_truncated,total_extracted_chars:all_chars as u32,
        source_sha256,
    })
}
impl NativeTool for ReaderTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,invocation:ToolInvocation)->ToolFuture<'_>{
        let source=Arc::clone(&self.source);
        Box::pin(async move{
            if invocation.tool_id!=TOOL_ID
                ||invocation.input.type_url!=INPUT_TYPE
                ||invocation.input.bytes.len()>8192{
                return Err(failure("READER_INVALID_ENVELOPE"));
            }
            let request=ReaderRequest::decode(invocation.input.bytes.as_slice())
                .map_err(|_|failure("READER_BAD_PROTOBUF"))?;
            if request.encode_to_vec()!=invocation.input.bytes{
                return Err(failure("READER_NONCANONICAL_PROTOBUF"));
            }
            let result=tokio::task::spawn_blocking(move||read_authorized(&source,request))
                .await.map_err(|_|failure("READER_WORKER_UNAVAILABLE"))??;
            Ok(ToolOutput{payload:ProtoPayload{
                type_url:OUTPUT_TYPE.into(),bytes:result.encode_to_vec(),
            }})
        })
    }
}
struct ReaderCodec;
impl ModelToolCodec for ReaderCodec{
    fn input_type_url(&self)->&'static str{INPUT_TYPE}
    fn output_type_url(&self)->&'static str{OUTPUT_TYPE}
    fn encode(&self,args:&Map<String,Value>)
        ->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["path","max_bytes","max_chars","offset_chars","inspect_only","selector","expected_source_sha256"],&["path"])?;
        let path=args.get("path").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments(
                "Reader path must be a string".into()))?;
        relative_path(path).map_err(|_|ModelToolError::InvalidArguments(
            "Reader requires a workspace-relative safe path".into()))?;
        let max_bytes=if args.contains_key("max_bytes"){
            Some(optional_uint(args,"max_bytes",DEFAULT_BYTES,MAX_BYTES)?)
        }else{None};
        let max_chars=optional_uint(args,"max_chars",DEFAULT_CHARS,MAX_CHARS)?;
        let offset=match args.get("offset_chars"){
            None=>0,
            Some(v)=>v.as_u64().filter(|n|*n<=u64::from(MAX_OFFSET_CHARS))
                .ok_or_else(||ModelToolError::InvalidArguments(
                    "offset_chars exceeds the 8 MiB source window".into()))? as u32,
        };
        let inspect_only=match args.get("inspect_only"){
            None=>false,
            Some(Value::Bool(v))=>*v,
            _=>return Err(ModelToolError::InvalidArguments(
                "inspect_only must be a boolean".into())),
        };
        if inspect_only&&offset!=0{return Err(ModelToolError::InvalidArguments(
            "inspection must start at character offset zero".into()))}
        let selector=match args.get("selector"){
            None=>"",
            Some(Value::String(s)) if s.len()<=160&&!s.contains('\0')=>s,
            _=>return Err(ModelToolError::InvalidArguments(
                "selector must be a string of up to 160 characters".into())),
        };
        let expected_source_sha256=match args.get("expected_source_sha256"){
            None=>"",
            Some(Value::String(s)) if s.len()==64
                &&s.bytes().all(|b|b.is_ascii_hexdigit())
                &&s.bytes().all(|b|!b.is_ascii_uppercase())=>s,
            _=>return Err(ModelToolError::InvalidArguments(
                "expected_source_sha256 must be a 64-character lowercase hex digest".into()))
        };
        Ok(ProtoPayload{type_url:INPUT_TYPE.into(),
            bytes:ReaderRequest{
                path:path.into(),max_bytes,
                max_chars:Some(max_chars),
                offset_chars:Some(offset),inspect_only,
                selector:selector.to_owned(),
                expected_source_sha256:expected_source_sha256.to_owned(),
            }.encode_to_vec()})
    }
    fn decode(&self,result:&ProtoPayload)->Result<Value,ModelToolError>{
        if result.type_url!=OUTPUT_TYPE{
            return Err(ModelToolError::InvalidResult("Wrong Reader response".into()));
        }
        let output=ReaderResponse::decode(result.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult(
                "Malformed Reader response".into()))?;
        Ok(json!({"format":output.format,"text":output.text,
            "truncated":output.truncated,"bytes_read":output.bytes_read,
            "offset_chars":output.offset_chars,
            "next_offset_chars":output.next_offset_chars,
            "source_truncated":output.source_truncated,
            "total_extracted_chars":output.total_extracted_chars,
            "source_sha256":output.source_sha256}))
    }
}
/// Register Reader for a host-approved directory; registration itself does
/// not grant a model access. The registry's per-turn authorizer remains required.
pub fn register_reader(
    registry:&NativeToolRegistry,approved_root:&Path
)->Result<ModelToolCatalog,String>{
    let root=approved_root.canonicalize().map_err(|_|"Invalid Reader root")?;
    if !root.is_dir(){return Err("Reader root is not a directory".into());}
    let root=Arc::new(Dir::open_ambient_dir(root,ambient_authority())
        .map_err(|_|"Cannot open Reader root")?);
    register_reader_source(registry,ReaderSource::Workspace(root))
}
/// Register exactly one document chosen and copied by the trusted host.
/// The model never sees the source path and cannot open neighboring files.
/// Use a fresh registry per approved turn and let the conversation authority
/// revoke dispatch independently; a catalog is *not* a permission.
pub fn register_reader_pinned(
    registry:&NativeToolRegistry,filename:&str,bytes:Vec<u8>
)->Result<ModelToolCatalog,String>{
    let path=relative_path(filename).map_err(|_|"Invalid pinned document name")?;
    if path.components().count()!=1||bytes.is_empty()||bytes.len()>MAX_BYTES as usize{
        return Err("Pinned Reader requires one safe file <=8 MiB".into());
    }
    let ext=crate::native_reader::classify(filename,&bytes[..bytes.len().min(16)]);
    if matches!(ext,crate::native_reader::ReaderFormat::Unknown){
        return Err("Unrecognized pinned document format".into());
    }
    register_reader_source(registry,ReaderSource::Pinned{
        filename:filename.to_owned(),bytes:Arc::new(bytes),
    })
}
fn register_reader_source(
    registry:&NativeToolRegistry,source:ReaderSource
)->Result<ModelToolCatalog,String>{
    let descriptor=ToolDescriptor{
        id:TOOL_ID.into(),display_name:"Read document text".into(),
        effect:ToolEffect::ReadWorkspace,
        input_type_url:INPUT_TYPE.into(),output_type_url:OUTPUT_TYPE.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.reader".into(),display_name:"Lu Reader".into(),
        contract_version:CORE_CONTRACT_VERSION,
        kind:ModuleKind::Core,tools:vec![descriptor.clone()],
        panels:vec![],workflows:vec![],
    },vec![Arc::new(ReaderTool{descriptor,source:Arc::new(source)}) as Arc<dyn NativeTool>])
        .map_err(|e:RegistryError|format!("{e:?}"))?;
    let mut catalog=ModelToolCatalog::new();
    catalog.insert(registry,ModelToolDefinition{
        name:"lu_reader_read".into(),native_tool_id:TOOL_ID.into(),
        description:"Read authorized text, PDF text layers, DOCX, XLSX/ODS sheet or A1 cell ranges, PPTX/ODP slides, JSON, CSV and source files. Scanned PDFs and image OCR require separate explicit offline review.".into(),
        parameters:json!({"type":"object","properties":{
            "path":{"type":"string","description":"Authorized workspace-relative file path"},
            "max_bytes":{"type":"integer","minimum":1,"maximum":MAX_BYTES},
            "max_chars":{"type":"integer","minimum":1,"maximum":MAX_CHARS},
            "offset_chars":{"type":"integer","minimum":0,"maximum":MAX_OFFSET_CHARS},
            "inspect_only":{"type":"boolean","description":"Return bounded document format and extraction size without text; then page with offset_chars"},
            "selector":{"type":"string","maxLength":160,"description":"Optional sheet:Revenue, range:Revenue!A1:D50, slide:2 or page:3; limits extraction to the chosen section"},
            "expected_source_sha256":{"type":"string","description":"Optional digest from prior Reader response; refuses changed files between paged reads"}
        },"required":["path"],"additionalProperties":false}),
        codec:Arc::new(ReaderCodec),
    }).map_err(|e|format!("{e:?}"))?;
    Ok(catalog)
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::contracts::{AuthorizationError,ToolAuthorizer};
    struct Allow;
    impl ToolAuthorizer for Allow{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
          ->Result<(),AuthorizationError>{Ok(())}
    }
    struct Deny;
    impl ToolAuthorizer for Deny{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
          ->Result<(),AuthorizationError>{
            Err(AuthorizationError::new("denied"))
        }
    }
    fn workspace()->PathBuf{
        std::env::temp_dir().join(format!(
            "lu-reader-{}-{:?}",std::process::id(),
            std::thread::current().id()))
    }
    fn invocation(file:&str)->ToolInvocation{
        ToolInvocation{call_id:"read-call-1".into(),tool_id:TOOL_ID.into(),
            workspace_id:Some("project".into()),input:ProtoPayload{
                type_url:INPUT_TYPE.into(),bytes:ReaderRequest{
                    path:file.into(),max_bytes:None,max_chars:None,offset_chars:None,inspect_only:false,selector:String::new(),expected_source_sha256:String::new()
                }.encode_to_vec()}}
    }
    #[tokio::test]
    async fn larger_pinned_document_is_readable_without_model_byte_limit_hint(){
        let registry=NativeToolRegistry::new();
        let payload="x".repeat(400_000).into_bytes();
        register_reader_pinned(&registry,"long.txt",payload).unwrap();
        let response=registry.dispatch(invocation("long.txt"),&Allow).await.unwrap();
        let output=ReaderResponse::decode(response.payload.bytes.as_slice()).unwrap();
        assert_eq!(output.bytes_read,400_000);
        assert!(!output.source_truncated);
        assert_eq!(output.total_extracted_chars,400_000);
        assert_eq!(output.next_offset_chars,Some(16_000));
        let mut req=ReaderRequest::decode(invocation("long.txt").input.bytes.as_slice()).unwrap();
        req.max_bytes=Some(1024);
        let limited=ToolInvocation{
            call_id:"read-limited".into(),tool_id:TOOL_ID.into(),
            workspace_id:Some("project".into()),input:ProtoPayload{
                type_url:INPUT_TYPE.into(),bytes:req.encode_to_vec()
            }
        };
        assert!(registry.dispatch(limited,&Allow).await.is_err());
    }
    #[tokio::test]
    async fn full_plain_text_paging_keeps_unicode_and_reports_true_end(){
        let registry=NativeToolRegistry::new();
        let payload=format!("{}{}", "x".repeat(300_000),"🦈".repeat(10_000)).into_bytes();
        register_reader_pinned(&registry,"logs.txt",payload).unwrap();
        let mut req=invocation("logs.txt");
        let mut request=ReaderRequest::decode(req.input.bytes.as_slice()).unwrap();
        request.offset_chars=Some(299_999);
        request.max_chars=Some(4);
        req.input.bytes=request.encode_to_vec();
        let output=ReaderResponse::decode(registry.dispatch(req.clone(),&Allow)
            .await.unwrap().payload.bytes.as_slice()).unwrap();
        assert_eq!(output.text,"x🦈🦈🦈");
        assert_eq!(output.total_extracted_chars,310_000);
        assert!(!output.source_truncated);
        assert_eq!(output.next_offset_chars,Some(300_003));
        request.offset_chars=Some(309_999);
        request.max_chars=Some(4);
        req.input.bytes=request.encode_to_vec();
        let output=ReaderResponse::decode(registry.dispatch(req.clone(),&Allow)
            .await.unwrap().payload.bytes.as_slice()).unwrap();
        assert_eq!(output.text,"🦈");
        assert_eq!(output.next_offset_chars,None);
        assert!(!output.truncated);
        request.inspect_only=true;
        request.offset_chars=Some(0);
        req.input.bytes=request.encode_to_vec();
        let output=ReaderResponse::decode(registry.dispatch(req,&Allow)
            .await.unwrap().payload.bytes.as_slice()).unwrap();
        assert!(output.text.is_empty());
        assert_eq!(output.total_extracted_chars,310_000);
    }
    #[tokio::test]
    async fn pinned_document_never_opens_other_paths_or_mutated_disk_content(){
        let path=workspace();let _=std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("memo.md"),"secret disk contents").unwrap();
        let registry=NativeToolRegistry::new();
        let _catalog=register_reader_pinned(
            &registry,"memo.md",b"approved immutable snapshot".to_vec()).unwrap();
        let response=registry.dispatch(invocation("memo.md"),&Allow).await.unwrap();
        let parsed=ReaderResponse::decode(response.payload.bytes.as_slice()).unwrap();
        assert_eq!(parsed.text,"approved immutable snapshot");
        std::fs::write(path.join("memo.md"),"modified disk contents").unwrap();
        let response=registry.dispatch(invocation("memo.md"),&Allow).await.unwrap();
        let parsed=ReaderResponse::decode(response.payload.bytes.as_slice()).unwrap();
        assert_eq!(parsed.text,"approved immutable snapshot");
        assert!(registry.dispatch(invocation("other.md"),&Allow).await.is_err());
        assert!(registry.dispatch(invocation("../memo.md"),&Allow).await.is_err());
        assert!(registry.dispatch(invocation("memo.md"),&Deny).await.is_err());
        assert!(register_reader_pinned(&NativeToolRegistry::new(),
            "../memo.md",b"secret".to_vec()).is_err());
        let _=std::fs::remove_dir_all(path);
    }
    #[tokio::test]
    async fn source_fingerprint_rejects_mutation_between_reader_pages(){
        let path=workspace();let _=std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("report.txt"),"unchanged version").unwrap();
        let registry=NativeToolRegistry::new();
        let _catalog=register_reader(&registry,&path).unwrap();
        let mut invocation=invocation("report.txt");
        let result=registry.dispatch(invocation.clone(),&Allow).await.unwrap();
        let first=ReaderResponse::decode(result.payload.bytes.as_slice()).unwrap();
        assert_eq!(first.source_sha256.len(),64);
        let mut request=ReaderRequest::decode(invocation.input.bytes.as_slice()).unwrap();
        request.expected_source_sha256=first.source_sha256;
        request.offset_chars=Some(3);
        invocation.input.bytes=request.encode_to_vec();
        assert!(registry.dispatch(invocation.clone(),&Allow).await.is_ok());
        std::fs::write(path.join("report.txt"),"changed version").unwrap();
        let result=registry.dispatch(invocation,&Allow).await;
        assert!(result.is_err());
        let _=std::fs::remove_dir_all(path);
    }
    #[tokio::test]
    async fn inspect_then_read_uses_one_native_tool_and_no_file_text_on_inspect(){
        let path=workspace();let _=std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("notes.md"),"Reader inspection content").unwrap();
        let registry=NativeToolRegistry::new();
        let _catalog=register_reader(&registry,&path).unwrap();
        let mut invocation=invocation("notes.md");
        let mut req=ReaderRequest::decode(invocation.input.bytes.as_slice()).unwrap();
        req.inspect_only=true;
        invocation.input.bytes=req.encode_to_vec();
        let reply=registry.dispatch(invocation.clone(),&Allow).await.unwrap();
        let info=ReaderResponse::decode(reply.payload.bytes.as_slice()).unwrap();
        assert_eq!(info.format,"Markdown");
        assert!(info.text.is_empty());
        assert_eq!(info.total_extracted_chars,25);
        assert_eq!(info.next_offset_chars,None);
        req.inspect_only=false;
        req.max_chars=Some(8);
        invocation.input.bytes=req.encode_to_vec();
        let reply=registry.dispatch(invocation,&Allow).await.unwrap();
        let body=ReaderResponse::decode(reply.payload.bytes.as_slice()).unwrap();
        assert_eq!(body.text,"Reader i");
        assert_eq!(body.next_offset_chars,Some(8));
        let _=std::fs::remove_dir_all(path);
    }
    #[tokio::test]
    async fn pages_unicode_document_without_splitting_codepoints_or_granting_paths(){
        let path=workspace();let _=std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("sample.txt"),"αβγδεζηθ").unwrap();
        let registry=NativeToolRegistry::new();
        let _catalog=register_reader(&registry,&path).unwrap();
        let mut req=invocation("sample.txt");
        req.input.bytes=ReaderRequest{
            path:"sample.txt".into(),max_bytes:None,
            max_chars:Some(3),offset_chars:Some(2),
            inspect_only:false,selector:String::new(),expected_source_sha256:String::new(),
        }.encode_to_vec();
        let result=registry.dispatch(req.clone(),&Allow).await.unwrap();
        let response=ReaderResponse::decode(result.payload.bytes.as_slice()).unwrap();
        assert_eq!(response.text,"γδε");
        assert_eq!(response.offset_chars,2);
        assert_eq!(response.next_offset_chars,Some(5));
        assert!(response.truncated);
        assert!(!response.source_truncated);
        let mut next=ReaderRequest::decode(req.input.bytes.as_slice()).unwrap();
        next.offset_chars=Some(5);
        req.input.bytes=next.encode_to_vec();
        let response=ReaderResponse::decode(registry.dispatch(req.clone(),&Allow).await
            .unwrap().payload.bytes.as_slice()).unwrap();
        assert_eq!(response.text,"ζηθ");
        assert_eq!(response.next_offset_chars,None);
        assert!(!response.truncated);
        next.offset_chars=Some(100);
        req.input.bytes=next.encode_to_vec();
        assert!(registry.dispatch(req,&Allow).await.is_err());
        let _=std::fs::remove_dir_all(path);
    }
    #[tokio::test]
    async fn registered_reader_is_scoped_and_no_ocr_is_claimed(){
        let path=workspace();let _=std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("readme.md"),"# Verified source").unwrap();
        std::fs::write(path.join("scan.pdf"),"%PDF-1.7").unwrap();
        let registry=NativeToolRegistry::new();
        let _catalog=register_reader(&registry,&path).unwrap();
        let result=registry.dispatch(invocation("readme.md"),&Allow).await.unwrap();
        let output=ReaderResponse::decode(result.payload.bytes.as_slice()).unwrap();
        assert_eq!(output.format,"Markdown");
        assert_eq!(output.text,"# Verified source");
        assert!(registry.dispatch(invocation("scan.pdf"),&Allow).await.is_err());
        assert!(registry.dispatch(invocation("../escape"),&Allow).await.is_err());
        assert!(registry.dispatch(invocation("readme.md"),&Deny).await.is_err());
        let _=std::fs::remove_dir_all(path);
    }
}
