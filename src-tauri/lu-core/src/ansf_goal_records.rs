//! M2 native relational ANSF goal records, no JSON event serialization.
//!
//! Old ansf_goals::mirror_goal_event remains a legacy migration adapter;
//! current user decisions are persisted directly as typed SQLite columns.
//! Goal/clarification memory is data, not an execution capability.
use crate::ansf::{AnsfError,AnsfStore};
use crate::ansf_goals::{GoalEvent,GoalEventKind,GoalSnapshot,project_goal_events};
use rusqlite::{params,Connection,OptionalExtension};

pub(crate) fn initialize_goal_record_schema(conn:&Connection)->Result<(),AnsfError>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_goal_records (
           seq INTEGER PRIMARY KEY AUTOINCREMENT,
           session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
           event_id TEXT NOT NULL UNIQUE,
           goal_id TEXT NOT NULL,
           question_id TEXT,
           kind TEXT NOT NULL CHECK(kind IN ('set_goal','ask_question','respond_question')),
           text TEXT NOT NULL,
           choice TEXT CHECK(choice IS NULL OR choice IN ('yes','no','not_now')),
           comment TEXT,
           source_origin TEXT NOT NULL CHECK(source_origin IN ('host','legacy')),
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE INDEX IF NOT EXISTS ansf_goal_records_session_seq
            ON ansf_goal_records(session_id,seq);
         CREATE TRIGGER IF NOT EXISTS ansf_goal_records_immutable_update
            BEFORE UPDATE ON ansf_goal_records
            BEGIN SELECT RAISE(ABORT,'ANSF goal records are immutable'); END;
         CREATE TRIGGER IF NOT EXISTS ansf_goal_records_immutable_delete
            BEFORE DELETE ON ansf_goal_records
            BEGIN SELECT RAISE(ABORT,'ANSF goal records are immutable'); END;"
    )?;
    Ok(())
}
fn kind_value(k:GoalEventKind)->&'static str{
    match k{GoalEventKind::SetGoal=>"set_goal",
        GoalEventKind::AskQuestion=>"ask_question",
        GoalEventKind::RespondQuestion=>"respond_question"}
}
fn parse_kind(value:&str)->Result<GoalEventKind,AnsfError>{
    match value{"set_goal"=>Ok(GoalEventKind::SetGoal),
        "ask_question"=>Ok(GoalEventKind::AskQuestion),
        "respond_question"=>Ok(GoalEventKind::RespondQuestion),
        _=>Err(AnsfError::InvalidInput)}
}
fn load_rows(conn:&Connection,session_id:&str)->Result<Vec<GoalEvent>,AnsfError>{
    let mut stmt=conn.prepare(
        "SELECT event_id,goal_id,question_id,kind,text,choice,comment
         FROM ansf_goal_records WHERE session_id=?1 ORDER BY seq"
    )?;
    let rows=stmt.query_map([session_id],|r|{
        Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,
            r.get::<_,Option<String>>(2)?,r.get::<_,String>(3)?,
            r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,
            r.get::<_,Option<String>>(6)?))
    })?;
    let mut result=Vec::new();
    for row in rows{
        let (event_id,goal_id,question_id,kind,text,choice,comment)=row?;
        let event=GoalEvent{event_id,goal_id,question_id,
            kind:parse_kind(&kind)?,text,choice,comment};
        event.validate()?;
        result.push(event);
        if result.len()>1024{return Err(AnsfError::InvalidInput)}
    }
    project_goal_events(&result)?;
    Ok(result)
}
impl AnsfStore{
    pub fn native_goal_records(&self,session_id:&str)->Result<Vec<GoalEvent>,AnsfError>{
        if session_id.is_empty()||session_id.len()>128||session_id.contains('\0'){
            return Err(AnsfError::InvalidInput)
        }
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        load_rows(&conn,session_id)
    }
    pub fn native_goal_snapshot(&self,session_id:&str)->Result<GoalSnapshot,AnsfError>{
        project_goal_events(&self.native_goal_records(session_id)?)
    }
    /// Appends a verified UI-host decision. Old imported goal answers must
    /// instead use import_observed_goal_record; they are historical data only.
    pub fn append_native_goal_record(&self,session_id:&str,event:&GoalEvent)
        ->Result<GoalSnapshot,AnsfError>{
        self.append_record(session_id,event,"host",false)
    }
    pub fn import_observed_goal_record(&self,session_id:&str,event:&GoalEvent)
        ->Result<GoalSnapshot,AnsfError>{
        self.append_record(session_id,event,"legacy",true)
    }
    fn append_record(&self,session_id:&str,event:&GoalEvent,origin:&str,idempotent:bool)
        ->Result<GoalSnapshot,AnsfError>{
        event.validate()?;
        if session_id.is_empty()||session_id.len()>128||session_id.contains('\0'){
            return Err(AnsfError::InvalidInput)
        }
        let mut conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let tx=conn.transaction()?;
        let existing:Option<(String,GoalEvent)>=tx.query_row(
            "SELECT source_origin,goal_id,question_id,kind,text,choice,comment
             FROM ansf_goal_records WHERE event_id=?1",
            [&event.event_id],|r|{
                Ok((r.get::<_,String>(0)?,
                    r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,
                    r.get::<_,String>(3)?,r.get::<_,String>(4)?,
                    r.get::<_,Option<String>>(5)?,r.get::<_,Option<String>>(6)?))
            }
        ).optional()?.map(|(saved_origin,goal_id,question_id,kind,text,choice,comment)|{
            Ok::<_,AnsfError>((saved_origin,GoalEvent{
                event_id:event.event_id.clone(),goal_id,question_id,
                kind:parse_kind(&kind)?,text,choice,comment,
            }))
        }).transpose()?;
        if let Some((saved_origin,old))=existing{
            if idempotent&&saved_origin==origin&&old==*event {
                return project_goal_events(&load_rows(&tx,session_id)?)
            }
            return Err(AnsfError::InvalidInput)
        }
        let mut prior=load_rows(&tx,session_id)?;
        prior.push(event.clone());
        let state=project_goal_events(&prior)?;
        tx.execute(
            "INSERT INTO ansf_goal_records(session_id,event_id,goal_id,question_id,
                kind,text,choice,comment,source_origin)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![session_id,event.event_id,event.goal_id,event.question_id,
                kind_value(event.kind),event.text,event.choice,event.comment,origin],
        )?;
        tx.commit()?;
        Ok(state)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn event(n:u8,k:GoalEventKind,q:Option<String>,text:&str,choice:Option<&str>)->GoalEvent{
        GoalEvent{event_id:format!("m-{n:032x}"),goal_id:format!("goal-{:032x}",1),
            question_id:q,kind:k,text:text.into(),
            choice:choice.map(str::to_owned),comment:None}
    }
    #[test]
    fn relational_goal_records_no_json_and_explicit_deferral(){
        let store=AnsfStore::in_memory().unwrap();
        store.create_session("c-1","local","Test").unwrap();
        let q=format!("q-{:032x}",8);
        store.append_native_goal_record("c-1",
            &event(1,GoalEventKind::SetGoal,None,"Build Lu",None)).unwrap();
        store.append_native_goal_record("c-1",
            &event(2,GoalEventKind::AskQuestion,Some(q.clone()),"Use tests?",None)).unwrap();
        let mut answer=event(3,GoalEventKind::RespondQuestion,
            Some(q.clone()),"",Some("not_now"));
        assert_eq!(store.append_native_goal_record("c-1",&answer)
            .unwrap().questions[0].status,"deferred");
        assert!(store.append_native_goal_record("c-1",&answer).is_err());
        answer=event(4,GoalEventKind::RespondQuestion,Some(q),"",Some("yes"));
        let state=store.append_native_goal_record("c-1",&answer).unwrap();
        assert_eq!(state.questions[0].status,"answered");
        assert!(store.native_goal_records("c-2").unwrap().is_empty());
        let old=event(5,GoalEventKind::SetGoal,None,"Legacy goal",None);
        assert!(store.import_observed_goal_record("c-1",&old).is_ok());
        assert!(store.import_observed_goal_record("c-1",&old).is_ok());
        let mut altered=old.clone();altered.text="Changed legacy".into();
        assert!(store.import_observed_goal_record("c-1",&altered).is_err());
    }
}
