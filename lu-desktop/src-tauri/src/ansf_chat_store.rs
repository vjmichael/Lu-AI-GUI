//! Lu desktop chat facade backed by authoritative ANSF SQLite conversation books.
//!
//! Old JSONL and conversation-index.json are read *only at the migration
//! boundary*. All new chat events and folder/favorite metadata use SQLite.
//! Preserves host-generated user identity and current tool authority rules.
//! Book/Library indexing and chapter/page tools remain independently reusable.
use lu_core::{
    ansf::AnsfStore,
    ansf_library::{AnsfLibrary,BookIndex,ChapterIndex},
    ansf_goals::{GoalEvent,GoalSnapshot,project_goal_events},
    ansf_live_chat::{LiveEvent,LiveKind,StoredLiveEvent},
};
use rand::{rngs::OsRng,RngCore};
use rusqlite::{params,Connection,OptionalExtension};
use serde::{Serialize,Deserialize};
use std::{
    collections::{HashMap,HashSet,VecDeque},
    fs,
    path::{Path,PathBuf},
    sync::{Arc,Mutex},
};
use crate::ansf_migration::prepare_legacy_ansf_books;

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ChatSummary{
    pub chat_id:String,
    pub title:String,
    pub folder_id:Option<String>,
    pub favorite:bool,
    pub updated_seq:u64,
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ChatFolder{pub folder_id:String,pub name:String}
#[derive(Debug,Clone,Serialize)]
pub struct ChatMessage{
    pub role:String,
    pub text:String,
    pub interrupted:bool,
    pub cancelled:bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCatalog{
    version:u32,
    sequence:u64,
    folders:Vec<ChatFolder>,
    chats:Vec<ChatSummary>,
}
pub struct ChatStore{
    root:PathBuf,
    library:AnsfLibrary,
    catalog:Mutex<Connection>,
    // Bounded hot-book handles avoid repeated SQLite schema initialization
    // for each streamed checkpoint; cold books remain solely on disk.
    hot_books:Mutex<VecDeque<(String,Arc<AnsfStore>)>>,
}
fn id(prefix:&str)->String{
    let mut bytes=[0u8;16];OsRng.fill_bytes(&mut bytes);
    format!("{}{}",prefix,bytes.iter().map(|b|format!("{b:02x}")).collect::<String>())
}
fn valid_id(value:&str,prefix:&str)->bool{
    value.len()==prefix.len()+32&&value.starts_with(prefix)&&
    value[prefix.len()..].bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn valid_name(name:&str)->bool{
    !name.trim().is_empty()&&name.len()<=100&&!name.contains('\0')
}
fn to_err<E:std::fmt::Debug>(e:E)->String{format!("{e:?}")}
fn current_seq(conn:&Connection)->Result<u64,String>{
    let v:i64=conn.query_row(
        "SELECT value FROM ansf_desktop_state WHERE name='sequence'",[],|r|r.get(0))
        .map_err(to_err)?;
    u64::try_from(v).map_err(to_err)
}
fn bump_seq(conn:&Connection)->Result<u64,String>{
    conn.execute("UPDATE ansf_desktop_state SET value=value+1 WHERE name='sequence'",[])
        .map_err(to_err)?;
    current_seq(conn)
}
impl ChatStore {
    pub fn open(root:&Path)->Result<Self,String>{
        fs::create_dir_all(root).map_err(to_err)?;
        let storage=root.join("ansf");
        let books=storage.join("books");
        fs::create_dir_all(&books).map_err(to_err)?;
        for p in [&storage,&books]{
            let meta=fs::symlink_metadata(p).map_err(to_err)?;
            if meta.file_type().is_symlink()||!meta.is_dir(){
                return Err("ANSF storage directory is not a real local directory".into())
            }
        }
        let library_path=storage.join("library.db");
        if let Ok(meta)=fs::symlink_metadata(&library_path){
            if meta.file_type().is_symlink()||!meta.is_file(){
                return Err("ANSF Library database is not a regular file".into())
            }
        }
        let library=AnsfLibrary::open(&library_path).map_err(to_err)?;
        let conn=Connection::open(&library_path).map_err(to_err)?;
        conn.pragma_update(None,"journal_mode","WAL").map_err(to_err)?;
        conn.pragma_update(None,"synchronous","FULL").map_err(to_err)?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS ansf_desktop_folders(
               folder_id TEXT PRIMARY KEY,name TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS ansf_desktop_chats(
               chat_id TEXT PRIMARY KEY,title TEXT NOT NULL,
               folder_id TEXT REFERENCES ansf_desktop_folders(folder_id),
               favorite INTEGER NOT NULL DEFAULT 0 CHECK(favorite IN (0,1)),
               updated_seq INTEGER NOT NULL,
               last_event_seq INTEGER NOT NULL DEFAULT 0);
             CREATE INDEX IF NOT EXISTS ansf_desktop_chat_activity
               ON ansf_desktop_chats(updated_seq DESC);
             CREATE TABLE IF NOT EXISTS ansf_desktop_state(
               name TEXT PRIMARY KEY,value INTEGER NOT NULL);
             INSERT OR IGNORE INTO ansf_desktop_state(name,value) VALUES('sequence',0);
             INSERT OR IGNORE INTO ansf_desktop_state(name,value) VALUES('legacy_cutover',0);"
        ).map_err(to_err)?;
        let mut store=Self{root:root.to_owned(),library,catalog:Mutex::new(conn),
            hot_books:Mutex::new(VecDeque::new())};
        store.reconcile_legacy_cutover()?;
        store.reconcile_activity()?;
        Ok(store)
    }
    fn ansf_book(&self,chat_id:&str)->Result<Arc<AnsfStore>,String>{
        if !valid_id(chat_id,"c-"){return Err("invalid conversation identity".into())}
        {
            let mut hot=self.hot_books.lock().map_err(to_err)?;
            if let Some(i)=hot.iter().position(|(id,_)|id==chat_id){
                let item=hot.remove(i).ok_or("Cache bookkeeping failed")?;
                let result=Arc::clone(&item.1);
                hot.push_back(item);
                return Ok(result)
            }
        }
        let book=self.library.get_book(chat_id).map_err(to_err)?
            .ok_or("ANSF conversation not indexed")?;
        if book.session_id!=chat_id||
           book.workspace_id!="lu-desktop-local-profile"||
           book.relative_path!=format!("books/{chat_id}.sqlite"){
            return Err("ANSF book identity or path changed".into())
        }
        let books=self.root.join("ansf/books");
        let path=books.join(format!("{chat_id}.sqlite"));
        let meta=fs::symlink_metadata(&path).map_err(to_err)?;
        if !meta.is_file()||meta.file_type().is_symlink(){
            return Err("ANSF book is not a regular local SQLite database".into())
        }
        let opened=Arc::new(AnsfStore::open(&path).map_err(to_err)?);
        let mut hot=self.hot_books.lock().map_err(to_err)?;
        // Another thread may have opened the same book while we checked
        // the host-owned Library. Reuse that handle instead of duplicating.
        if let Some((_,prior))=hot.iter().find(|(name,_)|name==chat_id){
            return Ok(Arc::clone(prior))
        }
        while hot.len()>=8 {hot.pop_front();}
        hot.push_back((chat_id.to_owned(),Arc::clone(&opened)));
        Ok(opened)
    }
    fn known(conn:&Connection,chat_id:&str)->Result<bool,String>{
        if !valid_id(chat_id,"c-"){return Ok(false)}
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM ansf_desktop_chats WHERE chat_id=?1)",
            [chat_id],|r|r.get::<_,bool>(0)
        ).map_err(to_err)
    }
    fn require(&self,chat_id:&str)->Result<Arc<AnsfStore>,String>{
        let conn=self.catalog.lock().map_err(to_err)?;
        if !Self::known(&conn,chat_id)?{return Err("unknown conversation".into())}
        self.ansf_book(chat_id)
    }
    fn reconcile_legacy_cutover(&mut self)->Result<(),String>{
        {
            let conn=self.catalog.lock().map_err(to_err)?;
            let done:i64=conn.query_row(
                "SELECT value FROM ansf_desktop_state WHERE name='legacy_cutover'",
                [],|r|r.get(0)).map_err(to_err)?;
            if done!=0{return Ok(())}
        }
        // Prepare all original messages before publishing the new SQLite catalog.
        // The JSON/JSONL source remains untouched, recoverable and read-only.
        let migration_receipts=prepare_legacy_ansf_books(&self.root)?;
        let index=self.root.join("conversation-index.json");
        let legacy:Option<LegacyCatalog>=if index.exists(){
            let metadata=fs::symlink_metadata(&index).map_err(to_err)?;
            if metadata.file_type().is_symlink()||!metadata.is_file()||
               metadata.len()>512*1024{
                return Err("unsafe legacy conversation catalog".into())
            }
            Some(serde_json::from_slice(&fs::read(index).map_err(to_err)?)
                .map_err(to_err)?)
        }else{None};
        let mut conn=self.catalog.lock().map_err(to_err)?;
        let tx=conn.transaction().map_err(to_err)?;
        let has_legacy_index=legacy.is_some();
        if let Some(saved)=legacy{
            if saved.version!=1||saved.chats.len()>1024||
               saved.folders.len()>128||saved.sequence>i64::MAX as u64{
                return Err("legacy catalog schema mismatch".into())
            }
            let mut seen=HashSet::new();
            for folder in saved.folders{
                if !valid_id(&folder.folder_id,"f-")||!valid_name(&folder.name)||
                   !seen.insert(folder.folder_id.clone()){
                    return Err("invalid legacy folder".into())
                }
                tx.execute(
                    "INSERT OR IGNORE INTO ansf_desktop_folders(folder_id,name)
                     VALUES(?1,?2)",params![folder.folder_id,folder.name]
                ).map_err(to_err)?;
            }
            for chat in saved.chats{
                if !valid_id(&chat.chat_id,"c-")||!valid_name(&chat.title)||
                   chat.updated_seq>saved.sequence{
                    return Err("invalid legacy chat".into())
                }
                // Preserve old conversation ordering. Do not mistake
                // imported source events for new post-migration activity.
                let source_seq=self.ansf_book(&chat.chat_id)?
                    .latest_live_chat_seq(&chat.chat_id).map_err(to_err)?;
                tx.execute(
                    "INSERT OR IGNORE INTO ansf_desktop_chats
                     (chat_id,title,folder_id,favorite,updated_seq,last_event_seq)
                     VALUES(?1,?2,?3,?4,?5,?6)",
                    params![chat.chat_id,chat.title,chat.folder_id,
                        chat.favorite as i64,chat.updated_seq as i64,source_seq]
                ).map_err(to_err)?;
            }
            tx.execute(
                "UPDATE ansf_desktop_state SET value=?1 WHERE name='sequence'",
                [saved.sequence as i64]
            ).map_err(to_err)?;
        }
        if !has_legacy_index {
            // Older R14 clients only had a flat chats.jsonl, not the
            // conversation index. It was converted to an ANSF book above.
            for receipt in &migration_receipts{
                tx.execute(
                    "INSERT OR IGNORE INTO ansf_desktop_chats
                     (chat_id,title,folder_id,favorite,updated_seq,last_event_seq)
                     VALUES(?1,'Previous Lu conversation',NULL,0,1,?2)",
                    params![receipt.session_id,self.ansf_book(&receipt.session_id)?
                        .latest_live_chat_seq(&receipt.session_id).map_err(to_err)?]
                ).map_err(to_err)?;
            }
            if !migration_receipts.is_empty(){
                tx.execute(
                    "UPDATE ansf_desktop_state SET value=1 WHERE name='sequence'",[]
                ).map_err(to_err)?;
            }
        }
        tx.execute(
            "UPDATE ansf_desktop_state SET value=1 WHERE name='legacy_cutover'",[]
        ).map_err(to_err)?;
        tx.commit().map_err(to_err)
    }
    /// Recover from a crash between an immutable book commit and the
    /// derived Library/catalog activity update. Never replay an effect or
    /// fabricate an omitted user-turn grant: this repairs UI metadata only.
    fn reconcile_activity(&self)->Result<(),String>{
        let mut conn=self.catalog.lock().map_err(to_err)?;
        let mut stmt=conn.prepare(
            "SELECT chat_id,last_event_seq FROM ansf_desktop_chats ORDER BY chat_id"
        ).map_err(to_err)?;
        let rows=stmt.query_map([],|r|
            Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))
            .map_err(to_err)?;
        let mut stale=Vec::new();
        for row in rows{
            let(chat_id,last_seen)=row.map_err(to_err)?;
            let original=self.ansf_book(&chat_id)?
                .latest_live_chat_seq(&chat_id).map_err(to_err)?;
            if original>last_seen{stale.push((chat_id,original))}
            if original<last_seen{
                return Err("ANSF original chat history is truncated or catalog forged".into())
            }
        }
        drop(stmt);
        for (chat_id,original) in stale{
            let tx=conn.transaction().map_err(to_err)?;
            let next=bump_seq(&tx)?;
            tx.execute(
                "UPDATE ansf_desktop_chats SET updated_seq=?2,last_event_seq=?3
                 WHERE chat_id=?1",
                params![chat_id,next as i64,original]
            ).map_err(to_err)?;
            tx.commit().map_err(to_err)?;
        }
        Ok(())
    }
    pub fn list(&self)->Result<(Vec<ChatSummary>,Vec<ChatFolder>),String>{
        let conn=self.catalog.lock().map_err(to_err)?;
        let mut chats=Vec::new();
        let mut stmt=conn.prepare(
            "SELECT chat_id,title,folder_id,favorite,updated_seq
             FROM ansf_desktop_chats ORDER BY updated_seq DESC"
        ).map_err(to_err)?;
        let rows=stmt.query_map([],|r|{
            Ok(ChatSummary{chat_id:r.get(0)?,title:r.get(1)?,
                folder_id:r.get(2)?,favorite:r.get::<_,bool>(3)?,
                updated_seq:r.get::<_,i64>(4)? as u64})
        }).map_err(to_err)?;
        for row in rows{chats.push(row.map_err(to_err)?)}
        drop(stmt);
        let mut folders=Vec::new();
        let mut stmt=conn.prepare(
            "SELECT folder_id,name FROM ansf_desktop_folders ORDER BY rowid"
        ).map_err(to_err)?;
        let rows=stmt.query_map([],|r|
            Ok(ChatFolder{folder_id:r.get(0)?,name:r.get(1)?})).map_err(to_err)?;
        for row in rows{folders.push(row.map_err(to_err)?)}
        Ok((chats,folders))
    }
    pub fn create_chat(&self)->Result<ChatSummary,String>{
        let mut conn=self.catalog.lock().map_err(to_err)?;
        let chat_id=id("c-");
        let dest=self.root.join("ansf/books").join(format!("{chat_id}.sqlite"));
        if dest.exists(){return Err("duplicate chat path".into())}
        let book=AnsfStore::open(&dest).map_err(to_err)?;
        book.create_session(&chat_id,"lu-desktop-local-profile","New chat")
            .map_err(to_err)?;
        let chapter=book.start_chapter(&chat_id,"Conversation")
            .map_err(to_err)?;
        self.library.index_book(&BookIndex{
            session_id:chat_id.clone(),workspace_id:"lu-desktop-local-profile".into(),
            relative_path:format!("books/{chat_id}.sqlite"),title:"New chat".into(),
            summary:"Original Lu conversation history".into()
        }).map_err(to_err)?;
        self.library.index_chapter(&ChapterIndex{
            session_id:chat_id.clone(),chapter_id:chapter,
            title:"Conversation".into(),summary:String::new(),
            start_entry_id:None,end_entry_id:None
        }).map_err(to_err)?;
        let tx=conn.transaction().map_err(to_err)?;
        let seq=bump_seq(&tx)?;
        tx.execute("INSERT INTO ansf_desktop_chats
            (chat_id,title,folder_id,favorite,updated_seq)
            VALUES(?1,'New chat',NULL,0,?2)",
            params![chat_id,seq as i64]).map_err(to_err)?;
        tx.commit().map_err(to_err)?;
        Ok(ChatSummary{chat_id,title:"New chat".into(),
            folder_id:None,favorite:false,updated_seq:seq})
    }
    pub fn create_folder(&self,name:&str)->Result<ChatFolder,String>{
        if !valid_name(name){return Err("invalid folder name".into())}
        let conn=self.catalog.lock().map_err(to_err)?;
        let count:i64=conn.query_row("SELECT COUNT(*) FROM ansf_desktop_folders",
            [],|r|r.get(0)).map_err(to_err)?;
        if count>=128{return Err("folder limit reached".into())}
        let folder=ChatFolder{folder_id:id("f-"),name:name.trim().into()};
        conn.execute("INSERT INTO ansf_desktop_folders(folder_id,name) VALUES(?1,?2)",
            params![folder.folder_id,folder.name]).map_err(to_err)?;
        Ok(folder)
    }
    pub fn favorite(&self,chat_id:&str,favorite:bool)->Result<(),String>{
        let conn=self.catalog.lock().map_err(to_err)?;
        let changed=conn.execute(
            "UPDATE ansf_desktop_chats SET favorite=?2 WHERE chat_id=?1",
            params![chat_id,favorite]).map_err(to_err)?;
        if changed!=1{return Err("unknown chat".into())}
        Ok(())
    }
    pub fn move_chat(&self,chat_id:&str,folder_id:Option<String>)->Result<(),String>{
        let conn=self.catalog.lock().map_err(to_err)?;
        if let Some(folder)=folder_id.as_ref(){
            let exists:bool=conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM ansf_desktop_folders WHERE folder_id=?1)",
                [folder],|r|r.get(0)).map_err(to_err)?;
            if !exists{return Err("unknown folder".into())}
        }
        let changed=conn.execute(
            "UPDATE ansf_desktop_chats SET folder_id=?2 WHERE chat_id=?1",
            params![chat_id,folder_id]).map_err(to_err)?;
        if changed!=1{return Err("unknown chat".into())}
        Ok(())
    }
    fn append(&self,chat_id:&str,kind:LiveKind,text:&str,
        turn_id:Option<&str>,generation_id:Option<&str>,
        call_id:Option<&str>,tool_id:Option<&str>)->Result<(),String>{
        // serialize appends and catalog activity under the profile-owner lease.
        let mut conn=self.catalog.lock().map_err(to_err)?;
        if !Self::known(&conn,chat_id)?{return Err("unknown chat".into())}
        let book=self.ansf_book(chat_id)?;
        let event=LiveEvent{
            event_id:id("e-"),kind,text:text.into(),
            user_turn_id:turn_id.map(str::to_owned),
            legacy_turn_id:None,
            generation_id:generation_id.map(str::to_owned),
            call_id:call_id.map(str::to_owned),tool_id:tool_id.map(str::to_owned),
        };
        let committed=book.append_host_chat_event(chat_id,&event).map_err(to_err)?;
        // The canonical event is durable before any derived UI catalog update.
        let tx=conn.transaction().map_err(to_err)?;
        if matches!(kind,LiveKind::User|LiveKind::ReaderUser){
            let title:String=text.chars().take(65).collect();
            tx.execute(
                "UPDATE ansf_desktop_chats SET title=?2
                 WHERE chat_id=?1 AND title='New chat'",
                params![chat_id,title]
            ).map_err(to_err)?;
        }
        let seq=bump_seq(&tx)?;
        tx.execute(
            "UPDATE ansf_desktop_chats SET updated_seq=?2,last_event_seq=?3 WHERE chat_id=?1",
            params![chat_id,seq as i64,committed.seq]
        ).map_err(to_err)?;
        tx.commit().map_err(to_err)?;
        Ok(())
    }
    pub fn append_user(&self,chat_id:&str,text:&str)->Result<(),String>{
        if text.is_empty()||text.len()>16*1024{return Err("invalid chat input".into())}
        self.append(chat_id,LiveKind::User,text,None,None,None,None)
    }
    pub fn append_user_with_turn_id(&self,chat_id:&str,user_turn_id:&str,
        text:&str)->Result<(),String>{
        if !valid_id(user_turn_id,"g-")||text.is_empty()||text.len()>16*1024{
            return Err("invalid authenticated human turn".into())
        }
        self.append(chat_id,LiveKind::User,text,Some(user_turn_id),None,None,None)
    }
    pub fn append_reader_user(&self,chat_id:&str,generation_id:&str,
        text:&str)->Result<(),String>{
        if !valid_id(generation_id,"g-")||text.is_empty()||text.len()>16*1024{
            return Err("invalid Reader generation".into())
        }
        self.append(chat_id,LiveKind::ReaderUser,text,None,Some(generation_id),None,None)
    }
    pub fn latest_authenticated_user_turn_id(&self,chat_id:&str)->Option<String>{
        let book=self.require(chat_id).ok()?;
        book.latest_authenticated_chat_turn(chat_id).ok().flatten().map(|(id,_)|id)
    }
    pub fn trusted_user_turn_text(&self,chat_id:&str,user_turn_id:&str)
        ->Option<String>{
        self.require(chat_id).ok()?.authenticated_chat_turn(chat_id,user_turn_id)
            .ok().flatten().map(|(text,_)|text)
    }
    pub fn reader_user_turn_matches(&self,chat_id:&str,generation_id:&str,
        expected_text:&str)->Result<bool,String>{
        self.require(chat_id)?.authenticated_reader_turn(chat_id,generation_id,expected_text)
            .map_err(to_err)
    }
    pub fn append_progress(&self,chat_id:&str,generation_id:&str,
        text:&str)->Result<(),String>{
        self.append(chat_id,LiveKind::AssistantProgress,text,None,
            Some(generation_id),None,None)
    }
    pub fn complete(&self,chat_id:&str,generation_id:&str)->Result<(),String>{
        self.append(chat_id,LiveKind::AssistantCompleted,"",None,
            Some(generation_id),None,None)
    }
    pub fn cancel(&self,chat_id:&str,generation_id:&str)->Result<(),String>{
        self.append(chat_id,LiveKind::AssistantCancelled,"",None,
            Some(generation_id),None,None)
    }
    pub fn append_browser_observation(&self,chat_id:&str,generation_id:&str,
        call_id:&str,tool:&str,summary:&str)->Result<(),String>{
        if self.trusted_user_turn_text(chat_id,generation_id).is_none(){
            return Err("Browser receipt lacks verified current human turn".into())
        }
        self.append(chat_id,LiveKind::BrowserObservation,summary,None,
            Some(generation_id),Some(call_id),Some(tool))
    }
    pub fn memory_events(&self,chat_id:&str)->Result<Vec<GoalEvent>,String>{
        self.require(chat_id)?.native_goal_records(chat_id).map_err(to_err)
    }
    pub fn append_goal_memory_event(&self,chat_id:&str,event:GoalEvent)->Result<(),String>{
        self.require(chat_id)?.append_native_goal_record(chat_id,&event)
            .map_err(to_err).map(|_|())
    }
    /// Model-only bounded original window; do not materialize the full
    /// human-auditable history to form a 16-turn typed inference context.
    /// Incomplete/cancelled assistant streams have no finalized ANSF entry.
    pub fn recent_model_history(&self,chat_id:&str,limit:usize)
        ->Result<Vec<ChatMessage>,String>{
        let source=self.require(chat_id)?.recent_context_entries(chat_id,limit)
            .map_err(to_err)?;
        Ok(source.into_iter().map(|entry|ChatMessage{
            role:entry.role,text:entry.content,
            interrupted:false,cancelled:false,
        }).collect())
    }
    pub fn history(&self,chat_id:&str)->Result<Vec<ChatMessage>,String>{
        let book=self.require(chat_id)?;
        let mut last=0i64;
        let mut result=Vec::<ChatMessage>::new();
        let mut pending=HashMap::<String,usize>::new();
        loop{
            let page=book.live_chat_events_after(chat_id,last,100).map_err(to_err)?;
            if page.is_empty(){break}
            for StoredLiveEvent{seq,event,..} in page{
                last=seq;
                match event.kind{
                    LiveKind::User|LiveKind::ReaderUser=>result.push(ChatMessage{
                        role:"user".into(),text:event.text,interrupted:false,cancelled:false}),
                    LiveKind::AssistantFinal=>result.push(ChatMessage{
                        role:"assistant".into(),text:event.text,interrupted:false,cancelled:false}),
                    LiveKind::AssistantProgress=>{
                        let gen=event.generation_id.unwrap_or_default();
                        if let Some(i)=pending.get(&gen){
                            result[*i].text=event.text;
                        }else{
                            pending.insert(gen,result.len());
                            result.push(ChatMessage{role:"assistant".into(),text:event.text,
                                interrupted:true,cancelled:false});
                        }
                    }
                    LiveKind::AssistantCompleted=>{
                        if let Some(i)=pending.remove(event.generation_id.as_deref().unwrap_or("")){
                            result[i].interrupted=false;
                        }
                    }
                    LiveKind::AssistantCancelled=>{
                        if let Some(i)=pending.remove(event.generation_id.as_deref().unwrap_or("")){
                            result[i].interrupted=true;result[i].cancelled=true;
                        }
                    }
                    LiveKind::BrowserObservation=>{}
                }
            }
        }
        Ok(result)
    }
}
impl lu_core::conversation_authority::TrustedUserTurnVerifier for ChatStore{
    fn verify_user_turn(&self,chat_id:&str,user_turn_id:&str)->bool{
        self.trusted_user_turn_text(chat_id,user_turn_id).is_some()
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::time::{SystemTime,UNIX_EPOCH};
    fn tmp()->PathBuf{
        std::env::temp_dir().join(format!("lu-ansf-desktop-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()))
    }
    #[test]
    fn sqlite_is_primary_for_new_messages_and_restarts(){
        let root=tmp();
        let chat_id;
        let turn=format!("g-{:032x}",3);
        {
            let chat=ChatStore::open(&root).unwrap();
            let c=chat.create_chat().unwrap();chat_id=c.chat_id;
            let folder=chat.create_folder("Research").unwrap();
            chat.move_chat(&chat_id,Some(folder.folder_id.clone())).unwrap();
            chat.favorite(&chat_id,true).unwrap();
            chat.append_user_with_turn_id(&chat_id,&turn,"Remember ANSF").unwrap();
            chat.append_progress(&chat_id,&turn,"partial").unwrap();
            chat.append_progress(&chat_id,&turn,"complete").unwrap();
            chat.complete(&chat_id,&turn).unwrap();
            assert_eq!(chat.history(&chat_id).unwrap().len(),2);
            assert!(chat.trusted_user_turn_text(&chat_id,&turn).is_some());
            assert!(chat.append_user_with_turn_id(&chat_id,&turn,"duplicate").is_err());
            assert!(!root.join("conversation-index.json").exists());
            assert!(!root.join("conversations").exists());
        }
        {
            let chat=ChatStore::open(&root).unwrap();
            assert_eq!(chat.history(&chat_id).unwrap()[1].text,"complete");
            assert!(chat.list().unwrap().0[0].favorite);
            assert_eq!(chat.trusted_user_turn_text(&chat_id,&turn).as_deref(),
                Some("Remember ANSF"));
            assert!(chat.history("../../bad").is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn imports_old_chat_and_preserves_favorites_but_drops_old_grants(){
        let root=tmp();
        fs::create_dir_all(root.join("conversations")).unwrap();
        let chat=format!("c-{:032x}",4);
        let folder=format!("f-{:032x}",5);
        let old_turn=format!("g-{:032x}",6);
        let index=format!(
            r#"{{"version":1,"sequence":5,"folders":[{{"folderId":"{folder}","name":"Original projects"}}],"chats":[{{"chatId":"{chat}","title":"Original title","folderId":"{folder}","favorite":true,"updatedSeq":5}}]}}"#
        );
        let journal=format!(
            "{{\"kind\":\"user\",\"text\":\"Ancient instruction\",\"user_turn_id\":\"{old_turn}\"}}\n\
             {{\"kind\":\"assistant_final\",\"text\":\"Ancient response\"}}\n"
        );
        let journal_path=root.join("conversations").join(format!("{chat}.jsonl"));
        fs::write(root.join("conversation-index.json"),&index).unwrap();
        fs::write(&journal_path,&journal).unwrap();
        {
            let store=ChatStore::open(&root).unwrap();
            assert_eq!(store.history(&chat).unwrap().len(),2);
            let (chats,folders)=store.list().unwrap();
            assert_eq!(chats[0].title,"Original title");
            assert!(chats[0].favorite);
            assert_eq!(chats[0].folder_id.as_deref(),Some(folder.as_str()));
            assert_eq!(folders[0].name,"Original projects");
            assert!(store.trusted_user_turn_text(&chat,&old_turn).is_none());
            let new_turn=format!("g-{:032x}",7);
            store.append_user_with_turn_id(&chat,&new_turn,"New permission-scoped turn").unwrap();
            assert_eq!(store.trusted_user_turn_text(&chat,&new_turn).as_deref(),
                Some("New permission-scoped turn"));
        }
        // The historical JSONL remains a backup but is no longer a writer
        // or canonical source after the committed SQLite cutover marker.
        fs::write(&journal_path,"invalid legacy JSONL after cutover\n").unwrap();
        let reopened=ChatStore::open(&root).unwrap();
        assert_eq!(reopened.history(&chat).unwrap().len(),3);
        assert!(reopened.trusted_user_turn_text(&chat,&old_turn).is_none());
        assert_eq!(fs::read_to_string(root.join("conversation-index.json")).unwrap(),index);
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn recovers_catalog_activity_after_book_commit_without_replaying_effects(){
        let root=tmp();
        let chat_id;
        {
            let s=ChatStore::open(&root).unwrap();
            chat_id=s.create_chat().unwrap().chat_id;
            s.append_user(&chat_id,"Durable original").unwrap();
        }
        // Emulate power loss between SQLite book commit and a separate
        // Library activity commit. The immutable book is source of truth.
        let index=root.join("ansf/library.db");
        let conn=Connection::open(&index).unwrap();
        conn.execute("UPDATE ansf_desktop_chats SET last_event_seq=0
                      WHERE chat_id=?1",[&chat_id]).unwrap();
        drop(conn);
        let reopened=ChatStore::open(&root).unwrap();
        assert_eq!(reopened.history(&chat_id).unwrap().len(),1);
        let conn=Connection::open(index).unwrap();
        let last:i64=conn.query_row(
            "SELECT last_event_seq FROM ansf_desktop_chats WHERE chat_id=?1",
            [&chat_id],|r|r.get(0)).unwrap();
        assert!(last>=1);
        drop(conn);drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounded_model_context_skips_old_entries_and_unfinished_assistant(){
        let root=tmp();
        let store=ChatStore::open(&root).unwrap();
        let chat=store.create_chat().unwrap();
        for i in 0..128{
            store.append_user(&chat.chat_id,&format!("Turn {i}")).unwrap();
        }
        let gen=format!("g-{:032x}",42);
        store.append_progress(&chat.chat_id,&gen,"Interrupted partial").unwrap();
        let recent=store.recent_model_history(&chat.chat_id,16).unwrap();
        assert_eq!(recent.len(),16);
        assert_eq!(recent[0].text,"Turn 112");
        assert_eq!(recent[15].text,"Turn 127");
        assert!(!recent.iter().any(|m|m.interrupted||m.text.contains("partial")));
        assert_eq!(store.history(&chat.chat_id).unwrap().len(),129);
        assert!(store.recent_model_history(&chat.chat_id,129).is_err());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn flat_r14_no_index_is_visible_without_new_permission(){
        let root=tmp();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("chats.jsonl"),
            "{\"role\":\"user\",\"text\":\"Old R14 message\"}\n").unwrap();
        let store=ChatStore::open(&root).unwrap();
        let (chats,_)=store.list().unwrap();
        assert_eq!(chats.len(),1);
        assert_eq!(store.history(&chats[0].chat_id).unwrap()[0].text,
            "Old R14 message");
        assert!(store.latest_authenticated_user_turn_id(&chats[0].chat_id).is_none());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}
