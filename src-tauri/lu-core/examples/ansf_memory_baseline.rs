//! Headless *real Lu Core* ANSF book seed / indexed-lookup adapter.
//!
//! The executable speaks JSON ONLY at its external test-process boundary.
//! It is not Lu's internal IPC contract, and is not an installed desktop app.
//! The trusted host enforces the single benchmark workspace/known book root.
use lu_core::{
    ansf::{AnsfStore,EntryRole},
    ansf_library::{AnsfLibrary,BookIndex,ChapterIndex},
    ansf_source_ids::{AnsfSourceId,DirectReadError,HostSourceReadAccess,resolve_known_source},
};
use serde::{Deserialize,Serialize};
use std::{collections::BTreeMap,env,fs,path::{Path,PathBuf}};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture { sessions:Vec<Session> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Session { id:String,title:String,chapters:Vec<Chapter> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Chapter { title:String,entries:Vec<Source> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source { id:String,role:String,summary:String,text:String }
#[derive(Serialize)]
struct Prepared { source_ids:BTreeMap<String,String> }
struct BenchAccess;
impl HostSourceReadAccess for BenchAccess{
    fn may_read(&self,active:&str,source:&str,workspace:&str)->bool{
        active.starts_with("c-")&&source.starts_with("c-")&&workspace=="ansf-benchmark"
    }
}
fn valid_name(name:&str)->bool{
    !name.is_empty()&&name.len()<=96&&
    name.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn must(condition:bool,description:&str)->Result<(),String>{
    if condition{Ok(())}else{Err(description.into())}
}
fn book_file(root:&Path,session:&str)->Result<PathBuf,String>{
    must(valid_name(session),"invalid fixture conversation ID")?;
    Ok(root.join("books").join(format!("{session}.sqlite")))
}
fn prepare(root:&Path,fixture_path:&Path)->Result<(),String>{
    must(!root.exists(),"benchmark root already exists: fresh isolated books required")?;
    let input=fs::read(fixture_path).map_err(|e|e.to_string())?;
    must(input.len()<1_000_000,"unbounded fixture")?;
    let dataset:Fixture=serde_json::from_slice(&input).map_err(|e|e.to_string())?;
    must(!dataset.sessions.is_empty()&&dataset.sessions.len()<=16,"invalid session count")?;
    fs::create_dir_all(root.join("books")).map_err(|e|e.to_string())?;
    let lib=AnsfLibrary::open(&root.join("library.db")).map_err(|e|format!("{e:?}"))?;
    let mut known=BTreeMap::new();
    for session in dataset.sessions{
        must(valid_name(&session.id),"invalid conversation ID")?;
        must(!session.chapters.is_empty()&&session.chapters.len()<=16,"invalid chapter count")?;
        let book=AnsfStore::open(&book_file(root,&session.id)?)
            .map_err(|e|format!("{e:?}"))?;
        book.create_session(&session.id,"ansf-benchmark",&session.title)
            .map_err(|e|format!("{e:?}"))?;
        lib.index_book(&BookIndex{
            session_id:session.id.clone(),workspace_id:"ansf-benchmark".into(),
            relative_path:format!("books/{}.sqlite",session.id),
            title:session.title.clone(),summary:"Benchmark-specific memory source".into(),
        }).map_err(|e|format!("{e:?}"))?;
        for chapter in session.chapters{
            must(!chapter.entries.is_empty()&&chapter.entries.len()<=96,"invalid entry count")?;
            let id=book.start_chapter(&session.id,&chapter.title)
                .map_err(|e|format!("{e:?}"))?;
            for entry in chapter.entries{
                must(valid_name(&entry.id),"invalid source message ID")?;
                let role=match entry.role.as_str(){
                    "user"=>EntryRole::User,
                    "assistant"=>EntryRole::Assistant,
                    "tool_call"=>EntryRole::ToolCall,
                    "tool_result"=>EntryRole::ToolResult,
                    _=>return Err("invalid source role".into()),
                };
                let entry_id=book.append_entry(&session.id,&entry.id,role,
                    &entry.summary,&entry.text).map_err(|e|format!("{e:?}"))?;
                let stable=AnsfSourceId::new(&session.id,entry_id)
                    .map_err(|e|format!("{e:?}"))?.encode();
                must(known.insert(format!("{}/{}",session.id,entry.id),stable).is_none(),
                    "duplicate source ID")?;
            }
            let chapter_row=book.chapters(&session.id).map_err(|e|format!("{e:?}"))?
                .into_iter().find(|row|row.id==id).ok_or("lost chapter")?;
            lib.index_chapter(&ChapterIndex{
                session_id:session.id.clone(),chapter_id:id,
                title:chapter_row.title,summary:"Indexed original conversation".into(),
                start_entry_id:chapter_row.start_entry_id,end_entry_id:chapter_row.end_entry_id,
            }).map_err(|e|format!("{e:?}"))?;
            book.close_chapter(&session.id,"Completed indexed conversation chapter")
                .map_err(|e|format!("{e:?}"))?;
        }
    }
    println!("{}",serde_json::to_string(&Prepared{source_ids:known}).map_err(|e|e.to_string())?);
    Ok(())
}
fn allowed_book_opener(root:&Path,book:&BookIndex)->Result<AnsfStore,DirectReadError>{
    if !valid_name(&book.session_id)||book.workspace_id!="ansf-benchmark"||
       book.relative_path!=format!("books/{}.sqlite",book.session_id){
        return Err(DirectReadError::OpenUnavailable)
    }
    let canonical_root=fs::canonicalize(root.join("books"))
        .map_err(|_|DirectReadError::OpenUnavailable)?;
    let target=book_file(root,&book.session_id)
        .map_err(|_|DirectReadError::OpenUnavailable)?;
    let m=fs::symlink_metadata(&target).map_err(|_|DirectReadError::OpenUnavailable)?;
    if !m.is_file()||m.file_type().is_symlink(){
        return Err(DirectReadError::OpenUnavailable)
    }
    let canonical_file=fs::canonicalize(&target)
        .map_err(|_|DirectReadError::OpenUnavailable)?;
    if !canonical_file.starts_with(&canonical_root){
        return Err(DirectReadError::OpenUnavailable)
    }
    AnsfStore::open(&target).map_err(|_|DirectReadError::OpenUnavailable)
}
fn read(root:&Path,active:&str,source_id:&str)->Result<(),String>{
    let lib=AnsfLibrary::open(&root.join("library.db")).map_err(|e|format!("{e:?}"))?;
    let data=resolve_known_source(source_id,active,&lib,&BenchAccess,
        |book|allowed_book_opener(root,book),8192);
    match data {
        Ok(found)=>println!("{}",serde_json::json!({
            "status":"found","source_id":found.source_id,
            "source_session":found.source_session_id,
            "source_message_id":found.source_message_id,
            "source_role":found.source_role,"summary":found.summary,
            "content":found.content
        })),
        Err(DirectReadError::EntryMissing|DirectReadError::NotIndexed)=>{
            println!("{}",serde_json::json!({"status":"not_found","source_id":source_id}));
        },
        Err(e)=>return Err(format!("source lookup failed closed: {e:?}")),
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmbeddingBatch {
    model_id:String,
    entries:Vec<EmbeddingSource>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmbeddingSource { source_id:String, vector:Vec<f32> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscoverQuery {
    active_chat:String,
    model_id:String,
    question:String,
    sessions:Vec<String>,
    embedding:Vec<f32>,
    limit:usize,
}
fn input_json<T:serde::de::DeserializeOwned>(path:&Path)->Result<T,String>{
    let bytes=fs::read(path).map_err(|e|e.to_string())?;
    must(!bytes.is_empty()&&bytes.len()<=8*1024*1024,"invalid bounded semantic input")?;
    serde_json::from_slice(&bytes).map_err(|e|e.to_string())
}
/// Model embeddings are locally computed by the benchmark's independently
/// SHA-pinned llama-server; Lu Core writes only derived index vectors.
fn index_embeddings(root:&Path,request:&Path)->Result<(),String>{
    let batch:EmbeddingBatch=input_json(request)?;
    must(batch.model_id.len()>2&&batch.entries.len()<=512&&
        !batch.entries.is_empty(),"invalid embedding index batch")?;
    let lib=AnsfLibrary::open(&root.join("library.db"))
        .map_err(|e|format!("{e:?}"))?;
    let mut count=0;
    for row in batch.entries{
        let source=AnsfSourceId::parse(&row.source_id)
            .map_err(|e|format!("{e:?}"))?;
        let record=resolve_known_source(
            &row.source_id,"c-active",&lib,&BenchAccess,
            |book|allowed_book_opener(root,book),8192
        ).map_err(|e|format!("untrusted original index rejected: {e:?}"))?;
        must(record.source_role=="user"||record.source_role=="assistant",
            "ineligible historical role")?;
        let book=lib.get_book(source.session_id())
            .map_err(|e|format!("{e:?}"))?.ok_or("missing authorized book")?;
        let db=allowed_book_opener(root,&book).map_err(|e|format!("{e:?}"))?;
        db.index_semantic_passage(source.session_id(),source.entry_id(),
            0,&batch.model_id,&row.vector)
            .map_err(|e|format!("semantic projection rejected: {e:?}"))?;
        count+=1;
    }
    println!("{}",serde_json::json!({"indexed":count,"model_id":batch.model_id}));
    Ok(())
}
/// Host-scoped native discovery across ALL authorized books. Neither question
/// nor runtime receives an oracle source ID, expected answer or gold document.
/// Strict source re-opening applies after FTS5/vector rank fusion.
fn discover_sources(root:&Path,request:&Path)->Result<(),String>{
    use lu_core::ansf_semantic::fuse_source_ranks;
    let q:DiscoverQuery=input_json(request)?;
    must(valid_name(&q.active_chat)&&q.active_chat=="c-active"&&
        q.model_id.len()>=3&&q.question.len()<=512&&
        !q.question.trim().is_empty()&&
        q.sessions.len()<=16&&!q.sessions.is_empty()&&
        (1..=5).contains(&q.limit),"invalid bounded semantic discovery")?;
    let lib=AnsfLibrary::open(&root.join("library.db"))
        .map_err(|e|format!("{e:?}"))?;
    let words=q.question.split(|c:char|!c.is_ascii_alphanumeric())
        .map(|w|w.to_ascii_lowercase())
        .filter(|w|w.len()>=4&&!matches!(w.as_str(),
            "what"|"which"|"does"|"when"|"many"|"from"|"with"|"were"|
            "their"|"about"|"into"|"that"|"this"|"have"|"been"|"answer"))
        .take(8).collect::<Vec<_>>();
    let fts=words.join(" OR ");
        .collect::<Vec<_>>().join(" OR ");
    let mut lexical=Vec::new();
    let mut semantic=Vec::new();
    let mut seen=std::collections::HashSet::new();
    for session in &q.sessions{
        must(valid_name(session)&&session.starts_with("c-")&&seen.insert(session),
            "duplicate or invalid source book")?;
        let book=lib.get_book(session).map_err(|e|format!("{e:?}"))?
            .ok_or("missing original book")?;
        must(BenchAccess.may_read(&q.active_chat,session,&book.workspace_id),
            "unauthorized history source")?;
        let db=allowed_book_opener(root,&book).map_err(|e|format!("{e:?}"))?;
        if !fts.is_empty(){
            for e in db.search(session,&fts,8).map_err(|e|format!("{e:?}"))?{
                if matches!(e.role.as_str(),"user"|"assistant"){
                    lexical.push(e.stable_source_id().map_err(|e|format!("{e:?}"))?);
                }
            }
        }
        for hit in db.semantic_candidates(session,&q.model_id,&q.embedding,8)
            .map_err(|e|format!("{e:?}"))?{
            semantic.push((
                hit.entry.stable_source_id().map_err(|e|format!("{e:?}"))?,
                hit.similarity
            ));
        }
    }
    semantic.sort_by(|a,b|b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let semantic_ids=semantic.into_iter().map(|x|x.0).collect::<Vec<_>>();
    let ranked=fuse_source_ranks(&lexical,&semantic_ids,q.limit)
        .map_err(|e|format!("{e:?}"))?;
    let mut result=Vec::new();
    for rank in ranked{
        let hit=resolve_known_source(
            &rank.source_id,&q.active_chat,&lib,&BenchAccess,
            |book|allowed_book_opener(root,book),8192
        ).map_err(|e|format!("ranked original not verified: {e:?}"))?;
        must(hit.source_role=="user"||hit.source_role=="assistant",
            "tool source cannot be selected")?;
        result.push(serde_json::json!({
            "source_id":hit.source_id,"source_session":hit.source_session_id,
            "source_message_id":hit.source_message_id,
            "content":hit.content,"rank_score":rank.score
        }));
    }
    println!("{}",serde_json::json!({
        "status":"found","results":result,"lexical_candidates":lexical.len(),
        "semantic_candidates":semantic_ids.len(),
    }));
    Ok(())
}

fn run()->Result<(),String>{
    let a:Vec<String>=env::args().collect();
    match a.as_slice(){
        [_,cmd,root,file] if cmd=="prepare"=>prepare(Path::new(root),Path::new(file)),
        [_,cmd,root,active,source] if cmd=="read"=>
            read(Path::new(root),active,source),
        [_,cmd,root,input] if cmd=="index"=>
            index_embeddings(Path::new(root),Path::new(input)),
        [_,cmd,root,input] if cmd=="discover"=>
            discover_sources(Path::new(root),Path::new(input)),
        _=>Err("usage: ansf_memory_baseline prepare <root> <fixture.json> | read <root> <active-chat> <source-id>".into())
    }
}
fn main(){if let Err(e)=run(){eprintln!("LU_ANSF_BENCHMARK_ERROR: {e}");std::process::exit(2)}}
