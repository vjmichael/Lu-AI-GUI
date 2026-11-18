//! Phase 2 — durable, fail-closed human-directed replanning.
//! An uncertain previous worker is NEVER retried or used as an approval.
//! A replacement plan is a NEW draft with new human origin, review, owner,
//! resources and effect journal. A linked replan is not a tool grant.
use std::path::Path;
use rusqlite::{params,Connection,OpenFlags,OptionalExtension,TransactionBehavior};
use crate::conversation_authority::TrustedUserTurnVerifier;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ReplanParent {
    pub parent_plan_id:String,
    pub original_turn_id:String,
    pub parent_revision_sha:String,
    pub fresh_human_turn_id:String,
    pub reason:String,
}

fn valid(v:&str,prefix:&str)->bool{
    v.len()==prefix.len()+32 && v.starts_with(prefix)
        && v[prefix.len()..].bytes().all(|b|
            b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn valid_sha(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|
        b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn valid_reason(s:&str)->bool{
    matches!(s,"REVISE_DEPENDENCIES"|"CORRECT_RESULT"|"CHANGE_APPROACH"|"GOAL_CHANGED")
}
fn schema(db:&Connection)->Result<(),String>{
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS lu_agent_replan_intents(
           parent_plan_id TEXT NOT NULL REFERENCES lu_agent_plans(plan_id),
           parent_revision_sha TEXT NOT NULL,
           chat_id TEXT NOT NULL,
           original_turn_id TEXT NOT NULL,
           fresh_human_turn_id TEXT NOT NULL UNIQUE,
           reason TEXT NOT NULL,
           PRIMARY KEY(parent_plan_id,parent_revision_sha,fresh_human_turn_id)
         );
         CREATE TABLE IF NOT EXISTS lu_agent_replan_children(
           parent_plan_id TEXT NOT NULL,
           parent_revision_sha TEXT NOT NULL,
           fresh_human_turn_id TEXT NOT NULL,
           child_plan_id TEXT NOT NULL UNIQUE REFERENCES lu_agent_plans(plan_id),
           PRIMARY KEY(parent_plan_id,parent_revision_sha,fresh_human_turn_id),
           FOREIGN KEY(parent_plan_id,parent_revision_sha,fresh_human_turn_id)
             REFERENCES lu_agent_replan_intents(parent_plan_id,parent_revision_sha,fresh_human_turn_id)
         );
         CREATE TRIGGER IF NOT EXISTS lu_agent_replan_intent_no_update
           BEFORE UPDATE ON lu_agent_replan_intents BEGIN
             SELECT RAISE(ABORT,'Immutable replan intent'); END;
         CREATE TRIGGER IF NOT EXISTS lu_agent_replan_intent_no_delete
           BEFORE DELETE ON lu_agent_replan_intents BEGIN
             SELECT RAISE(ABORT,'Immutable replan intent'); END;
         CREATE TRIGGER IF NOT EXISTS lu_agent_replan_child_no_update
           BEFORE UPDATE ON lu_agent_replan_children BEGIN
             SELECT RAISE(ABORT,'Immutable replan child link'); END;
         CREATE TRIGGER IF NOT EXISTS lu_agent_replan_child_no_delete
           BEFORE DELETE ON lu_agent_replan_children BEGIN
             SELECT RAISE(ABORT,'Immutable replan child link'); END;"
    ).map_err(|e|format!("Cannot create original replan journal: {e}"))
}
fn open_writable(file:&Path)->Result<Connection,String>{
    let db=Connection::open(file).map_err(|e|format!("Original journal unavailable: {e}"))?;
    db.pragma_update(None,"foreign_keys","ON").map_err(|e|e.to_string())?;
    db.pragma_update(None,"synchronous","FULL").map_err(|e|e.to_string())?;
    schema(&db)?;
    Ok(db)
}
fn latest(db:&Connection,parent:&str)->Result<String,String>{
    db.query_row(
        "SELECT revision_sha256 FROM lu_agent_plan_revisions
         WHERE plan_id=?1 ORDER BY revision_index DESC LIMIT 1",
        params![parent],|r|r.get(0)
    ).map_err(|_|"Original plan has no current revision".into())
}
pub fn stage_human_replan(
    file:&Path,chat:&str,parent:&str,original_turn:&str,
    revision:&str,fresh_turn:&str,reason:&str,
    human:&dyn TrustedUserTurnVerifier,
)->Result<(),String>{
    if !valid(chat,"c-")||!valid(parent,"p-")
        ||!valid(original_turn,"g-")||!valid(fresh_turn,"g-")
        ||!valid_sha(revision)||!valid_reason(reason)
        ||original_turn==fresh_turn
        ||!human.verify_user_turn(chat,fresh_turn){
        return Err("Unverified original scope or fresh human replan turn".into())
    }
    let mut db=open_writable(file)?;
    let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e|e.to_string())?;
    let (old_chat,old_turn):(String,String)=tx.query_row(
        "SELECT conversation_id,human_turn_id FROM lu_agent_plans
         WHERE plan_id=?1",params![parent],|r|Ok((r.get(0)?,r.get(1)?))
    ).map_err(|_|"Original plan missing".to_string())?;
    if old_chat!=chat||old_turn!=original_turn||latest(&tx,parent)?!=revision {
        return Err("Replan original context or revision is stale".into())
    }
    let recorded:Option<(String,String,String,String,String)>=tx.query_row(
        "SELECT parent_plan_id,parent_revision_sha,chat_id,original_turn_id,reason
         FROM lu_agent_replan_intents WHERE fresh_human_turn_id=?1",
        params![fresh_turn],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
    ).optional().map_err(|e|e.to_string())?;
    if let Some(old)=recorded {
        if old!=(parent.into(),revision.into(),chat.into(),original_turn.into(),reason.into()){
            return Err("Fresh human replan intent already belongs to another plan".into())
        }
    }else{
        tx.execute(
            "INSERT INTO lu_agent_replan_intents(
             parent_plan_id,parent_revision_sha,chat_id,original_turn_id,
             fresh_human_turn_id,reason) VALUES(?1,?2,?3,?4,?5,?6)",
            params![parent,revision,chat,original_turn,fresh_turn,reason]
        ).map_err(|e|format!("Human replan intent conflict: {e}"))?;
    }
    tx.commit().map_err(|e|e.to_string())
}
pub fn bind_replan_child(
    file:&Path,chat:&str,parent:&str,revision:&str,
    fresh_turn:&str,child:&str,
)->Result<(),String>{
    if !valid(chat,"c-")||!valid(parent,"p-")||!valid(child,"p-")
        ||child==parent||!valid_sha(revision)||!valid(fresh_turn,"g-"){
        return Err("Invalid host-owned replan linkage".into())
    }
    let mut db=open_writable(file)?;
    let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e|e.to_string())?;
    let linked:Option<String>=tx.query_row(
        "SELECT reason FROM lu_agent_replan_intents
         WHERE parent_plan_id=?1 AND parent_revision_sha=?2
           AND chat_id=?3 AND fresh_human_turn_id=?4",
        params![parent,revision,chat,fresh_turn],|r|r.get(0)
    ).optional().map_err(|e|e.to_string())?;
    if linked.as_deref().is_none_or(|r|!valid_reason(r)) {
        return Err("No exact saved human replan intent".into())
    }
    if latest(&tx,parent)?!=revision {
        return Err("Parent revision changed during replan binding".into())
    }
    let child_origin:Option<(String,String)>=tx.query_row(
        "SELECT conversation_id,human_turn_id FROM lu_agent_plans WHERE plan_id=?1",
        params![child],|r|Ok((r.get(0)?,r.get(1)?))
    ).optional().map_err(|e|e.to_string())?;
    if child_origin!=Some((chat.into(),fresh_turn.into())) {
        return Err("Replacement is not the exact freshly authenticated draft".into())
    }
    let old:Option<String>=tx.query_row(
        "SELECT child_plan_id FROM lu_agent_replan_children
         WHERE parent_plan_id=?1 AND parent_revision_sha=?2
           AND fresh_human_turn_id=?3",
        params![parent,revision,fresh_turn],|r|r.get(0)
    ).optional().map_err(|e|e.to_string())?;
    match old {
        Some(existing) if existing!=child=>return Err("Replan already links a different child".into()),
        Some(_)=>{},
        None=>{
            tx.execute(
                "INSERT INTO lu_agent_replan_children(
                 parent_plan_id,parent_revision_sha,fresh_human_turn_id,child_plan_id)
                 VALUES(?1,?2,?3,?4)",
                params![parent,revision,fresh_turn,child]
            ).map_err(|e|format!("Replan lineage conflict: {e}"))?;
        }
    }
    tx.commit().map_err(|e|e.to_string())
}
/// Before *any* replacement-plan worker, detect a linked or interrupted
/// replan. A crash between intent and child linkage is NEVER a new root.
pub fn inspect_replan_parent(
    file:&Path,chat:&str,child:&str,
)->Result<Option<ReplanParent>,String>{
    if !valid(chat,"c-")||!valid(child,"p-"){
        return Err("Invalid replan read scope".into())
    }
    let db=Connection::open_with_flags(file,
        OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX
    ).map_err(|e|e.to_string())?;
    let table:Option<String>=db.query_row(
        "SELECT name FROM sqlite_master WHERE type='table'
         AND name='lu_agent_replan_intents'",
        [],|r|r.get(0)
    ).optional().map_err(|e|e.to_string())?;
    if table.is_none(){return Ok(None)}
    let (actual_chat,turn):(String,String)=db.query_row(
        "SELECT conversation_id,human_turn_id FROM lu_agent_plans
         WHERE plan_id=?1",params![child],
        |r|Ok((r.get(0)?,r.get(1)?))
    ).map_err(|_|"Replacement plan missing".to_string())?;
    if actual_chat!=chat {return Err("Replan is outside authenticated chat".into())}
    let row:Option<(String,String,String,String,String,String)>=db.query_row(
        "SELECT i.parent_plan_id,i.parent_revision_sha,i.original_turn_id,
                i.fresh_human_turn_id,i.reason,c.child_plan_id
         FROM lu_agent_replan_intents i
         LEFT JOIN lu_agent_replan_children c
           ON c.parent_plan_id=i.parent_plan_id
          AND c.parent_revision_sha=i.parent_revision_sha
          AND c.fresh_human_turn_id=i.fresh_human_turn_id
         WHERE i.chat_id=?1 AND i.fresh_human_turn_id=?2",
        params![chat,turn],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get::<_,Option<String>>(5)?.unwrap_or_default()))
    ).optional().map_err(|e|e.to_string())?;
    let Some((parent,revision,original,fresh,reason,linked))=row else{return Ok(None)};
    if linked!=child {
        return Err("Unlinked replan draft after interrupted transaction; reconcile first".into())
    }
    if !valid(&parent,"p-")||!valid(&original,"g-")
        ||!valid(&fresh,"g-")||!valid_sha(&revision)||!valid_reason(&reason){
        return Err("Damaged historical replan lineage".into())
    }
    Ok(Some(ReplanParent{
        parent_plan_id:parent,original_turn_id:original,
        parent_revision_sha:revision,fresh_human_turn_id:fresh,reason
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Human;
    impl TrustedUserTurnVerifier for Human {
        fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
            chat==&format!("c-{}","1".repeat(32))
                &&turn==&format!("g-{}","8".repeat(32))
        }
    }
    fn fixture()->(std::path::PathBuf,String,String,String,String,String,String){
        use std::time::{SystemTime,UNIX_EPOCH};
        let path=std::env::temp_dir().join(format!(
            "lu-phase2-replan-{}-{}.sqlite",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let chat=format!("c-{}","1".repeat(32));
        let parent=format!("p-{}","2".repeat(32));
        let child=format!("p-{}","3".repeat(32));
        let old=format!("g-{}","4".repeat(32));
        let fresh=format!("g-{}","8".repeat(32));
        let sha="a".repeat(64);
        let db=Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE lu_agent_plans(
              plan_id TEXT PRIMARY KEY,
              conversation_id TEXT NOT NULL,
              human_turn_id TEXT NOT NULL);
             CREATE TABLE lu_agent_plan_revisions(
              plan_id TEXT NOT NULL,
              revision_index INTEGER NOT NULL,
              revision_sha256 TEXT NOT NULL);
            "
        ).unwrap();
        db.execute("INSERT INTO lu_agent_plans VALUES(?1,?2,?3)",
            params![parent,chat,old]).unwrap();
        db.execute("INSERT INTO lu_agent_plans VALUES(?1,?2,?3)",
            params![child,chat,fresh]).unwrap();
        db.execute("INSERT INTO lu_agent_plan_revisions VALUES(?1,1,?2)",
            params![parent,sha]).unwrap();
        drop(db);
        (path,chat,parent,child,old,fresh,sha)
    }
    #[test]
    fn immutable_replan_intent_and_exact_child_survive_restart(){
        let (file,chat,parent,child,old,fresh,sha)=fixture();
        stage_human_replan(
            &file,&chat,&parent,&old,&sha,&fresh,"CHANGE_APPROACH",&Human
        ).unwrap();
        stage_human_replan(
            &file,&chat,&parent,&old,&sha,&fresh,"CHANGE_APPROACH",&Human
        ).unwrap();
        assert!(inspect_replan_parent(&file,&chat,&child).is_err());
        bind_replan_child(&file,&chat,&parent,&sha,&fresh,&child).unwrap();
        bind_replan_child(&file,&chat,&parent,&sha,&fresh,&child).unwrap();
        let history=inspect_replan_parent(&file,&chat,&child).unwrap().unwrap();
        assert_eq!(history.parent_plan_id,parent);
        assert_eq!(history.original_turn_id,old);
        assert_eq!(history.parent_revision_sha,sha);
        assert_eq!(history.reason,"CHANGE_APPROACH");
        assert!(stage_human_replan(
            &file,&chat,&parent,&old,&sha,&fresh,"AUTHORIZE_EXECUTION",&Human
        ).is_err());
        let db=Connection::open(&file).unwrap();
        assert!(db.execute("DELETE FROM lu_agent_replan_children",[]).is_err());
        assert!(db.execute(
            "UPDATE lu_agent_replan_intents SET reason='GOAL_CHANGED'",[]
        ).is_err());
        drop(db);
        std::fs::remove_file(file).unwrap();
    }
    #[test]
    fn source_revision_change_refuses_to_bind_a_stale_replan(){
        let (file,chat,parent,child,old,fresh,sha)=fixture();
        stage_human_replan(
            &file,&chat,&parent,&old,&sha,&fresh,"CORRECT_RESULT",&Human
        ).unwrap();
        let db=Connection::open(&file).unwrap();
        db.execute(
            "UPDATE lu_agent_plan_revisions SET revision_sha256=?1",
            params!["b".repeat(64)]
        ).unwrap();
        drop(db);
        assert!(bind_replan_child(
            &file,&chat,&parent,&sha,&fresh,&child
        ).is_err());
        assert!(inspect_replan_parent(&file,&chat,&child).is_err());
        std::fs::remove_file(file).unwrap();
    }
    #[test]
    fn host_id_and_reason_validation_cannot_accept_model_permission() {
        assert!(valid(&format!("p-{}","a".repeat(32)),"p-"));
        assert!(!valid(&format!("p-{}","A".repeat(32)),"p-"));
        assert!(!valid_sha(&"g".repeat(64)));
        assert!(valid_reason("CHANGE_APPROACH"));
        assert!(!valid_reason("AUTHORIZE_EXECUTION"));
    }
}
