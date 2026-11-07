//! M2 staging importer: legacy Lu JSONL -> real ANSF book SQLite.
//!
//! This runs only under the trusted desktop owner's exclusive data lease.
//! JSON is a *legacy import boundary*, never the new canonical event format.
//! The original sources are never removed or modified. They do not grant tools.
//!
//! Deliberately not wired to ordinary startup yet: the final cutover requires
//! a transactional SQLite chat/folder catalog and host verifier switch.
use lu_core::{
    ansf::AnsfStore,
    ansf_goals::GoalEvent,
    ansf_library::{AnsfLibrary,BookIndex,ChapterIndex},
    ansf_live_chat::{LiveEvent,LiveKind},
};
use serde::Deserialize;
use sha2::{Digest,Sha256};
use std::{fs::{self,File},io::{BufRead,BufReader},path::{Path,PathBuf}};

const MAX_LEGACY_INDEX:u64=512*1024;
const MAX_LEGACY_CHAT:u64=32*1024*1024;
const MAX_LEGACY_LINE:usize=2*1024*1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyIndex{
    version:u32,
    sequence:u64,
    folders:Vec<LegacyFolder>,
    chats:Vec<LegacyChat>,
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct LegacyFolder{folder_id:String,name:String}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct LegacyChat{
    chat_id:String,title:String,folder_id:Option<String>,favorite:bool,
    updated_seq:u64,
}
#[derive(Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
enum LegacyEvent{
    User{text:String,#[serde(default)]user_turn_id:Option<String>},
    ReaderUser{text:String,generation_id:String},
    AssistantFinal{text:String},
    AssistantProgress{generation_id:String,text:String},
    AssistantCompleted{generation_id:String},
    AssistantCancelled{generation_id:String},
    BrowserObservation{generation_id:String,call_id:String,tool:String,summary:String},
    GoalMemory{entry:GoalEvent},
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ImportReceipt{
    pub session_id:String,
    pub imported_chat_events:usize,
    pub imported_goal_events:usize,
    pub last_source_seq:u64,
}
fn valid_id(value:&str,prefix:&str)->bool{
    value.len()==prefix.len()+32&&value.starts_with(prefix)&&
        value[prefix.len()..].bytes().all(|x|x.is_ascii_hexdigit()&&!x.is_ascii_uppercase())
}
fn regular_or_missing(path:&Path)->Result<(),String>{
    match fs::symlink_metadata(path){
        Ok(meta) if meta.file_type().is_symlink()||!meta.is_file()=>
            Err(format!("Unsafe legacy source or destination: {}",path.display())),
        Ok(_)=>Ok(()),
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),
        Err(e)=>Err(e.to_string()),
    }
}
fn regular_required(path:&Path,limit:u64)->Result<(),String>{
    let meta=fs::symlink_metadata(path).map_err(|e|e.to_string())?;
    if meta.file_type().is_symlink()||!meta.is_file()||meta.len()>limit{
        return Err("Legacy journal unavailable, unsafe or outside size gate".into())
    }
    Ok(())
}
fn event_identity(session:&str,seq:u64)->String{
    let mut sha=Sha256::new();
    sha.update(b"lu-legacy-ansf-event-v1");
    sha.update(session.as_bytes());
    sha.update(seq.to_be_bytes());
    let digest=sha.finalize();
    format!("e-{}",digest[..16].iter().map(|v|format!("{v:02x}")).collect::<String>())
}
fn type_event(record:LegacyEvent,event_id:String)->Result<Option<LiveEvent>,String>{
    let mut e=LiveEvent{
        event_id,kind:LiveKind::User,text:String::new(),
        user_turn_id:None,legacy_turn_id:None,generation_id:None,call_id:None,tool_id:None,
    };
    match record{
        LegacyEvent::User{text,user_turn_id}=>{
            e.kind=LiveKind::User;e.text=text;e.legacy_turn_id=user_turn_id;
            // The imported g-* is historical metadata in the untouched
            // legacy file, never a new host-authenticated human grant.
        }
        LegacyEvent::ReaderUser{text,generation_id}=>{
            e.kind=LiveKind::ReaderUser;e.text=text;e.generation_id=Some(generation_id);
        }
        LegacyEvent::AssistantFinal{text}=>{
            e.kind=LiveKind::AssistantFinal;e.text=text;
        }
        LegacyEvent::AssistantProgress{generation_id,text}=>{
            e.kind=LiveKind::AssistantProgress;e.generation_id=Some(generation_id);
            e.text=text;
        }
        LegacyEvent::AssistantCompleted{generation_id}=>{
            e.kind=LiveKind::AssistantCompleted;e.generation_id=Some(generation_id);
        }
        LegacyEvent::AssistantCancelled{generation_id}=>{
            e.kind=LiveKind::AssistantCancelled;e.generation_id=Some(generation_id);
        }
        LegacyEvent::BrowserObservation{generation_id,call_id,tool,summary}=>{
            e.kind=LiveKind::BrowserObservation;e.generation_id=Some(generation_id);
            e.call_id=Some(call_id);e.tool_id=Some(tool);e.text=summary;
        }
        LegacyEvent::GoalMemory{..}=>return Ok(None),
    }
    Ok(Some(e))
}
fn import_one(root:&Path,library:&AnsfLibrary,book_root:&Path,chat:&LegacyChat)
    ->Result<ImportReceipt,String>{
    if !valid_id(&chat.chat_id,"c-")||chat.title.trim().is_empty()||
       chat.title.len()>2048{
        return Err("Invalid legacy conversation metadata".into())
    }
    let journal=root.join("conversations").join(format!("{}.jsonl",chat.chat_id));
    regular_required(&journal,MAX_LEGACY_CHAT)?;
    let dest=book_root.join(format!("{}.sqlite",chat.chat_id));
    regular_or_missing(&dest)?;
    let book=AnsfStore::open(&dest).map_err(|e|format!("{e:?}"))?;
    book.ensure_session(&chat.chat_id,"lu-desktop-local-profile",&chat.title)
        .map_err(|e|format!("{e:?}"))?;
    let chapters=book.chapters(&chat.chat_id).map_err(|e|format!("{e:?}"))?;
    if chapters.is_empty(){
        book.start_chapter(&chat.chat_id,"Migrated conversation")
            .map_err(|e|format!("{e:?}"))?;
    }else if !chapters.iter().any(|ch|!ch.closed){
        return Err("Existing book has no active import chapter".into())
    }
    let mut n=0usize;
    let mut goals=0usize;
    let input=BufReader::new(File::open(&journal).map_err(|e|e.to_string())?);
    for (line_no,line) in input.lines().enumerate(){
        let raw=line.map_err(|e|e.to_string())?;
        if raw.len()>MAX_LEGACY_LINE{
            return Err(format!("Oversized legacy line {}",line_no+1))
        }
        if raw.trim().is_empty(){
            return Err("Blank legacy event cannot be silently discarded".into())
        }
        let parsed:LegacyEvent=serde_json::from_str(&raw)
            .map_err(|e|format!("Legacy chat {} line {}: {e}",chat.chat_id,line_no+1))?;
        match parsed{
            LegacyEvent::GoalMemory{entry}=>{
                book.import_observed_goal_record(&chat.chat_id,&entry)
                    .map_err(|e|format!("Legacy goal transition invalid: {e:?}"))?;
                goals+=1;
            }
            other=>{
                let event=type_event(other,event_identity(&chat.chat_id,(line_no+1) as u64))?
                    .ok_or("Lost original event")?;
                book.import_legacy_chat_event(&chat.chat_id,&event)
                    .map_err(|e|format!("Legacy event conflict at line {}: {e:?}",line_no+1))?;
                n+=1;
            }
        }
    }
    // Detect source truncation: an older, longer previously indexed history
    // cannot be silently rewritten by a shorter new snapshot.
    let mut after=0i64;
    let mut stored=0usize;
    loop{
        let page=book.live_chat_events_after(&chat.chat_id,after,100)
            .map_err(|e|format!("{e:?}"))?;
        if page.is_empty(){break}
        stored+=page.len();
        after=page.last().unwrap().seq;
    }
    if stored!=n{
        return Err("Legacy source was truncated or ANSF book contains unexpected events".into())
    }
    // Index last, once the original book's records are verified. A failed
    // Library write can be recovered by replay; originals remain immutable.
    library.index_book(&BookIndex{
        session_id:chat.chat_id.clone(),workspace_id:"lu-desktop-local-profile".into(),
        relative_path:format!("books/{}.sqlite",chat.chat_id),
        title:chat.title.clone(),summary:"Exact imported Lu conversation history".into(),
    }).map_err(|e|format!("{e:?}"))?;
    for chapter in book.chapters(&chat.chat_id).map_err(|e|format!("{e:?}"))?{
        library.index_chapter(&ChapterIndex{
            session_id:chat.chat_id.clone(),chapter_id:chapter.id,
            title:chapter.title,summary:chapter.summary,
            start_entry_id:chapter.start_entry_id,end_entry_id:chapter.end_entry_id,
        }).map_err(|e|format!("{e:?}"))?;
    }
    Ok(ImportReceipt{session_id:chat.chat_id.clone(),
        imported_chat_events:n,imported_goal_events:goals,
        last_source_seq:chat.updated_seq})
}
/// R14 flat history had no chat index; it is legacy import input ONLY.
fn import_r14_flat(root:&Path,library:&AnsfLibrary,books:&Path)
    ->Result<ImportReceipt,String>{
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FlatLine{role:String,text:String}
    let source=root.join("chats.jsonl");
    regular_required(&source,MAX_LEGACY_CHAT)?;
    let session=format!("c-{:032x}",1);
    let dest=books.join(format!("{session}.sqlite"));
    regular_or_missing(&dest)?;
    let book=AnsfStore::open(&dest).map_err(|e|format!("{e:?}"))?;
    book.ensure_session(&session,"lu-desktop-local-profile","Previous Lu conversation")
        .map_err(|e|format!("{e:?}"))?;
    if book.chapters(&session).map_err(|e|format!("{e:?}"))?.is_empty(){
        book.start_chapter(&session,"R14 imported history")
            .map_err(|e|format!("{e:?}"))?;
    }
    let mut imported=0;
    for (index,line) in BufReader::new(File::open(source).map_err(|e|e.to_string())?)
        .lines().enumerate(){
        let line=line.map_err(|e|e.to_string())?;
        if line.len()>MAX_LEGACY_LINE{return Err("Oversized R14 line".into())}
        if line.trim().is_empty(){continue}
        let original:FlatLine=serde_json::from_str(&line)
            .map_err(|e|format!("Invalid R14 history line {}: {e}",index+1))?;
        let kind=match original.role.as_str(){
            "user"=>LiveKind::User,
            "assistant"=>LiveKind::AssistantFinal,
            _=>return Err("Unsupported R14 conversation role".into()),
        };
        let event=LiveEvent{
            event_id:event_identity(&session,(index+1) as u64),
            kind,text:original.text,user_turn_id:None,legacy_turn_id:None,generation_id:None,
            call_id:None,tool_id:None,
        };
        book.import_legacy_chat_event(&session,&event)
            .map_err(|e|format!("R14 import conflict: {e:?}"))?;
        imported+=1;
    }
    let mut count=0usize;
    let mut cursor=0;
    loop{
        let page=book.live_chat_events_after(&session,cursor,100)
            .map_err(|e|format!("{e:?}"))?;
        if page.is_empty(){break}
        cursor=page.last().unwrap().seq;
        count+=page.len();
    }
    if count!=imported{return Err("Truncated R14 history cannot overwrite ANSF".into())}
    library.index_book(&BookIndex{
        session_id:session.clone(),workspace_id:"lu-desktop-local-profile".into(),
        relative_path:format!("books/{session}.sqlite"),
        title:"Previous Lu conversation".into(),summary:"R14 exact historical messages".into()
    }).map_err(|e|format!("{e:?}"))?;
    for chapter in book.chapters(&session).map_err(|e|format!("{e:?}"))?{
        library.index_chapter(&ChapterIndex{
            session_id:session.clone(),chapter_id:chapter.id,
            title:chapter.title,summary:chapter.summary,
            start_entry_id:chapter.start_entry_id,end_entry_id:chapter.end_entry_id
        }).map_err(|e|format!("{e:?}"))?;
    }
    Ok(ImportReceipt{
        session_id:session,imported_chat_events:imported,
        imported_goal_events:0,last_source_seq:1
    })
}
/// Prepare immutable per-conversation ANSF books under a separate directory.
/// This does NOT switch the desktop's current writers. The final cutover is
/// gated on a SQLite conversation/folder catalog and exact acceptance.
pub fn prepare_legacy_ansf_books(root:&Path)->Result<Vec<ImportReceipt>,String>{
    let index=root.join("conversation-index.json");
    let metadata:Option<LegacyIndex>=if index.exists(){
        regular_required(&index,MAX_LEGACY_INDEX)?;
        let bytes=fs::read(&index).map_err(|e|e.to_string())?;
        let decoded:LegacyIndex=serde_json::from_slice(&bytes)
            .map_err(|e|format!("Legacy chat metadata cannot be decoded: {e}"))?;
        if decoded.version!=1||decoded.chats.len()>1024||decoded.folders.len()>128{
            return Err("Legacy catalog version or capacity invalid".into())
        }
        let mut chat_ids=std::collections::HashSet::new();
        let folder_ids:std::collections::HashSet<_>=decoded.folders.iter()
            .map(|f|f.folder_id.as_str()).collect();
        if folder_ids.len()!=decoded.folders.len()||
           decoded.folders.iter().any(|f|!valid_id(&f.folder_id,"f-")||
               f.name.trim().is_empty()){
            return Err("Invalid legacy folders".into())
        }
        for chat in &decoded.chats{
            if !chat_ids.insert(&chat.chat_id)||
                chat.updated_seq>decoded.sequence||
                chat.folder_id.as_ref().is_some_and(|f|!folder_ids.contains(f.as_str())){
                return Err("Invalid or conflicting legacy chat catalog".into())
            }
        }
        Some(decoded)
    }else{None};
    if metadata.is_none()&&!root.join("chats.jsonl").exists(){return Ok(vec![])}
    let dest=root.join("ansf");
    fs::create_dir_all(&dest).map_err(|e|e.to_string())?;
    let md=fs::symlink_metadata(&dest).map_err(|e|e.to_string())?;
    if md.file_type().is_symlink()||!md.is_dir(){
        return Err("ANSF root is not a real local directory".into())
    }
    let books=dest.join("books");
    fs::create_dir_all(&books).map_err(|e|e.to_string())?;
    let bm=fs::symlink_metadata(&books).map_err(|e|e.to_string())?;
    if bm.file_type().is_symlink()||!bm.is_dir(){
        return Err("ANSF books root is not a real directory".into())
    }
    regular_or_missing(&dest.join("library.db"))?;
    let library=AnsfLibrary::open(&dest.join("library.db"))
        .map_err(|e|format!("{e:?}"))?;
    let mut receipts=Vec::new();
    if let Some(meta)=metadata{
        for chat in &meta.chats{
            receipts.push(import_one(root,&library,&books,chat)?);
        }
    }else{
        receipts.push(import_r14_flat(root,&library,&books)?);
    }
    Ok(receipts)
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::time::{SystemTime,UNIX_EPOCH};
    fn temp()->PathBuf{
        std::env::temp_dir().join(format!("lu-ansf-transition-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()))
    }
    #[test]
    fn jsonl_migration_is_idempotent_and_legacy_turns_are_not_grants(){
        let root=temp();
        fs::create_dir_all(root.join("conversations")).unwrap();
        let chat=format!("c-{:032x}",1);
        let turn=format!("g-{:032x}",2);
        fs::write(root.join("conversation-index.json"),
            format!(r#"{{"version":1,"sequence":1,"folders":[],"chats":[{{"chatId":"{chat}","title":"Test","folderId":null,"favorite":false,"updatedSeq":1}}]}}"#)).unwrap();
        let jsonl=format!(
            "{{\"kind\":\"user\",\"text\":\"Remember my decision\",\"user_turn_id\":\"{turn}\"}}\n\
             {{\"kind\":\"assistant_progress\",\"generation_id\":\"{turn}\",\"text\":\"partial\"}}\n\
             {{\"kind\":\"assistant_completed\",\"generation_id\":\"{turn}\"}}\n"
        );
        let path=root.join("conversations").join(format!("{chat}.jsonl"));
        fs::write(&path,&jsonl).unwrap();
        let a=prepare_legacy_ansf_books(&root).unwrap();
        let b=prepare_legacy_ansf_books(&root).unwrap();
        assert_eq!(a,b);
        assert_eq!(a[0].imported_chat_events,3);
        let db=AnsfStore::open(&root.join("ansf/books").join(format!("{chat}.sqlite"))).unwrap();
        assert_eq!(db.live_chat_events_after(&chat,0,100).unwrap().len(),3);
        assert_eq!(db.authenticated_chat_turn(&chat,&turn).unwrap(),None);
        assert_eq!(db.authenticated_reader_turn(&chat,&turn,"Remember my decision").unwrap(),false);
        assert_eq!(db.live_chat_events_after(&chat,0,100).unwrap()[0].event.text,
            "Remember my decision");
        assert_eq!(fs::read_to_string(&path).unwrap(),jsonl);
        // Editing a previously imported source line must fail, not rewrite history.
        let modified=jsonl.replace("Remember my decision","Forged edit to old message");
        fs::write(&path,modified).unwrap();
        assert!(prepare_legacy_ansf_books(&root).is_err());
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn r14_flat_jsonl_is_migrated_without_an_index_or_new_authority(){
        let root=temp();
        fs::create_dir_all(&root).unwrap();
        let flat="{\"role\":\"user\",\"text\":\"historical goal\"}\n\
                  {\"role\":\"assistant\",\"text\":\"historical answer\"}\n";
        fs::write(root.join("chats.jsonl"),flat).unwrap();
        let receipts=prepare_legacy_ansf_books(&root).unwrap();
        assert_eq!(receipts.len(),1);
        assert_eq!(receipts[0].imported_chat_events,2);
        let session=&receipts[0].session_id;
        let book=AnsfStore::open(
            &root.join("ansf/books").join(format!("{session}.sqlite"))).unwrap();
        assert_eq!(book.live_chat_events_after(session,0,10).unwrap().len(),2);
        assert!(book.authenticated_chat_turn(session,
            &format!("g-{:032x}",1)).unwrap().is_none());
        assert_eq!(fs::read_to_string(root.join("chats.jsonl")).unwrap(),flat);
        assert_eq!(prepare_legacy_ansf_books(&root).unwrap(),receipts);
        drop(book);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_truncated_and_unrecognized_legacy_events(){
        let root=temp();
        fs::create_dir_all(root.join("conversations")).unwrap();
        let chat=format!("c-{:032x}",3);
        fs::write(root.join("conversation-index.json"),
            format!(r#"{{"version":1,"sequence":1,"folders":[],"chats":[{{"chatId":"{chat}","title":"Test","folderId":null,"favorite":true,"updatedSeq":1}}]}}"#)).unwrap();
        let p=root.join("conversations").join(format!("{chat}.jsonl"));
        fs::write(&p,"{\"kind\":\"user\",\"text\":\"first\"}\n").unwrap();
        assert!(prepare_legacy_ansf_books(&root).is_ok());
        fs::write(&p,"").unwrap();
        assert!(prepare_legacy_ansf_books(&root).is_err());
        fs::write(&p,"{\"kind\":\"fake_user\",\"text\":\"evil\"}\n").unwrap();
        assert!(prepare_legacy_ansf_books(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
