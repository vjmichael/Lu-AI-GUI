//! AGENT-01J: read-only lineage and crash-orphan recovery, never execution authority.
use std::{collections::BTreeSet,fs,path::Path};
use rusqlite::{params,Connection,OpenFlags,OptionalExtension};
const MAX_DEPTH:usize=32;
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct LineageReport{
    pub status:&'static str,
    pub root_plan_id:Option<String>,
    pub immediate_parent_id:Option<String>,
    pub generations:u8,
    pub execution_authority:&'static str,
}
fn valid(v:&str,p:&str)->bool{
    v.len()==p.len()+32&&v.starts_with(p)&&v[p.len()..].bytes()
        .all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn sha(v:&str)->bool{
    v.len()==64&&v.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn exists(db:&Connection,t:&str)->Result<bool,String>{
    Ok(db.query_row("SELECT name FROM sqlite_master WHERE type='table' AND name=?1",
        params![t],|r|r.get::<_,String>(0)).optional()
        .map_err(|e|e.to_string())?.is_some())
}
fn result(status:&'static str,root:Option<String>,parent:Option<String>,n:usize)->LineageReport{
    LineageReport{status,root_plan_id:root,immediate_parent_id:parent,
        generations:n.min(MAX_DEPTH) as u8,execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION"}
}
fn read(db:&Connection,chat:&str,plan:&str)->Result<LineageReport,String>{
    if !valid(chat,"c-")||!valid(plan,"p-"){return Err("Invalid lineage scope".into())}
    if !exists(db,"lu_agent_plans")?{return Err("Missing plan origin table".into())}
    let intents=exists(db,"lu_agent_continuation_intents")?;
    let children=exists(db,"lu_agent_continuation_children")?;
    let mut current=plan.to_owned();
    let mut seen=BTreeSet::new();
    let mut first=None;
    for depth in 0..=MAX_DEPTH{
        if !seen.insert(current.clone()){
            return Ok(result("STOP_LINEAGE_CYCLE",None,first,depth))
        }
        let origin:Option<(String,String)>=db.query_row(
            "SELECT conversation_id,human_turn_id FROM lu_agent_plans WHERE plan_id=?1",
            params![current],|r|Ok((r.get(0)?,r.get(1)?))
        ).optional().map_err(|e|e.to_string())?;
        let Some((saved_chat,turn))=origin else{
            return Ok(result("STOP_MISSING_PLAN_ORIGIN",None,first,depth))
        };
        if saved_chat!=chat||!valid(&turn,"g-"){
            return Ok(result("STOP_CROSS_CHAT_ORIGIN",None,first,depth))
        }
        let link:Option<(String,String,i64,String)>=if children{
            db.query_row("SELECT parent_plan_id,revision_sha256,step_index,
                fresh_human_turn_id FROM lu_agent_continuation_children
                WHERE child_plan_id=?1",params![current],
                |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))
            ).optional().map_err(|e|e.to_string())?
        }else{None};
        let Some((parent,digest,step,fresh))=link else{
            if intents{
                let reserved:i64=db.query_row(
                    "SELECT COUNT(*) FROM lu_agent_continuation_intents
                     WHERE fresh_human_turn_id=?1",params![turn],|r|r.get(0)
                ).map_err(|e|e.to_string())?;
                if reserved!=0{return Ok(result("STOP_ORPHAN_CONTINUATION",None,first,depth))}
            }
            return if depth==0{
                Ok(result("ROOT_PLAN_NO_CONTINUATION",Some(current),None,0))
            }else{
                Ok(result("LINEAGE_RECORDED_REVIEW_ONLY",Some(current),first,depth))
            }
        };
        if depth==MAX_DEPTH{return Ok(result("STOP_DEPTH_LIMIT",None,first,depth))}
        if !valid(&parent,"p-")||!sha(&digest)||!(1..=31).contains(&step)
            ||!valid(&fresh,"g-")||fresh!=turn||parent==current{
            return Ok(result("STOP_DAMAGED_LINEAGE",None,first,depth))
        }
        if first.is_none(){first=Some(parent.clone())}
        if !intents{
            return Ok(result("STOP_MISSING_CONTINUATION_INTENT",None,first,depth))
        }
        let state:Option<String>=db.query_row(
            "SELECT state FROM lu_agent_continuation_intents
             WHERE plan_id=?1 AND revision_sha256=?2 AND step_index=?3
               AND fresh_human_turn_id=?4",
            params![parent,digest,step,fresh],|r|r.get(0)
        ).optional().map_err(|e|e.to_string())?;
        if state.as_deref()!=Some("AWAITING_NEW_HOST_REVIEW"){
            return Ok(result("STOP_MISSING_CONTINUATION_INTENT",None,first,depth))
        }
        current=parent;
    }
    Ok(result("STOP_DEPTH_LIMIT",None,first,MAX_DEPTH))
}
/// A recorded ancestry link does not prove live source freshness, reviewed
/// execution, worker process termination, or a new human permission.
pub fn inspect_continuation_lineage(
    file:&Path,chat:&str,plan:&str
)->Result<LineageReport,String>{
    let meta=fs::symlink_metadata(file).map_err(|_|"Missing plan journal")?;
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>64*1024*1024{
        return Err("Untrusted plan journal".into())
    }
    let db=Connection::open_with_flags(file,
        OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX
    ).map_err(|e|e.to_string())?;
    read(&db,chat,plan)
}
#[cfg(test)]
mod tests{
    use super::*;
    fn id(p:&str,n:u8)->String{format!("{p}{n:032x}")}
    fn fixture()->Connection{
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE lu_agent_plans(
            plan_id TEXT PRIMARY KEY,conversation_id TEXT,human_turn_id TEXT);
            CREATE TABLE lu_agent_continuation_intents(
            plan_id TEXT,revision_sha256 TEXT,step_index INTEGER,
            fresh_human_turn_id TEXT,state TEXT);
            CREATE TABLE lu_agent_continuation_children(
            parent_plan_id TEXT,revision_sha256 TEXT,step_index INTEGER,
            fresh_human_turn_id TEXT,child_plan_id TEXT UNIQUE);").unwrap();
        db
    }
    fn plan(db:&Connection,p:&str,c:&str,t:&str){
        db.execute("INSERT INTO lu_agent_plans VALUES(?1,?2,?3)",
            params![p,c,t]).unwrap();
    }
    fn link(db:&Connection,p:&str,c:&str,t:&str){
        let digest="a".repeat(64);
        db.execute("INSERT INTO lu_agent_continuation_intents VALUES(
            ?1,?2,1,?3,'AWAITING_NEW_HOST_REVIEW')",
            params![p,digest,t]).unwrap();
        db.execute("INSERT INTO lu_agent_continuation_children VALUES(
            ?1,?2,1,?3,?4)",params![p,digest,t,c]).unwrap();
    }
    #[test]
    fn root_and_two_generation_lineage_remain_review_only(){
        let db=fixture();let chat=id("c-",1);
        let (a,b,c)=(id("p-",2),id("p-",3),id("p-",4));
        let (ta,tb,tc)=(id("g-",5),id("g-",6),id("g-",7));
        for (p,t) in [(&a,&ta),(&b,&tb),(&c,&tc)]{plan(&db,p,&chat,t)}
        assert_eq!(read(&db,&chat,&a).unwrap().status,"ROOT_PLAN_NO_CONTINUATION");
        link(&db,&a,&b,&tb);link(&db,&b,&c,&tc);
        let r=read(&db,&chat,&c).unwrap();
        assert_eq!(r.status,"LINEAGE_RECORDED_REVIEW_ONLY");
        assert_eq!(r.root_plan_id,Some(a));
        assert_eq!(r.immediate_parent_id,Some(b));
        assert_eq!(r.generations,2);
        assert_eq!(r.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
    }
    #[test]
    fn orphan_and_cross_chat_child_are_blocked(){
        let db=fixture();let (chat,other)=(id("c-",1),id("c-",2));
        let (a,b)=(id("p-",3),id("p-",4));
        let (ta,tb)=(id("g-",5),id("g-",6));
        plan(&db,&a,&chat,&ta);plan(&db,&b,&chat,&tb);
        db.execute("INSERT INTO lu_agent_continuation_intents VALUES(
            ?1,?2,1,?3,'AWAITING_NEW_HOST_REVIEW')",
            params![a,"a".repeat(64),tb]).unwrap();
        assert_eq!(read(&db,&chat,&b).unwrap().status,"STOP_ORPHAN_CONTINUATION");
        assert_eq!(read(&db,&other,&b).unwrap().status,"STOP_CROSS_CHAT_ORIGIN");
    }
    #[test]
    fn cycle_and_corrupt_revision_do_not_become_recovery(){
        let db=fixture();let chat=id("c-",1);
        let (a,b)=(id("p-",2),id("p-",3));
        let (ta,tb)=(id("g-",4),id("g-",5));
        plan(&db,&a,&chat,&ta);plan(&db,&b,&chat,&tb);
        link(&db,&a,&b,&tb);link(&db,&b,&a,&ta);
        assert_eq!(read(&db,&chat,&a).unwrap().status,"STOP_LINEAGE_CYCLE");
        db.execute("UPDATE lu_agent_continuation_children
            SET revision_sha256='BAD' WHERE child_plan_id=?1",params![a]).unwrap();
        assert_eq!(read(&db,&chat,&a).unwrap().status,"STOP_DAMAGED_LINEAGE");
    }
}
