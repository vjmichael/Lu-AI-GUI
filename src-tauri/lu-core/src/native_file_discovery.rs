//! P01 / FS-DISCOVER-01: indexed, capability-scoped filesystem discovery.
//! Pure metadata is cached only in the running host, never serialized to disk.
//! An authenticated, human-selected root is required on EVERY model turn;
//! registry presence, a cache hit and a matching pathname are not authority.
//! Optional content matching is read fresh through cap-std and never cached.
use std::{
    collections::{BTreeMap,VecDeque},
    io::Read,
    path::{Path,PathBuf},
    sync::{Arc,Mutex,OnceLock},
    time::{Duration,Instant,UNIX_EPOCH},
};
use cap_std::{ambient_authority,fs::Dir};
use prost::Message;
use serde_json::{json,Map,Value};
use crate::{
    contracts::{ModuleKind,ModuleManifest,NativeTool,ProtoPayload,ToolDescriptor,
        ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    registry::NativeToolRegistry,
};

pub const ID:&str="lu.files.discover";
const INPUT:&str="type.googleapis.com/lu.v1.DiscoverFilesRequest";
const OUTPUT:&str="type.googleapis.com/lu.v1.DiscoverFilesResult";
const MAX_ENTRIES:usize=12_000;
const MAX_DIRECTORIES:usize=512;
const MAX_DEPTH:u8=20;
const MAX_RETURN:usize=80;
const MAX_CONTENT_FILES:usize=96;
const MAX_CONTENT_BYTES:u64=4*1024*1024;
const MAX_CONTENT_FILE_BYTES:u64=64*1024;
const MAX_CACHE_ROOTS:usize=4;
const REFRESH_AFTER:Duration=Duration::from_secs(15);
const MAX_DISCOVERY_RUNTIME:Duration=Duration::from_secs(4);

#[derive(Clone,PartialEq,Message)]
pub struct DiscoverFilesRequest{
    #[prost(string,tag="1")] pub query:String,
    #[prost(string,tag="2")] pub directory:String,
    #[prost(string,tag="3")] pub file_suffix:String,
    #[prost(string,tag="4")] pub kind:String,
    #[prost(uint64,optional,tag="5")] pub min_bytes:Option<u64>,
    #[prost(uint64,optional,tag="6")] pub max_bytes:Option<u64>,
    #[prost(uint64,optional,tag="7")] pub modified_since:Option<u64>,
    #[prost(uint32,optional,tag="8")] pub limit:Option<u32>,
    #[prost(bool,tag="9")] pub refresh:bool,
    #[prost(string,tag="10")] pub content_query:String,
    #[prost(string,tag="11")] pub content_after:String,
}
#[derive(Clone,PartialEq,Message)]
pub struct DiscoverFilesHit{
    #[prost(string,tag="1")] pub path:String,
    #[prost(string,tag="2")] pub kind:String,
    #[prost(uint64,tag="3")] pub bytes:u64,
    #[prost(uint64,tag="4")] pub modified_unix:u64,
    #[prost(string,tag="5")] pub matched_by:String,
    #[prost(string,tag="6")] pub excerpt:String,
}
#[derive(Clone,PartialEq,Message)]
pub struct DiscoverFilesResult{
    #[prost(message,repeated,tag="1")] pub hits:Vec<DiscoverFilesHit>,
    #[prost(uint32,tag="2")] pub indexed_entries:u32,
    #[prost(uint32,tag="3")] pub indexed_directories:u32,
    #[prost(bool,tag="4")] pub index_incomplete:bool,
    #[prost(bool,tag="5")] pub results_truncated:bool,
    #[prost(bool,tag="6")] pub content_incomplete:bool,
    #[prost(uint32,tag="7")] pub content_files_examined:u32,
    #[prost(bool,tag="8")] pub index_refreshed:bool,
    #[prost(uint64,tag="9")] pub index_generation:u64,
    #[prost(string,tag="10")] pub next_content_after:String,
}
#[derive(Clone)]
struct Entry{
    path:String,name:String,kind:&'static str,
    bytes:u64,modified_unix:u64,modified_tick:u128,
}
#[derive(Clone)]
struct DirectorySnapshot{
    stamp:u128,children:Vec<Entry>,
}
struct TextCache{
    // Per authorized turn only. Never globally shared or persisted.
    texts:BTreeMap<String,(u128,String)>,bytes:usize,
}
impl TextCache{
    fn new()->Self{Self{texts:BTreeMap::new(),bytes:0}}
    fn get(&self,path:&str,stamp:u128)->Option<&str>{
        self.texts.get(path).and_then(|(at,text)|
            if *at==stamp{Some(text.as_str())}else{None})
    }
    fn insert(&mut self,path:String,stamp:u128,text:String){
        if text.len()>MAX_CONTENT_BYTES as usize{return}
        if self.texts.len()>=MAX_CONTENT_FILES||
           self.bytes.saturating_add(text.len())>MAX_CONTENT_BYTES as usize{
            self.texts.clear();self.bytes=0;
        }
        self.bytes+=text.len();
        self.texts.insert(path,(stamp,text));
    }
}
struct Index{
    directories:BTreeMap<String,DirectorySnapshot>,
    entries:BTreeMap<String,Entry>,
    updated:Option<Instant>,
    generation:u64,complete:bool,
}
impl Index{
    fn new()->Self{Self{
        directories:BTreeMap::new(),entries:BTreeMap::new(),
        updated:None,generation:0,complete:false,
    }}
}
static CACHE:OnceLock<Mutex<BTreeMap<String,Arc<Mutex<Index>>>>>=OnceLock::new();
fn cache_for(root:&Path)->Result<Arc<Mutex<Index>>,String>{
    let key=root.to_str().ok_or("Non-Unicode workspace root is not supported")?;
    let map=CACHE.get_or_init(||Mutex::new(BTreeMap::new()));
    let mut cache=map.lock().map_err(|_|"Discovery cache lock unavailable")?;
    if let Some(found)=cache.get(key){return Ok(Arc::clone(found))}
    if cache.len()>=MAX_CACHE_ROOTS{
        // Bounded metadata-only RAM. Active turns retain their Arc, never an
        // authority token, and are safe even when the map evicts a cache.
        if let Some(first)=cache.keys().next().cloned(){cache.remove(&first);}
    }
    let value=Arc::new(Mutex::new(Index::new()));
    cache.insert(key.to_owned(),Arc::clone(&value));
    Ok(value)
}
fn fail(code:&str)->ToolFailure{ToolFailure::new(code,code,false)}
fn valid_relative(raw:&str,allow_empty:bool)->bool{
    (allow_empty&&raw.is_empty())||
    (!raw.is_empty()&&raw.len()<=4096
        &&!raw.chars().any(|ch|ch.is_control()||matches!(ch,'\\'|':')
            ||matches!(ch,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'))
        &&!Path::new(raw).is_absolute()
        &&raw.split('/').all(|x|!x.is_empty()&&x!="."&&x!=".."))
}
fn valid_query(value:&str,allow_empty:bool)->bool{
    (allow_empty||!value.is_empty())&&value.len()<=128
        &&!value.chars().any(char::is_control)
}
fn valid_suffix(v:&str)->bool{
    v.len()<=64&&v.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'.'|b'_'|b'-'))
}
fn validate(req:&DiscoverFilesRequest)->Result<(),ToolFailure>{
    if !valid_query(&req.query,true)||!valid_query(&req.content_query,true)||
        (req.query.trim().is_empty()&&req.content_query.trim().is_empty()&&
            req.file_suffix.is_empty()&&req.kind.is_empty()&&
            req.directory.is_empty()&&req.min_bytes.is_none()&&
            req.max_bytes.is_none()&&req.modified_since.is_none()){
        return Err(fail("DISCOVERY_INVALID_QUERY"))
    }
    if !valid_relative(&req.directory,true)||
        !valid_relative(&req.content_after,true)||
        (!req.content_after.is_empty()&&req.content_query.is_empty())||
        !valid_suffix(&req.file_suffix)||
        !matches!(req.kind.as_str(),""|"file"|"directory")||
        req.limit.unwrap_or(24)==0||req.limit.unwrap_or(24)>MAX_RETURN as u32||
        req.min_bytes.zip(req.max_bytes).is_some_and(|(a,b)|a>b){
        return Err(fail("DISCOVERY_INVALID_FILTER"))
    }
    Ok(())
}
fn timestamp(meta:&cap_std::fs::Metadata)->(u64,u128){
    let d=meta.modified().ok()
        .and_then(|s|s.into_std().duration_since(UNIX_EPOCH).ok());
    (d.map_or(0,|v|v.as_secs()),d.map_or(0,|v|v.as_nanos()))
}
fn forbidden(meta:&cap_std::fs::Metadata)->bool{
    meta.file_type().is_symlink()||(!meta.is_file()&&!meta.is_dir())
}
fn directory_stamp(root:&Dir,path:&str)->Result<u128,ToolFailure>{
    let loc=if path.is_empty(){"."}else{path};
    let meta=root.symlink_metadata(loc).map_err(|_|fail("DISCOVERY_SCOPE_CHANGED"))?;
    if forbidden(&meta)||!meta.is_dir(){return Err(fail("DISCOVERY_SCOPE_CHANGED"))}
    Ok(timestamp(&meta).1)
}
fn scan(root:&Dir,cache:&mut Index,force:bool)->Result<(),ToolFailure>{
    let started=Instant::now();
    let old=cache.directories.clone();
    let mut directory_rows=BTreeMap::new();
    let mut indexed=BTreeMap::new();
    let mut queue=VecDeque::from([(String::new(),0u8)]);
    let mut complete=true;
    while let Some((dir_name,depth))=queue.pop_front(){
        if started.elapsed()>MAX_DISCOVERY_RUNTIME||
            directory_rows.len()>=MAX_DIRECTORIES{complete=false;break}
        let stamp=match directory_stamp(root,&dir_name){
            Ok(s)=>s,Err(_)=>{complete=false;continue}
        };
        let children=if !force{
            if let Some(old_dir)=old.get(&dir_name){
                if old_dir.stamp==stamp{Some(old_dir.children.clone())}else{None}
            }else{None}
        }else{None};
        let children=if let Some(rows)=children{rows}else{
            let d=match root.open_dir(if dir_name.is_empty(){"."}else{&dir_name}){
                Ok(d)=>d,Err(_)=>{complete=false;continue},
            };
            let mut rows=Vec::new();
            let items=d.read_dir(".").map_err(|_|fail("DISCOVERY_ENUMERATION_FAILED"))?;
            for item in items{
                if started.elapsed()>MAX_DISCOVERY_RUNTIME||
                    indexed.len()+rows.len()>=MAX_ENTRIES{complete=false;break}
                let entry=match item{Ok(e)=>e,Err(_)=>{complete=false;continue}};
                let Some(name)=entry.file_name().to_str().map(str::to_owned)else{
                    complete=false;continue
                };
                // Hidden/system dot entries are deliberately not indexed.
                if name.starts_with('.')||!valid_relative(&name,false){continue}
                let meta=match d.symlink_metadata(&name){
                    Ok(m)=>m,Err(_)=>{complete=false;continue}
                };
                if forbidden(&meta){continue}
                let path=if dir_name.is_empty(){name.clone()}
                    else{format!("{dir_name}/{name}")};
                if !valid_relative(&path,false){continue}
                let (secs,tick)=timestamp(&meta);
                rows.push(Entry{
                    path,name,kind:if meta.is_dir(){"directory"}else{"file"},
                    bytes:if meta.is_file(){meta.len()}else{0},
                    modified_unix:secs,modified_tick:tick,
                });
            }
            rows.sort_by(|a,b|a.path.cmp(&b.path));
            rows
        };
        for item in &children{
            if indexed.len()>=MAX_ENTRIES{complete=false;break}
            if item.kind=="directory"{
                if depth<MAX_DEPTH&&queue.len()+directory_rows.len()<MAX_DIRECTORIES{
                    queue.push_back((item.path.clone(),depth+1));
                }else{complete=false}
            }
            indexed.insert(item.path.clone(),item.clone());
        }
        directory_rows.insert(dir_name,DirectorySnapshot{stamp,children});
        if indexed.len()>=MAX_ENTRIES{complete=false;break}
    }
    if !queue.is_empty(){complete=false}
    cache.directories=directory_rows;
    cache.entries=indexed;
    cache.complete=complete;
    cache.updated=Some(Instant::now());
    cache.generation=cache.generation.saturating_add(1);
    Ok(())
}
fn scope_match(path:&str,directory:&str)->bool{
    directory.is_empty()||path==directory||
        path.strip_prefix(directory).is_some_and(|s|s.starts_with('/'))
}
fn safe_components(root:&Dir,relative:&str)->bool{
    let mut path=String::new();
    let parts=relative.split('/').collect::<Vec<_>>();
    for (index,part) in parts.iter().enumerate(){
        if !path.is_empty(){path.push('/')}
        path.push_str(part);
        let Ok(meta)=root.symlink_metadata(&path)else{return false};
        if forbidden(&meta){return false}
        if index+1<parts.len()&&!meta.is_dir(){return false}
    }
    true
}
fn fresh(root:&Dir,entry:&Entry)->bool{
    if !safe_components(root,&entry.path){return false}
    let Ok(meta)=root.symlink_metadata(&entry.path)else{return false};
    !forbidden(&meta)&&
        (if meta.is_dir(){"directory"}else{"file"})==entry.kind
        &&(entry.kind!="file"||meta.len()==entry.bytes)
        &&timestamp(&meta).1==entry.modified_tick
}
fn find_snippet(text:&str,needle:&str)->Option<String>{
    let lower=text.to_lowercase();
    let at=lower.find(&needle.to_lowercase())?;
    // Index into the *original* string by the Unicode scalar offset; folded
    // Unicode bytes are not guaranteed to align with the original UTF-8.
    let char_at=lower[..at].chars().count();
    let line=text.lines().find(|line|line.to_lowercase().contains(&needle.to_lowercase()));
    if let Some(line)=line{
        return Some(line.chars().filter(|c|!c.is_control()||*c=='\t')
            .take(200).collect())
    }
    Some(text.chars().skip(char_at.saturating_sub(40))
        .filter(|c|!c.is_control()||*c=='\t').take(200).collect())
}
fn perform(root:&Dir,cache:&Mutex<Index>,content:&Mutex<TextCache>,
    req:DiscoverFilesRequest)->Result<DiscoverFilesResult,ToolFailure>{
    validate(&req)?;
    if !req.directory.is_empty(){
        let meta=root.symlink_metadata(&req.directory)
            .map_err(|_|fail("DISCOVERY_INVALID_SCOPE"))?;
        if !meta.is_dir()||!safe_components(root,&req.directory){
            return Err(fail("DISCOVERY_INVALID_SCOPE"))
        }
    }
    let mut guard=cache.lock().map_err(|_|fail("DISCOVERY_CACHE_UNAVAILABLE"))?;
    let rebuild=req.refresh||guard.updated.is_none()||
        guard.updated.is_some_and(|t|t.elapsed()>=REFRESH_AFTER);
    if rebuild{scan(root,&mut guard,req.refresh)?}
    let term=req.query.to_lowercase();
    let mut ranked:Vec<(u8,DiscoverFilesHit)>=Vec::new();
    let mut stale=false;
    let mut examined=0usize;
    let mut content_bytes=0u64;
    let mut content_incomplete=false;
    let mut content_last_scanned=String::new();
    let started=Instant::now();
    for entry in guard.entries.values(){
        if started.elapsed()>MAX_DISCOVERY_RUNTIME{
            stale=true;content_incomplete=!req.content_query.is_empty();
            break
        }
        if !scope_match(&entry.path,&req.directory)||
            (!req.kind.is_empty()&&entry.kind!=req.kind)||
            (!req.file_suffix.is_empty()&&
                !entry.name.to_ascii_lowercase().ends_with(
                    &req.file_suffix.to_ascii_lowercase()))||
            req.min_bytes.is_some_and(|min|entry.bytes<min)||
            req.max_bytes.is_some_and(|max|entry.bytes>max)||
            req.modified_since.is_some_and(|since|entry.modified_unix<since){
            continue
        }
        let name=entry.name.to_lowercase();
        let path=entry.path.to_lowercase();
        let score=if term.is_empty(){1}else if name==term{5}
            else if name.starts_with(&term){4}
            else if name.contains(&term){3}
            else if path.contains(&term){2}else{0};
        if score==0{continue}
        if !fresh(root,entry){stale=true;continue}
        let mut excerpt=String::new();
        if !req.content_query.is_empty(){
            if entry.kind!="file"||(!req.content_after.is_empty()&&
                entry.path<=req.content_after){continue}
            if examined>=MAX_CONTENT_FILES||content_bytes>=MAX_CONTENT_BYTES{
                content_incomplete=true;
                break
            }
            examined+=1;
            content_last_scanned=entry.path.clone();
            let limit=MAX_CONTENT_FILE_BYTES.min(MAX_CONTENT_BYTES-content_bytes);
            if entry.bytes>limit{content_incomplete=true}
            let cached={
                let store=content.lock().map_err(|_|fail("DISCOVERY_CACHE_UNAVAILABLE"))?;
                store.get(&entry.path,entry.modified_tick).map(str::to_owned)
            };
            let text=if let Some(reused)=cached{
                // Cached text is still bounded by this query's byte budget.
                if reused.len() as u64>limit{
                    content_incomplete=true;
                    let take=limit as usize;
                    let boundary=(0..=take).rev()
                        .find(|&n|reused.is_char_boundary(n)).unwrap_or(0);
                    content_bytes+=boundary as u64;
                    reused[..boundary].to_owned()
                }else{
                    content_bytes+=reused.len() as u64;
                    reused
                }
            }else{
                let mut file=match root.open(&entry.path){
                    Ok(f)=>f,Err(_)=>{stale=true;continue}
                };
                if !file.metadata().is_ok_and(|m|m.is_file()
                    &&m.len()==entry.bytes&&timestamp(&m).1==entry.modified_tick){
                    stale=true;continue
                }
                let mut data=Vec::new();
                if file.by_ref().take(limit+1).read_to_end(&mut data).is_err(){
                    stale=true;continue
                }
                if data.len() as u64>limit{
                    data.truncate(limit as usize);content_incomplete=true
                }
                if entry.bytes>data.len() as u64{content_incomplete=true}
                content_bytes+=data.len() as u64;
                let Ok(text)=String::from_utf8(data)else{continue};
                content.lock().map_err(|_|fail("DISCOVERY_CACHE_UNAVAILABLE"))?
                    .insert(entry.path.clone(),entry.modified_tick,text.clone());
                text
            };
            // Cached snippets remain untrusted. A full read needs separate
            // tool authorization and source revalidation.
            if !fresh(root,entry){stale=true;continue}
            let Some(found)=find_snippet(&text,&req.content_query)else{continue};
            excerpt=found;
        }
        ranked.push((score,DiscoverFilesHit{
            path:entry.path.clone(),kind:entry.kind.into(),
            bytes:entry.bytes,modified_unix:entry.modified_unix,
            matched_by:if !req.content_query.is_empty(){"content".into()}
                else if score==5{"exact_name".into()}
                else if score>=3{"filename".into()}
                else if score==2{"path".into()}else{"filter".into()},
            excerpt,
        }));
    }
    ranked.sort_by(|(sa,a),(sb,b)|sb.cmp(sa)
        .then_with(||if term.is_empty(){
            b.modified_unix.cmp(&a.modified_unix)
        }else{std::cmp::Ordering::Equal})
        .then_with(||a.path.cmp(&b.path)));
    let max=req.limit.unwrap_or(24) as usize;
    let overflow=ranked.len()>max;
    let hits=ranked.into_iter().take(max).map(|(_,h)|h).collect();
    Ok(DiscoverFilesResult{
        hits,indexed_entries:guard.entries.len() as u32,
        indexed_directories:guard.directories.len() as u32,
        index_incomplete:!guard.complete||stale,
        results_truncated:overflow,
        content_incomplete,content_files_examined:examined as u32,
        index_refreshed:rebuild,index_generation:guard.generation,
        next_content_after:if content_incomplete{content_last_scanned}
            else{String::new()},
    })
}
struct DiscoveryTool{
    descriptor:ToolDescriptor,
    root:Arc<Dir>,cache:Arc<Mutex<Index>>,
    content:Arc<Mutex<TextCache>>,
}
impl NativeTool for DiscoveryTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let root=Arc::clone(&self.root);
        let cache=Arc::clone(&self.cache);
        let content=Arc::clone(&self.content);
        Box::pin(async move{
            if call.tool_id!=ID||call.input.type_url!=INPUT
                ||call.input.bytes.len()>4096{
                return Err(fail("DISCOVERY_INVALID_ENVELOPE"))
            }
            let request=DiscoverFilesRequest::decode(call.input.bytes.as_slice())
                .map_err(|_|fail("DISCOVERY_BAD_PROTOBUF"))?;
            if request.encode_to_vec()!=call.input.bytes{
                return Err(fail("DISCOVERY_NONCANONICAL_PROTOBUF"))
            }
            let data=tokio::task::spawn_blocking(move||
                perform(&root,&cache,&content,request))
                .await.map_err(|_|fail("DISCOVERY_WORKER_FAILED"))??;
            Ok(ToolOutput{payload:ProtoPayload{type_url:OUTPUT.into(),
                bytes:data.encode_to_vec()}})
        })
    }
}
struct DiscoveryCodec;
impl ModelToolCodec for DiscoveryCodec{
    fn input_type_url(&self)->&'static str{INPUT}
    fn output_type_url(&self)->&'static str{OUTPUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["query","directory","file_suffix","kind",
            "min_bytes","max_bytes","modified_since","limit","refresh",
            "content_query","content_after"],&[])?;
        let string=|key:&str|->Result<String,ModelToolError>{
            match args.get(key){
                None=>Ok(String::new()),
                Some(Value::String(v))=>Ok(v.clone()),
                _=>Err(ModelToolError::InvalidArguments(format!("{key} must be text"))),
            }
        };
        let number=|key:&str|->Result<Option<u64>,ModelToolError>{
            match args.get(key){
                None=>Ok(None),Some(v)=>v.as_u64().map(Some).ok_or_else(||
                    ModelToolError::InvalidArguments(format!("{key} must be a nonnegative integer"))),
            }
        };
        let refresh=match args.get("refresh"){
            None=>false,Some(Value::Bool(v))=>*v,
            _=>return Err(ModelToolError::InvalidArguments("refresh must be boolean".into()))
        };
        let limit=match number("limit")?{
            None=>Some(24),
            Some(n) if n>=1&&n<=MAX_RETURN as u64=>Some(n as u32),
            _=>return Err(ModelToolError::InvalidArguments("limit must be 1–80".into())),
        };
        let req=DiscoverFilesRequest{
            query:string("query")?,directory:string("directory")?,
            file_suffix:string("file_suffix")?,kind:string("kind")?,
            min_bytes:number("min_bytes")?,max_bytes:number("max_bytes")?,
            modified_since:number("modified_since")?,limit,refresh,
            content_query:string("content_query")?,
            content_after:string("content_after")?,
        };
        validate(&req).map_err(|e|ModelToolError::InvalidArguments(e.message))?;
        Ok(ProtoPayload{type_url:INPUT.into(),bytes:req.encode_to_vec()})
    }
    fn decode(&self,result:&ProtoPayload)->Result<Value,ModelToolError>{
        if result.type_url!=OUTPUT{
            return Err(ModelToolError::InvalidResult("Incorrect discovery result type".into()))
        }
        let data=DiscoverFilesResult::decode(result.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("Malformed discovery result".into()))?;
        Ok(json!({
            "hits":data.hits.into_iter().map(|h|json!({
                "path":h.path,"kind":h.kind,"bytes":h.bytes,
                "modified_unix":h.modified_unix,"matched_by":h.matched_by,
                "excerpt":h.excerpt,
                "freshness":"metadata_rechecked_not_content_hash_verified"
            })).collect::<Vec<_>>(),
            "indexed_entries":data.indexed_entries,
            "indexed_directories":data.indexed_directories,
            "index_incomplete":data.index_incomplete,
            "results_truncated":data.results_truncated,
            "content_incomplete":data.content_incomplete,
            "next_content_after":data.next_content_after,
            "content_files_examined":data.content_files_examined,
            "index_refreshed":data.index_refreshed,
            "index_generation":data.index_generation,
        }))
    }
}
/// Register only within a current operator-picked, authenticated read-only
/// workspace turn. A cache entry is never a grant and never contains contents.
pub fn register_discovery_tool(
    registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,
    approved_root:&Path
)->Result<(),String>{
    let meta=std::fs::symlink_metadata(approved_root)
        .map_err(|_|"Workspace root is unavailable")?;
    if meta.file_type().is_symlink()||!meta.is_dir(){
        return Err("Workspace root must be a real directory".into())
    }
    let canonical=approved_root.canonicalize()
        .map_err(|_|"Cannot canonicalize workspace root")?;
    let root=Arc::new(Dir::open_ambient_dir(&canonical,ambient_authority())
        .map_err(|_|"Cannot open authorized workspace root")?);
    let cache=cache_for(&canonical)?;
    let content=Arc::new(Mutex::new(TextCache::new()));
    let descriptor=ToolDescriptor{
        id:ID.into(),display_name:"Find workspace files by name, metadata or bounded text".into(),
        effect:ToolEffect::ReadWorkspace,
        input_type_url:INPUT.into(),output_type_url:OUTPUT.into(),
    };
    registry.register(ModuleManifest{
        id:"lu.file_discovery".into(),display_name:"Lu indexed workspace discovery".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![descriptor.clone()],panels:vec![],workflows:vec![],
    },vec![Arc::new(DiscoveryTool{descriptor,root,cache,content})])
        .map_err(|e|format!("Cannot register discovery tool: {e:?}"))?;
    catalog.insert(registry,ModelToolDefinition{
        name:"lu_files_discover".into(),native_tool_id:ID.into(),
        description:"Locate files and folders in the human-selected workspace by filename, partial path, suffix, type, size or modification date, using a reusable bounded read-only metadata index. Optional literal content_query scans up to 96 small files/4 MiB and returns untrusted excerpts. Set refresh=true to force reconciliation. Hidden files and symlinks excluded. A discovered path is NOT permission to open or modify it.".into(),
        parameters:json!({"type":"object","properties":{
            "query":{"type":"string","maxLength":128,"description":"Optional name/path substring; omit for filter-only or content searches"},
            "directory":{"type":"string","description":"Optional relative subtree"},
            "file_suffix":{"type":"string","maxLength":64},
            "kind":{"type":"string","enum":["","file","directory"]},
            "min_bytes":{"type":"integer","minimum":0},
            "max_bytes":{"type":"integer","minimum":0},
            "modified_since":{"type":"integer","minimum":0,"description":"Unix seconds"},
            "limit":{"type":"integer","minimum":1,"maximum":80},
            "refresh":{"type":"boolean","description":"Force a fresh directory walk, bypassing 15-second metadata cache"},
            "content_query":{"type":"string","maxLength":128,"description":"Optional literal UTF-8 content search, bounded and opt-in"},
            "content_after":{"type":"string","maxLength":4096,"description":"For continued content search only, the next_content_after path from the previous partial result"}
        },"additionalProperties":false}),
        codec:Arc::new(DiscoveryCodec),
    }).map_err(|e|format!("Cannot publish discovery schema: {e:?}"))?;
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    fn fixture()->(PathBuf,Dir){
        let root=std::env::temp_dir().join(format!(
            "lu-discovery-test-{}-{:?}",std::process::id(),std::thread::current().id()
        ));
        let _=std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("notes").join("Vulkan-Guide.txt"),
            "The AMD Vulkan memory budget matters.\n").unwrap();
        std::fs::write(root.join("notes").join("neutral.md"),"archive status").unwrap();
        std::fs::write(root.join(".private.txt"),"do not index").unwrap();
        let dir=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
        (root,dir)
    }
    fn query(text:&str)->DiscoverFilesRequest{DiscoverFilesRequest{
        query:text.into(),directory:String::new(),file_suffix:String::new(),
        kind:String::new(),min_bytes:None,max_bytes:None,modified_since:None,
        limit:Some(24),refresh:false,content_query:String::new(),
        content_after:String::new(),
    }}
    #[test]
    fn reuses_metadata_index_but_rechecks_live_target(){
        let (root,dir)=fixture();
        let cache=Mutex::new(Index::new());
        let content=Mutex::new(TextCache::new());
        let first=perform(&dir,&cache,&content,query("vulkan")).unwrap();
        assert_eq!(first.hits.len(),1);
        assert!(first.index_refreshed);
        assert_eq!(first.hits[0].path,"notes/Vulkan-Guide.txt");
        let second=perform(&dir,&cache,&content,query("vulkan")).unwrap();
        assert!(!second.index_refreshed);
        assert_eq!(second.index_generation,first.index_generation);
        std::fs::remove_file(root.join("notes").join("Vulkan-Guide.txt")).unwrap();
        let stale=perform(&dir,&cache,&content,query("vulkan")).unwrap();
        assert!(stale.hits.is_empty());
        assert!(stale.index_incomplete);
        let mut fresh=query("neutral");fresh.refresh=true;
        let rebuilt=perform(&dir,&cache,&content,fresh).unwrap();
        assert_eq!(rebuilt.hits.len(),1);
        assert!(rebuilt.index_refreshed);
        drop(dir);
        drop(cache);
        drop(content);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn content_search_is_explicit_bounded_and_filename_independent(){
        let (root,dir)=fixture();
        let cache=Mutex::new(Index::new());
        let content=Mutex::new(TextCache::new());
        let mut q=query("");
        q.content_query="memory budget".into();
        let result=perform(&dir,&cache,&content,q).unwrap();
        assert_eq!(result.hits.len(),1);
        assert_eq!(result.hits[0].matched_by,"content");
        assert!(result.hits[0].excerpt.contains("Vulkan"));
        assert_eq!(result.content_files_examined,2);
        drop(dir);
        drop(cache);
        drop(content);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_escape_filter_and_hidden_entries(){
        let (root,dir)=fixture();
        let cache=Mutex::new(Index::new());
        let content=Mutex::new(TextCache::new());
        let result=perform(&dir,&cache,&content,query("private")).unwrap();
        assert!(result.hits.is_empty());
        for invalid in ["../secret","a\\b","/absolute","a//b","a:b"]{
            let mut q=query("notes");q.directory=invalid.into();
            assert!(perform(&dir,&cache,&content,q).is_err());
        }
        drop(dir);
        drop(cache);
        drop(content);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn changed_nested_directory_is_detected_on_incremental_reconcile(){
        let (root,dir)=fixture();
        let cache=Mutex::new(Index::new());
        let content=Mutex::new(TextCache::new());
        let initial=perform(&dir,&cache,&content,query("new-page")).unwrap();
        assert!(initial.hits.is_empty());
        std::fs::write(root.join("notes/new-page.md"),"new content").unwrap();
        let mut check=query("new-page");check.refresh=true;
        let found=perform(&dir,&cache,&content,check).unwrap();
        assert_eq!(found.hits.len(),1);
        assert!(found.index_generation>initial.index_generation);
        drop(dir);drop(cache);drop(content);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounded_content_search_can_continue_after_a_scanned_path(){
        let (root,dir)=fixture();
        let cache=Mutex::new(Index::new());
        let content=Mutex::new(TextCache::new());
        let mut q=query("");q.content_query="archive".into();
        q.content_after="notes/Vulkan-Guide.txt".into();
        let result=perform(&dir,&cache,&content,q).unwrap();
        assert_eq!(result.hits.len(),1);
        assert_eq!(result.hits[0].path,"notes/neutral.md");
        drop(dir);drop(cache);drop(content);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn renamed_files_are_reconciled_without_a_path_grant(){
        let (root,dir)=fixture();
        let cache=Mutex::new(Index::new());
        let content=Mutex::new(TextCache::new());
        let before=perform(&dir,&cache,&content,query("neutral")).unwrap();
        assert_eq!(before.hits.len(),1);
        std::fs::rename(root.join("notes/neutral.md"),
            root.join("notes/renamed.md")).unwrap();
        let old=perform(&dir,&cache,&content,query("neutral")).unwrap();
        assert!(old.hits.is_empty());
        assert!(old.index_incomplete);
        let mut new=query("renamed");new.refresh=true;
        let found=perform(&dir,&cache,&content,new).unwrap();
        assert_eq!(found.hits.len(),1);
        assert_eq!(found.hits[0].path,"notes/renamed.md");
        drop(dir);drop(cache);drop(content);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn text_cache_is_turn_local_and_never_serves_missing_files(){
        let (root,dir)=fixture();
        let cache=Mutex::new(Index::new());
        let content=Mutex::new(TextCache::new());
        let mut text=query("");text.content_query="Vulkan".into();
        let first=perform(&dir,&cache,&content,text.clone()).unwrap();
        assert_eq!(first.hits.len(),1);
        assert_eq!(content.lock().unwrap().texts.len(),2);
        let second=perform(&dir,&cache,&content,text.clone()).unwrap();
        assert_eq!(second.hits.len(),1);
        assert!(!second.index_refreshed);
        std::fs::remove_file(root.join("notes/Vulkan-Guide.txt")).unwrap();
        let gone=perform(&dir,&cache,&content,text).unwrap();
        assert!(gone.hits.is_empty());
        assert!(gone.index_incomplete);
        drop(dir);drop(cache);drop(content);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_never_indexed_or_searched(){
        let (root,dir)=fixture();
        let outside=root.with_extension("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"),"invisible payload").unwrap();
        std::os::unix::fs::symlink(&outside,root.join("linked")).unwrap();
        let cache=Mutex::new(Index::new());
        let content=Mutex::new(TextCache::new());
        let result=perform(&dir,&cache,&content,query("secret")).unwrap();
        assert!(result.hits.is_empty());
        let mut req=query("");req.content_query="invisible payload".into();
        assert!(perform(&dir,&cache,&content,req).unwrap().hits.is_empty());
        drop(dir);drop(cache);drop(content);
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(outside).unwrap();
    }
}
