//! A first-party Reader tool for authorized workspace text documents.
//! Unsupported PDF and OCR/image files deliberately return an adapter
//! error until their audited parsers are installed. No MCP or OS shell.
use cap_std::{ambient_authority,fs::Dir};
use prost::Message;
use serde_json::{json,Map,Value};
use std::{io::Read,path::{Path,PathBuf},sync::Arc};
use crate::{
    contracts::{ModuleKind,ModuleManifest,NativeTool,ProtoPayload,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,
        ToolOutput,CORE_CONTRACT_VERSION},
    model::{expect_fields,optional_uint,ModelToolCatalog,ModelToolCodec,
        ModelToolDefinition,ModelToolError},
    native_reader::{read_text_bytes,ReaderFailure,MAX_READER_INPUT_BYTES},
    registry::{NativeToolRegistry,RegistryError},
};

pub const TOOL_ID:&str="lu.reader.read";
pub const INPUT_TYPE:&str="type.googleapis.com/lu.v1.ReaderRequest";
pub const OUTPUT_TYPE:&str="type.googleapis.com/lu.v1.ReaderResponse";
const MAX_BYTES:u32=1024*1024;
const DEFAULT_BYTES:u32=256*1024;
const MAX_CHARS:u32=32_000;
const MAX_OFFSET_CHARS:u32=200_000;
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
}
pub struct ReaderTool{
    descriptor:ToolDescriptor,
    root:Arc<Dir>,
}
fn failure(code:&str)->ToolFailure{
    ToolFailure::new(code,code,false)
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
fn read_authorized(root:&Dir,request:ReaderRequest)
    ->Result<ReaderResponse,ToolFailure>{
    let max_bytes=request.max_bytes.unwrap_or(DEFAULT_BYTES);
    let max_chars=request.max_chars.unwrap_or(DEFAULT_CHARS);
    let offset=request.offset_chars.unwrap_or(0);
    if offset>MAX_OFFSET_CHARS{return Err(failure("READER_INVALID_OFFSET"));}
    if max_bytes==0||max_bytes>MAX_BYTES
        ||max_chars==0||max_chars>MAX_CHARS
        ||u64::from(max_bytes)>MAX_READER_INPUT_BYTES{
        return Err(failure("READER_INVALID_LIMIT"));
    }
    let path=relative_path(&request.path)?;
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
    // Parse once into the bounded host extraction window, then return only
    // this invocation's authorized slice. A later tool call can page again.
    let text=read_text_bytes(&request.path,&bytes,MAX_OFFSET_CHARS as usize)
        .map_err(|err|match err{
            ReaderFailure::UnsupportedAdapter(_)=>failure("READER_ADAPTER_UNAVAILABLE"),
            ReaderFailure::UnsupportedEncoding=>failure("READER_INVALID_ENCODING"),
            ReaderFailure::InvalidDocument=>failure("READER_INVALID_DOCUMENT"),
            ReaderFailure::TooLarge=>failure("READER_INPUT_TOO_LARGE"),
            ReaderFailure::Empty=>failure("READER_EMPTY"),
            ReaderFailure::Io=>failure("READER_FILE_UNAVAILABLE"),
        })?;
    let all_chars=text.text.chars().count();
    if offset as usize>all_chars{
        return Err(failure("READER_OFFSET_BEYOND_EXTRACTED_TEXT"));
    }
    let end=(offset as usize).saturating_add(max_chars as usize).min(all_chars);
    let chunk:String=text.text.chars().skip(offset as usize)
        .take(end-offset as usize).collect();
    let has_next=end<all_chars;
    let next_offset_chars=if has_next{Some(end as u32)}else{None};
    Ok(ReaderResponse{
        format:format!("{:?}",text.format),
        text:chunk,truncated:has_next||text.truncated,
        bytes_read:bytes.len() as u64,
        offset_chars:offset,next_offset_chars,
        source_truncated:text.truncated,
    })
}
impl NativeTool for ReaderTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,invocation:ToolInvocation)->ToolFuture<'_>{
        let root=Arc::clone(&self.root);
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
            let result=tokio::task::spawn_blocking(move||read_authorized(&root,request))
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
        expect_fields(args,&["path","max_bytes","max_chars","offset_chars"],&["path"])?;
        let path=args.get("path").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments(
                "Reader path must be a string".into()))?;
        relative_path(path).map_err(|_|ModelToolError::InvalidArguments(
            "Reader requires a workspace-relative safe path".into()))?;
        let max_bytes=optional_uint(args,"max_bytes",DEFAULT_BYTES,MAX_BYTES)?;
        let max_chars=optional_uint(args,"max_chars",DEFAULT_CHARS,MAX_CHARS)?;
        let offset=match args.get("offset_chars"){
            None=>0,
            Some(v)=>v.as_u64().filter(|n|*n<=u64::from(MAX_OFFSET_CHARS))
                .ok_or_else(||ModelToolError::InvalidArguments(
                    "offset_chars must be between 0 and 200000".into()))? as u32,
        };
        Ok(ProtoPayload{type_url:INPUT_TYPE.into(),
            bytes:ReaderRequest{
                path:path.into(),max_bytes:Some(max_bytes),
                max_chars:Some(max_chars),
                offset_chars:Some(offset),
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
            "source_truncated":output.source_truncated}))
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
    },vec![Arc::new(ReaderTool{descriptor,root}) as Arc<dyn NativeTool>])
        .map_err(|e:RegistryError|format!("{e:?}"))?;
    let mut catalog=ModelToolCatalog::new();
    catalog.insert(registry,ModelToolDefinition{
        name:"lu_reader_read".into(),native_tool_id:TOOL_ID.into(),
        description:"Read authorized TXT, MD, DOCX, XLSX cells/formulas, PPTX slides, JSON, CSV, config and source files. PDF and image OCR are not yet supported.".into(),
        parameters:json!({"type":"object","properties":{
            "path":{"type":"string","description":"Authorized workspace-relative file path"},
            "max_bytes":{"type":"integer","minimum":1,"maximum":MAX_BYTES},
            "max_chars":{"type":"integer","minimum":1,"maximum":MAX_CHARS},
            "offset_chars":{"type":"integer","minimum":0,"maximum":MAX_OFFSET_CHARS}
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
                    path:file.into(),max_bytes:None,max_chars:None,offset_chars:None
                }.encode_to_vec()}}
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
