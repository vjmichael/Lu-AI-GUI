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
fn run()->Result<(),String>{
    let a:Vec<String>=env::args().collect();
    match a.as_slice(){
        [_,cmd,root,file] if cmd=="prepare"=>prepare(Path::new(root),Path::new(file)),
        [_,cmd,root,active,source] if cmd=="read"=>
            read(Path::new(root),active,source),
        _=>Err("usage: ansf_memory_baseline prepare <root> <fixture.json> | read <root> <active-chat> <source-id>".into())
    }
}
fn main(){if let Err(e)=run(){eprintln!("LU_ANSF_BENCHMARK_ERROR: {e}");std::process::exit(2)}}
