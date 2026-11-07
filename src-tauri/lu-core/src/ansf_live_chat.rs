//! M2 canonical Lu chat events: original history in ANSF SQLite, not JSONL.
//!
//! Protobuf remains Lu's internal IPC; SQLite uses relational columns.
//! A legacy JSONL reader may import observed records, but those records are
//! NOT authenticated current human turns and never become tool authority.
//! The host owns identity issuance, profile/ownership validation and dispatch.
use rusqlite::{params,Connection,OptionalExtension};
use crate::ansf::{AnsfError,AnsfStore,Entry};

pub const MAX_LIVE_TEXT_BYTES:usize=2*1024*1024;
pub const MAX_LIVE_EVENTS_PAGE:usize=100;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum LiveKind{
    User,ReaderUser,AssistantFinal,AssistantProgress,
    AssistantCompleted,AssistantCancelled,BrowserObservation,
}
impl LiveKind{
    fn as_str(self)->&'static str{
        match self{
            Self::User=>"user",
            Self::ReaderUser=>"reader_user",
            Self::AssistantFinal=>"assistant_final",
            Self::AssistantProgress=>"assistant_progress",
            Self::AssistantCompleted=>"assistant_completed",
            Self::AssistantCancelled=>"assistant_cancelled",
            Self::BrowserObservation=>"browser_observation",
        }
    }
    fn parse(value:&str)->Result<Self,AnsfError>{
        match value{
            "user"=>Ok(Self::User),"reader_user"=>Ok(Self::ReaderUser),
            "assistant_final"=>Ok(Self::AssistantFinal),
            "assistant_progress"=>Ok(Self::AssistantProgress),
            "assistant_completed"=>Ok(Self::AssistantCompleted),
            "assistant_cancelled"=>Ok(Self::AssistantCancelled),
            "browser_observation"=>Ok(Self::BrowserObservation),
            _=>Err(AnsfError::InvalidInput),
        }
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum LiveOrigin{
    /// Only a trusted desktop host may issue a new authenticated human turn.
    HostAuthenticated,
    /// Imported old chat: historical context only, NEVER a fresh tool grant.
    LegacyObserved,
}
impl LiveOrigin{
    fn as_str(self)->&'static str{
        match self{Self::HostAuthenticated=>"host",Self::LegacyObserved=>"legacy"}
    }
    fn parse(raw:&str)->Result<Self,AnsfError>{
        match raw{"host"=>Ok(Self::HostAuthenticated),
            "legacy"=>Ok(Self::LegacyObserved),_=>Err(AnsfError::InvalidInput)}
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct LiveEvent{
    pub event_id:String,
    pub kind:LiveKind,
    pub text:String,
    pub user_turn_id:Option<String>,
    /// Historical imported turn identity, retained for auditing, NEVER tool authority.
    pub legacy_turn_id:Option<String>,
    pub generation_id:Option<String>,
    pub call_id:Option<String>,
    pub tool_id:Option<String>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct StoredLiveEvent{
    pub seq:i64,
    pub session_id:String,
    pub event:LiveEvent,
    pub origin:LiveOrigin,
    pub source_entry_id:Option<i64>,
}
fn opaque(value:&str,expected:&str)->bool{
    value.len()==expected.len()+32&&value.starts_with(expected)&&
        value[expected.len()..].bytes().all(|v|v.is_ascii_hexdigit()&&!v.is_ascii_uppercase())
}
fn valid_session(s:&str)->bool{!s.trim().is_empty()&&s.len()<=128&&!s.contains('\0')}
impl LiveEvent{
    fn validate(&self,origin:LiveOrigin)->Result<(),AnsfError>{
        if !opaque(&self.event_id,"e-")||
           self.text.len()>MAX_LIVE_TEXT_BYTES||self.text.contains('\0'){
            return Err(AnsfError::InvalidInput)
        }
        let valid_generation=|s:&str| if origin==LiveOrigin::HostAuthenticated {
            opaque(s,"g-")
        } else { !s.is_empty()&&s.len()<=128&&!s.contains('\0') };
        let valid_call=|s:&str| if origin==LiveOrigin::HostAuthenticated {
            opaque(s,"browser-call-")
        } else { !s.is_empty()&&s.len()<=128&&!s.contains('\0') };
        if self.user_turn_id.as_deref().is_some_and(|s|!opaque(s,"g-"))||
           self.generation_id.as_deref().is_some_and(|s|!valid_generation(s))||
           self.call_id.as_deref().is_some_and(|s|!valid_call(s)){
            return Err(AnsfError::InvalidInput)
        }
        if self.legacy_turn_id.as_ref().is_some_and(|s|
            origin!=LiveOrigin::LegacyObserved||self.kind!=LiveKind::User||
            s.is_empty()||s.len()>128||s.contains('\0')){
            return Err(AnsfError::InvalidInput)
        }
        if self.tool_id.as_ref().is_some_and(|s|s.len()>128||s.contains('\0')){
            return Err(AnsfError::InvalidInput)
        }
        let ok=match self.kind{
            LiveKind::User=>
                (origin==LiveOrigin::LegacyObserved||!self.text.is_empty())&&
                self.generation_id.is_none()&&
                self.call_id.is_none()&&self.tool_id.is_none()&&
                (origin==LiveOrigin::LegacyObserved && self.user_turn_id.is_none() ||
                 origin==LiveOrigin::HostAuthenticated &&
                 self.user_turn_id.as_deref().is_none_or(|s|opaque(s,"g-"))),
            LiveKind::ReaderUser=>
                !self.text.is_empty()&&
                self.generation_id.is_some()&&self.user_turn_id.is_none()&&
                self.call_id.is_none()&&self.tool_id.is_none(),
            LiveKind::AssistantFinal=>
                self.user_turn_id.is_none()&&
                self.generation_id.is_none()&&self.call_id.is_none()&&self.tool_id.is_none(),
            LiveKind::AssistantProgress=>
                self.generation_id.is_some()&&self.user_turn_id.is_none()&&
                self.call_id.is_none()&&self.tool_id.is_none(),
            LiveKind::AssistantCompleted|LiveKind::AssistantCancelled=>
                self.text.is_empty()&&self.generation_id.is_some()&&
                self.user_turn_id.is_none()&&self.call_id.is_none()&&self.tool_id.is_none(),
            LiveKind::BrowserObservation=>
                self.generation_id.is_some()&&self.call_id.is_some()&&
                self.tool_id.as_deref()==Some("lu_browser_read")&&
                !self.text.is_empty()&&self.text.len()<=3200&&self.user_turn_id.is_none(),
        };
        if !ok{return Err(AnsfError::InvalidInput)}
        Ok(())
    }
}
fn map_row(r:&rusqlite::Row<'_>)->rusqlite::Result<(i64,String,String,String,
    String,Option<String>,Option<String>,Option<String>,Option<String>,Option<String>,String,Option<i64>)>{
    Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,
        r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?,r.get(11)?))
}
fn decode(row:(i64,String,String,String,String,Option<String>,Option<String>,
    Option<String>,Option<String>,Option<String>,String,Option<i64>))->Result<StoredLiveEvent,AnsfError>{
    let(seq,session_id,event_id,kind,text,user_turn_id,legacy_turn_id,generation_id,
        call_id,tool_id,origin,source_entry_id)=row;
    let origin=LiveOrigin::parse(&origin)?;
    let event=LiveEvent{event_id,kind:LiveKind::parse(&kind)?,text,
        user_turn_id,legacy_turn_id,generation_id,call_id,tool_id};
    event.validate(origin)?;
    Ok(StoredLiveEvent{seq,session_id,event,origin,source_entry_id})
}
pub(crate) fn initialize_live_chat_schema(conn:&Connection)->Result<(),AnsfError>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_live_chat_events (
           seq INTEGER PRIMARY KEY AUTOINCREMENT,
           session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
           event_id TEXT NOT NULL UNIQUE,
           kind TEXT NOT NULL CHECK(kind IN
             ('user','reader_user','assistant_final','assistant_progress',
              'assistant_completed','assistant_cancelled','browser_observation')),
           text TEXT NOT NULL,
           user_turn_id TEXT,
           legacy_turn_id TEXT,
           generation_id TEXT,
           call_id TEXT,
           tool_id TEXT,
           origin TEXT NOT NULL CHECK(origin IN ('host','legacy')),
           source_entry_id INTEGER REFERENCES ansf_entries(id),
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE INDEX IF NOT EXISTS ansf_live_chat_event_cursor
           ON ansf_live_chat_events(session_id,seq);
         CREATE UNIQUE INDEX IF NOT EXISTS ansf_live_authenticated_user
           ON ansf_live_chat_events(session_id,user_turn_id)
           WHERE kind='user' AND origin='host' AND user_turn_id IS NOT NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS ansf_live_reader_generation
           ON ansf_live_chat_events(session_id,generation_id)
           WHERE kind='reader_user' AND origin='host';
         CREATE TRIGGER IF NOT EXISTS ansf_live_chat_no_update
           BEFORE UPDATE ON ansf_live_chat_events BEGIN
             SELECT RAISE(ABORT,'ANSF live original events are immutable');
           END;
         CREATE TRIGGER IF NOT EXISTS ansf_live_chat_no_delete
           BEFORE DELETE ON ansf_live_chat_events BEGIN
             SELECT RAISE(ABORT,'ANSF live original events are immutable');
           END;"
    )?;
    Ok(())
}
impl AnsfStore{
    /// Append an original host-authenticated Lu event directly to SQLite.
    /// This API is for the trusted host only, never for model-supplied JSON,
    /// imported transcripts or generic MCP calls.
    pub fn append_host_chat_event(&self,session_id:&str,event:&LiveEvent)
        ->Result<StoredLiveEvent,AnsfError>{
        self.append_live_chat_event(session_id,event,LiveOrigin::HostAuthenticated,false)
    }
    /// Boundary-only migration of old JSONL events. Never authenticates a
    /// historical User record, even if its source contains a generation ID.
    /// Exact replay is idempotent; same ID with changed content fails.
    pub fn import_legacy_chat_event(&self,session_id:&str,event:&LiveEvent)
        ->Result<StoredLiveEvent,AnsfError>{
        self.append_live_chat_event(session_id,event,LiveOrigin::LegacyObserved,true)
    }
    fn append_live_chat_event(&self,session_id:&str,event:&LiveEvent,
        origin:LiveOrigin,idempotent:bool)->Result<StoredLiveEvent,AnsfError>{
        if !valid_session(session_id){return Err(AnsfError::InvalidInput)}
        event.validate(origin)?;
        let mut conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let tx=conn.transaction()?;
        let exists:Option<StoredLiveEvent>=tx.query_row(
            "SELECT seq,session_id,event_id,kind,text,user_turn_id,legacy_turn_id,generation_id,
                    call_id,tool_id,origin,source_entry_id
             FROM ansf_live_chat_events WHERE event_id=?1",
            [&event.event_id],map_row
        ).optional()?.map(decode).transpose()?;
        if let Some(saved)=exists{
            if idempotent && saved.session_id==session_id&&
               saved.event==*event&&saved.origin==origin{return Ok(saved)}
            return Err(AnsfError::InvalidInput)
        }
        let active_chapter:Option<i64>=tx.query_row(
            "SELECT id FROM ansf_chapters WHERE session_id=?1 AND closed=0",
            [session_id],|r|r.get(0)).optional()?;
        let chapter=active_chapter.ok_or(AnsfError::NoActiveChapter)?;
        // Complete events are not generated from arbitrary model text;
        // only the host's live generation ID may finish its recorded stream.
        let completed_text=if event.kind==LiveKind::AssistantCompleted {
            tx.query_row(
                "SELECT text FROM ansf_live_chat_events
                 WHERE session_id=?1 AND generation_id=?2
                   AND kind='assistant_progress'
                 ORDER BY seq DESC LIMIT 1",
                params![session_id,event.generation_id],
                |r|r.get::<_,String>(0)
            ).optional()?
        }else{None};
        if matches!(event.kind,LiveKind::AssistantCompleted|LiveKind::AssistantCancelled){
            let previous:i64=tx.query_row(
                "SELECT COUNT(*) FROM ansf_live_chat_events
                 WHERE session_id=?1 AND generation_id=?2
                 AND kind IN ('assistant_completed','assistant_cancelled')",
                params![session_id,event.generation_id],|r|r.get(0))?;
            if previous!=0{return Err(AnsfError::InvalidInput)}
        }
        if event.kind==LiveKind::BrowserObservation && origin==LiveOrigin::HostAuthenticated{
            let trusted:i64=tx.query_row(
                "SELECT COUNT(*) FROM ansf_live_chat_events
                 WHERE session_id=?1 AND origin='host' AND kind='user'
                 AND user_turn_id=?2",
                params![session_id,event.generation_id],|r|r.get(0))?;
            if trusted!=1{return Err(AnsfError::InvalidInput)}
        }
        let source_data=match event.kind {
            LiveKind::User|LiveKind::ReaderUser=>
                Some(("user",event.text.as_str(),event.event_id.as_str())),
            LiveKind::AssistantFinal=>
                Some(("assistant",event.text.as_str(),event.event_id.as_str())),
            LiveKind::AssistantCompleted if completed_text.as_deref().is_some_and(|s|!s.is_empty())=>
                Some(("assistant",completed_text.as_deref().unwrap_or(""),event.event_id.as_str())),
            _=>None,
        };
        let source_entry_id=if let Some((role,text,msg_id))=source_data{
            // Preserve exact original; generated summaries are navigation aids,
            // not the model's authority and not replacements for source text.
            let summary:String=text.chars().take(160).collect();
            tx.execute(
                "INSERT INTO ansf_entries
                 (session_id,chapter_id,message_id,role,summary,content)
                 VALUES(?1,?2,?3,?4,?5,?6)",
                params![session_id,chapter,msg_id,role,summary,text],
            )?;
            Some(tx.last_insert_rowid())
        }else{None};
        tx.execute(
            "INSERT INTO ansf_live_chat_events
             (session_id,event_id,kind,text,user_turn_id,legacy_turn_id,generation_id,call_id,
              tool_id,origin,source_entry_id)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![session_id,event.event_id,event.kind.as_str(),
                event.text,event.user_turn_id,event.legacy_turn_id,event.generation_id,
                event.call_id,event.tool_id,origin.as_str(),source_entry_id],
        )?;
        let seq=tx.last_insert_rowid();
        if let Some(entry_id)=source_entry_id {
            tx.execute(
                "UPDATE ansf_chapters
                 SET start_entry_id=COALESCE(start_entry_id,?1),
                     end_entry_id=?1 WHERE id=?2",
                params![entry_id,chapter],
            )?;
        }
        tx.commit()?;
        Ok(StoredLiveEvent{seq,session_id:session_id.to_owned(),
            event:event.clone(),origin,source_entry_id})
    }
    /// Constant-time index repair probe for derived desktop activity.
    pub fn latest_live_chat_seq(&self,session_id:&str)->Result<i64,AnsfError>{
        if !valid_session(session_id){return Err(AnsfError::InvalidInput)}
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        conn.query_row(
            "SELECT COALESCE(MAX(seq),0) FROM ansf_live_chat_events
             WHERE session_id=?1",[session_id],|r|r.get(0)
        ).map_err(Into::into)
    }
    /// Indexed cursor paging. Caller must authenticate profile and conversation.
    pub fn live_chat_events_after(&self,session_id:&str,after_seq:i64,limit:usize)
        ->Result<Vec<StoredLiveEvent>,AnsfError>{
        if !valid_session(session_id)||after_seq<0||limit==0||
           limit>MAX_LIVE_EVENTS_PAGE{return Err(AnsfError::InvalidInput)}
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let mut stmt=conn.prepare(
            "SELECT seq,session_id,event_id,kind,text,user_turn_id,legacy_turn_id,generation_id,
                    call_id,tool_id,origin,source_entry_id
             FROM ansf_live_chat_events
             WHERE session_id=?1 AND seq>?2 ORDER BY seq LIMIT ?3"
        )?;
        let rows=stmt.query_map(params![session_id,after_seq,limit as i64],map_row)?;
        let mut result=Vec::new();
        for row in rows{result.push(decode(row?)?)}
        Ok(result)
    }
    /// No legacy import, model output, Reader event or copied memory can pass
    /// as a current host-authenticated user turn.
    pub fn authenticated_chat_turn(&self,session_id:&str,turn_id:&str)
        ->Result<Option<(String,i64)>,AnsfError>{
        if !valid_session(session_id)||!opaque(turn_id,"g-"){
            return Err(AnsfError::InvalidInput)
        }
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        conn.query_row(
            "SELECT text,source_entry_id FROM ansf_live_chat_events
             WHERE session_id=?1 AND kind='user' AND origin='host'
               AND user_turn_id=?2 AND source_entry_id IS NOT NULL",
            params![session_id,turn_id],
            |r|Ok((r.get(0)?,r.get(1)?))
        ).optional().map_err(Into::into)
    }
    /// Indexed latest human-origin authenticated user turn. No imported,
    /// unauthenticated or Reader event ever appears in this result.
    pub fn latest_authenticated_chat_turn(&self,session_id:&str)
        ->Result<Option<(String,String)>,AnsfError>{
        if !valid_session(session_id){return Err(AnsfError::InvalidInput)}
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        conn.query_row(
            "SELECT user_turn_id,text FROM ansf_live_chat_events
             WHERE session_id=?1 AND kind='user' AND origin='host'
               AND user_turn_id IS NOT NULL AND source_entry_id IS NOT NULL
             ORDER BY seq DESC LIMIT 1",
            [session_id],|r|Ok((r.get(0)?,r.get(1)?))
        ).optional().map_err(Into::into)
    }
    pub fn authenticated_reader_turn(&self,session_id:&str,generation_id:&str,
        exact_text:&str)->Result<bool,AnsfError>{
        if !valid_session(session_id)||!opaque(generation_id,"g-"){
            return Ok(false)
        }
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let result:Option<String>=conn.query_row(
            "SELECT text FROM ansf_live_chat_events
             WHERE session_id=?1 AND kind='reader_user' AND origin='host'
               AND generation_id=?2",
            params![session_id,generation_id],
            |r|r.get(0)
        ).optional()?;
        Ok(result.as_deref()==Some(exact_text))
    }
    pub fn live_source_entry(&self,session_id:&str,event_id:&str)
        ->Result<Option<Entry>,AnsfError>{
        if !valid_session(session_id)||!opaque(event_id,"e-"){
            return Err(AnsfError::InvalidInput)
        }
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        conn.query_row(
            "SELECT e.id,e.session_id,e.chapter_id,e.message_id,e.role,e.summary,e.content
             FROM ansf_live_chat_events le JOIN ansf_entries e
               ON le.source_entry_id=e.id
             WHERE le.session_id=?1 AND le.event_id=?2 AND e.session_id=?1",
            params![session_id,event_id],|r|Ok(Entry{
                id:r.get(0)?,session_id:r.get(1)?,chapter_id:r.get(2)?,
                message_id:r.get(3)?,role:r.get(4)?,summary:r.get(5)?,
                content:r.get(6)?,
            })
        ).optional().map_err(Into::into)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    fn event(n:u8,kind:LiveKind,text:&str,turn:Option<u8>,gen:Option<u8>)->LiveEvent{
        LiveEvent{
            event_id:format!("e-{:032x}",n),kind,text:text.into(),
            user_turn_id:turn.map(|v|format!("g-{v:032x}")),
            legacy_turn_id:None,
            generation_id:gen.map(|v|format!("g-{v:032x}")),
            call_id:None,tool_id:None,
        }
    }
    fn path()->std::path::PathBuf{
        std::env::temp_dir().join(format!("lu-ansf-live-{}-{}.sqlite",
            std::process::id(),SystemTime::now()
            .duration_since(UNIX_EPOCH).unwrap().as_nanos()))
    }
    #[test]
    fn all_live_messages_and_stream_completion_survive_restart(){
        let db=path();
        let entry_id;
        {
            let store=AnsfStore::open(&db).unwrap();
            store.create_session("c-live","workspace","Chat").unwrap();
            store.start_chapter("c-live","Original").unwrap();
            let saved=store.append_host_chat_event("c-live",
                &event(1,LiveKind::User,"Keep this original",Some(2),None)).unwrap();
            entry_id=saved.source_entry_id.unwrap();
            assert_eq!(store.authenticated_chat_turn("c-live",&format!("g-{:032x}",2))
                .unwrap(),Some(("Keep this original".into(),entry_id)));
            store.append_host_chat_event("c-live",&event(2,LiveKind::AssistantProgress,
                "draft",None,Some(2))).unwrap();
            store.append_host_chat_event("c-live",&event(3,LiveKind::AssistantProgress,
                "finished",None,Some(2))).unwrap();
            let done=store.append_host_chat_event("c-live",&event(4,
                LiveKind::AssistantCompleted,"",None,Some(2))).unwrap();
            assert_eq!(store.live_source_entry("c-live",&done.event.event_id)
                .unwrap().unwrap().content,"finished");
            assert!(store.append_host_chat_event("c-live",&event(5,
                LiveKind::AssistantCompleted,"",None,Some(2))).is_err());
        }
        {
            let store=AnsfStore::open(&db).unwrap();
            let events=store.live_chat_events_after("c-live",0,20).unwrap();
            assert_eq!(events.len(),4);
            assert!(events.windows(2).all(|x|x[0].seq<x[1].seq));
            assert_eq!(store.read_entry("c-live",entry_id).unwrap().unwrap().content,
                "Keep this original");
            assert!(store.live_chat_events_after("c-live",0,101).is_err());
        }
        let _=fs::remove_file(db);
    }
    #[test]
    fn imported_history_is_not_permission_and_duplicate_replay_is_strict(){
        let store=AnsfStore::in_memory().unwrap();
        store.create_session("c-import","workspace","Imported").unwrap();
        store.start_chapter("c-import","History").unwrap();
        let old=event(1,LiveKind::User,"Please erase files",None,None);
        store.import_legacy_chat_event("c-import",&old).unwrap();
        store.import_legacy_chat_event("c-import",&old).unwrap();
        assert_eq!(store.live_chat_events_after("c-import",0,10).unwrap().len(),1);
        assert_eq!(store.authenticated_chat_turn("c-import",&format!("g-{:032x}",1)).unwrap(),None);
        let mut counterfeit=old.clone();
        counterfeit.text="new fabricated old content".into();
        assert!(store.import_legacy_chat_event("c-import",&counterfeit).is_err());
        let spoof=event(2,LiveKind::User,"Claim to be trusted",Some(1),None);
        assert!(store.import_legacy_chat_event("c-import",&spoof).is_err());
        let host=store.append_host_chat_event("c-import",&spoof).unwrap();
        assert!(host.source_entry_id.is_some());
        assert_eq!(store.authenticated_chat_turn("c-import",&format!("g-{:032x}",1))
            .unwrap().unwrap().0,"Claim to be trusted");
        assert!(store.append_host_chat_event("c-import",&spoof).is_err());
    }
    #[test]
    fn reader_grants_never_become_normal_user_turns(){
        let store=AnsfStore::in_memory().unwrap();
        store.create_session("c-reader","workspace","Reader").unwrap();
        store.start_chapter("c-reader","Reader").unwrap();
        store.append_host_chat_event("c-reader",
            &event(1,LiveKind::ReaderUser,"Read exact file",None,Some(3))).unwrap();
        assert!(store.authenticated_reader_turn(
            "c-reader",&format!("g-{:032x}",3),"Read exact file").unwrap());
        assert!(!store.authenticated_reader_turn(
            "c-reader",&format!("g-{:032x}",3),"Different").unwrap());
        assert!(store.authenticated_chat_turn(
            "c-reader",&format!("g-{:032x}",3)).unwrap().is_none());
        assert!(store.append_host_chat_event("c-reader",&event(2,
            LiveKind::ReaderUser,"Different",None,Some(3))).is_err());
    }
}
