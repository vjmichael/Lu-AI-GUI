//! M2: durable, human-origin goal and clarification events in an ANSF book.
//! This is task *context*, never execution authority. The live desktop
//! ChatStore event is the source. This SQLite table is its idempotent,
 //! source-checked ANSF projection, not a competing human-turn journal.
use crate::ansf::{AnsfError,AnsfStore};
use rusqlite::{params,Connection,OptionalExtension};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};

const MAX_EVENTS:usize=1024;
const MAX_QUESTIONS:usize=64;

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum GoalEventKind { SetGoal, AskQuestion, RespondQuestion }

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalEvent {
    pub event_id:String,
    pub goal_id:String,
    pub question_id:Option<String>,
    pub kind:GoalEventKind,
    pub text:String,
    pub choice:Option<String>,
    pub comment:Option<String>,
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Clarification {
    pub question_id:String,
    pub question:String,
    pub status:String,
    pub choice:Option<String>,
    pub comment:Option<String>,
    pub opened_event_id:String,
    pub last_event_id:String,
}
#[derive(Clone,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct GoalSnapshot {
    pub goal_id:Option<String>,
    pub goal:String,
    pub revision_event_id:Option<String>,
    pub questions:Vec<Clarification>,
}
fn opaque_id(id:&str,prefix:&str)->bool {
    id.len()==prefix.len()+32 && id.starts_with(prefix) &&
    id[prefix.len()..].bytes().all(|c|c.is_ascii_hexdigit()&&!c.is_ascii_uppercase())
}
fn text_ok(value:&str,max:usize)->bool{
    !value.trim().is_empty()&&value.len()<=max&&!value.contains('\0')
}
impl GoalEvent {
    pub fn validate(&self)->Result<(),AnsfError>{
        if !opaque_id(&self.event_id,"m-")||
           !opaque_id(&self.goal_id,"goal-"){
            return Err(AnsfError::InvalidInput)
        }
        if self.comment.as_ref().is_some_and(|c|c.len()>1024||c.contains('\0')){
            return Err(AnsfError::InvalidInput)
        }
        match self.kind {
            GoalEventKind::SetGoal=>{
                if self.question_id.is_some()||!text_ok(&self.text,4096)||
                   self.choice.is_some()||self.comment.is_some(){
                    return Err(AnsfError::InvalidInput)
                }
            },
            GoalEventKind::AskQuestion=>{
                if !self.question_id.as_deref().is_some_and(|q|opaque_id(q,"q-"))||
                   !text_ok(&self.text,1024)||self.choice.is_some()||
                   self.comment.is_some(){return Err(AnsfError::InvalidInput)}
            },
            GoalEventKind::RespondQuestion=>{
                if !self.question_id.as_deref().is_some_and(|q|opaque_id(q,"q-"))||
                   !self.text.is_empty()||
                   !matches!(self.choice.as_deref(),Some("yes"|"no"|"not_now")){
                    return Err(AnsfError::InvalidInput)
                }
            },
        }
        Ok(())
    }
}
/// Project only host-journaled events. Answered questions remain immutable;
/// Not now is a deferral, not implied consent and may later be answered.
/// A revised goal supersedes pending/deferred questions to avoid stale prompts.
pub fn project_goal_events(events:&[GoalEvent])->Result<GoalSnapshot,AnsfError>{
    if events.len()>MAX_EVENTS{return Err(AnsfError::InvalidInput);}
    let mut seen=std::collections::BTreeSet::new();
    let mut context=GoalSnapshot::default();
    for event in events{
        event.validate()?;
        if !seen.insert(event.event_id.as_str()){return Err(AnsfError::InvalidInput);}
        match event.kind{
            GoalEventKind::SetGoal=>{
                if context.goal_id.as_deref()!=Some(event.goal_id.as_str()) {
                    context.questions.clear();
                }else{
                    for q in &mut context.questions{
                        if q.status=="pending"||q.status=="deferred"{
                            q.status="superseded".into();
                        }
                    }
                }
                context.goal_id=Some(event.goal_id.clone());
                context.goal=event.text.clone();
                context.revision_event_id=Some(event.event_id.clone());
            },
            GoalEventKind::AskQuestion=>{
                if context.goal_id.as_deref()!=Some(event.goal_id.as_str())||
                   context.questions.len()>=MAX_QUESTIONS||
                   context.questions.iter().any(|q|Some(q.question_id.as_str())==
                       event.question_id.as_deref()){
                    return Err(AnsfError::InvalidInput)
                }
                context.questions.push(Clarification{
                    question_id:event.question_id.clone().unwrap_or_default(),
                    question:event.text.clone(),status:"pending".into(),
                    choice:None,comment:None,
                    opened_event_id:event.event_id.clone(),
                    last_event_id:event.event_id.clone(),
                });
            },
            GoalEventKind::RespondQuestion=>{
                if context.goal_id.as_deref()!=Some(event.goal_id.as_str()){
                    return Err(AnsfError::InvalidInput)
                }
                let q=context.questions.iter_mut().find(|q|
                    Some(q.question_id.as_str())==event.question_id.as_deref())
                    .ok_or(AnsfError::InvalidInput)?;
                if !matches!(q.status.as_str(),"pending"|"deferred"){
                    return Err(AnsfError::InvalidInput)
                }
                q.status=if event.choice.as_deref()==Some("not_now"){
                    "deferred".into()
                }else{"answered".into()};
                q.choice=event.choice.clone();
                q.comment=event.comment.clone();
                q.last_event_id=event.event_id.clone();
            }
        }
    }
    Ok(context)
}
pub(crate) fn initialize_goal_schema(conn:&Connection)->Result<(),AnsfError>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_goal_events (
            seq INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
            event_id TEXT NOT NULL UNIQUE,
            source_kind TEXT NOT NULL CHECK(source_kind='desktop_chat_journal'),
            event_json TEXT NOT NULL,
            source_sha256 TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS ansf_goal_events_session_seq
            ON ansf_goal_events(session_id,seq);"
    )?;
    Ok(())
}
fn sha(data:&str)->String{format!("{:x}",Sha256::digest(data.as_bytes()))}
impl AnsfStore {
    /// Replay exactly one stable source event. A partial JSONL -> ANSF sync
    /// is repaired by replay after restart; a conflicting duplicate fails.
    pub fn mirror_goal_event(&self,session_id:&str,event:&GoalEvent)->Result<(),AnsfError>{
        event.validate()?;
        if session_id.is_empty()||session_id.len()>128||session_id.contains('\0'){
            return Err(AnsfError::InvalidInput)
        }
        let json=serde_json::to_string(event).map_err(|_|AnsfError::InvalidInput)?;
        let digest=sha(&json);
        let mut conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let tx=conn.transaction()?;
        let prior:Option<(String,String)>=tx.query_row(
            "SELECT session_id,source_sha256 FROM ansf_goal_events WHERE event_id=?1",
            [&event.event_id],
            |r|Ok((r.get(0)?,r.get(1)?)),
        ).optional()?;
        if let Some((original_session,original_digest))=prior {
            if original_session==session_id && original_digest==digest {
                return Ok(());
            }
            return Err(AnsfError::InvalidInput)
        }
        let mut events=Vec::new();
        {
            let mut stmt=tx.prepare(
                "SELECT event_json,source_sha256 FROM ansf_goal_events
                 WHERE session_id=?1 ORDER BY seq"
            )?;
            let iter=stmt.query_map([session_id],|r|
                Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?;
            for row in iter{
                let (stored,digest)=row?;
                if sha(&stored)!=digest{return Err(AnsfError::InvalidInput);}
                events.push(serde_json::from_str::<GoalEvent>(&stored)
                    .map_err(|_|AnsfError::InvalidInput)?);
            }
        }
        events.push(event.clone());
        project_goal_events(&events)?;
        tx.execute(
            "INSERT INTO ansf_goal_events
             (session_id,event_id,source_kind,event_json,source_sha256)
             VALUES (?1,?2,'desktop_chat_journal',?3,?4)",
            params![session_id,event.event_id,json,digest],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn goal_events(&self,session_id:&str)->Result<Vec<GoalEvent>,AnsfError>{
        if session_id.is_empty()||session_id.len()>128||session_id.contains('\0'){
            return Err(AnsfError::InvalidInput)
        }
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let mut stmt=conn.prepare(
            "SELECT event_json,source_sha256 FROM ansf_goal_events
             WHERE session_id=?1 AND source_kind='desktop_chat_journal' ORDER BY seq"
        )?;
        let rows=stmt.query_map([session_id],|r|
            Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?;
        let mut result=Vec::new();
        for row in rows{
            let (value,digest)=row?;
            if sha(&value)!=digest{return Err(AnsfError::InvalidInput);}
            result.push(serde_json::from_str::<GoalEvent>(&value)
                .map_err(|_|AnsfError::InvalidInput)?);
        }
        project_goal_events(&result)?;
        Ok(result)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn event(n:u8,k:GoalEventKind,q:Option<String>,text:&str,choice:Option<&str>)->GoalEvent{
        GoalEvent{event_id:format!("m-{:032x}",n),goal_id:format!("goal-{:032x}",1),
            question_id:q,kind:k,text:text.into(),choice:choice.map(str::to_owned),comment:None}
    }
    #[test]
    fn goal_revision_question_deferral_and_answer(){
        let q=format!("q-{:032x}",2);
        let records=[
            event(1,GoalEventKind::SetGoal,None,"Create a screenplay",None),
            event(2,GoalEventKind::AskQuestion,Some(q.clone()),"Documentary?",None),
            event(3,GoalEventKind::RespondQuestion,Some(q.clone()),"",Some("not_now")),
            event(4,GoalEventKind::RespondQuestion,Some(q),"",Some("yes")),
        ];
        let view=project_goal_events(&records).unwrap();
        assert_eq!(view.questions[0].status,"answered");
        assert_eq!(view.questions[0].choice.as_deref(),Some("yes"));
        let mut illegal=records.to_vec();
        illegal.push(event(5,GoalEventKind::RespondQuestion,
            illegal[1].question_id.clone(),"",Some("no")));
        assert!(project_goal_events(&illegal).is_err());
        let revised=event(6,GoalEventKind::SetGoal,None,"Create a feature film",None);
        illegal=records[..3].to_vec(); illegal.push(revised);
        assert_eq!(project_goal_events(&illegal).unwrap().questions[0].status,"superseded");
    }
    #[test]
    fn duplicate_conflict_and_crash_replay_are_safe(){
        let store=AnsfStore::in_memory().unwrap();
        store.ensure_session("c-1","local-owner","Test").unwrap();
        let first=event(1,GoalEventKind::SetGoal,None,"Start the project",None);
        store.mirror_goal_event("c-1",&first).unwrap();
        store.mirror_goal_event("c-1",&first).unwrap();
        assert_eq!(store.goal_events("c-1").unwrap().len(),1);
        let mut changed=first.clone();changed.text="injected replacement".into();
        assert!(store.mirror_goal_event("c-1",&changed).is_err());
        assert!(store.mirror_goal_event("c-2",&first).is_err());
    }
}
