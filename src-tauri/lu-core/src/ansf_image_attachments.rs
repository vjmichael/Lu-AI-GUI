//! CP-01B: one host-owned ordered image manifest inside the original ANSF book.
//! Durable rows carry only metadata, never image bytes or paths. A draft row
//! is mutable by the human host until a transaction binds it to an original
//! authenticated user event. Saved rows are immutable and confer no tool grant.
use crate::ansf::{AnsfError,AnsfStore};
use crate::ansf_artifact_links::{ArtifactLink,insert_artifact_link};
use crate::multimodal_saved_turn::{MAX_IMAGES,MAX_IMAGE_BYTES,MAX_TOTAL_IMAGE_BYTES};
use rusqlite::{params,Connection,Transaction,OptionalExtension};

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ImageAttachment {
    pub attachment_id:String,
    pub display_name:String,
    pub media_type:String,
    pub bytes:u64,
    pub sha256:String,
    /// Local Observe pixels are memory-only; this row is metadata, not a
    /// promise that those pixels can be recovered or used after restart.
    pub observed:bool,
}

fn opaque(id:&str,prefix:&str)->bool{
    id.len()==prefix.len()+32 && id.starts_with(prefix) &&
    id[prefix.len()..].bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
pub fn valid_attachment_id(id:&str)->bool{opaque(id,"img-")}
fn valid_chat(id:&str)->bool{
    !id.is_empty() && id.len()<=128 &&
    id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn valid(a:&ImageAttachment)->bool {
    valid_attachment_id(&a.attachment_id) &&
    !a.display_name.trim().is_empty() && a.display_name.len()<=150 &&
    !a.display_name.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':')) &&
    matches!(a.media_type.as_str(),"image/png"|"image/jpeg") &&
    (!a.observed||a.media_type=="image/png") &&
    (1..=MAX_IMAGE_BYTES as u64).contains(&a.bytes) &&
    a.sha256.len()==64 && a.sha256.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn validate_manifest(rows:&[ImageAttachment])->Result<(),AnsfError>{
    let mut refs=std::collections::HashSet::new();
    let mut digests=std::collections::HashSet::new();
    let mut total=0u64;
    if rows.len()>MAX_IMAGES{return Err(AnsfError::InvalidInput)}
    for a in rows{
        if !valid(a)||!refs.insert(&a.attachment_id)||!digests.insert(&a.sha256){
            return Err(AnsfError::InvalidInput)
        }
        total=total.checked_add(a.bytes).ok_or(AnsfError::InvalidInput)?;
    }
    if total>MAX_TOTAL_IMAGE_BYTES as u64{return Err(AnsfError::InvalidInput)}
    Ok(())
}

pub(crate) fn initialize_image_attachment_schema(conn:&Connection)->Result<(),AnsfError>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_image_attachments(
            seq INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
            attachment_id TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL, media_type TEXT NOT NULL,
            bytes INTEGER NOT NULL CHECK(bytes>0 AND bytes<=8388608),
            sha256 TEXT NOT NULL,
            observed INTEGER NOT NULL CHECK(observed IN (0,1)),
            turn_id TEXT,
            UNIQUE(session_id,turn_id,sha256)
         );
         CREATE INDEX IF NOT EXISTS ansf_image_attachment_manifest
           ON ansf_image_attachments(session_id,turn_id,seq);
         CREATE TRIGGER IF NOT EXISTS ansf_image_saved_no_update
           BEFORE UPDATE ON ansf_image_attachments WHEN OLD.turn_id IS NOT NULL
           BEGIN SELECT RAISE(ABORT,'saved image attachments are immutable'); END;
         CREATE TRIGGER IF NOT EXISTS ansf_image_saved_no_delete
           BEFORE DELETE ON ansf_image_attachments WHEN OLD.turn_id IS NOT NULL
           BEGIN SELECT RAISE(ABORT,'saved image attachments are immutable'); END;"
    )?;
    Ok(())
}
pub(crate) fn read_manifest(conn:&Connection,chat:&str,turn:Option<&str>)
    ->Result<Vec<ImageAttachment>,AnsfError>{
    if !valid_chat(chat)||turn.is_some_and(|id|!opaque(id,"g-")){
        return Err(AnsfError::InvalidInput)
    }
    let mut stmt=conn.prepare(
        "SELECT attachment_id,display_name,media_type,bytes,sha256,observed
         FROM ansf_image_attachments WHERE session_id=?1 AND turn_id IS ?2
         ORDER BY seq LIMIT 5"
    )?;
    let rows=stmt.query_map(params![chat,turn],|r|{
        Ok(ImageAttachment{
            attachment_id:r.get(0)?,display_name:r.get(1)?,media_type:r.get(2)?,
            bytes:r.get(3)?,sha256:r.get(4)?,observed:r.get(5)?,
        })
    })?.collect::<Result<Vec<_>,_>>()?;
    validate_manifest(&rows)?;
    Ok(rows)
}

/// Called only from the original live user event transaction. Neither stale
/// renderer refs nor a concurrent change can silently omit/reorder an image.
pub(crate) fn bind_draft(tx:&Transaction<'_>,chat:&str,turn:&str,entry:i64,
    expected:&[ImageAttachment])->Result<(),AnsfError>{
    if !opaque(turn,"g-")||entry<=0||read_manifest(tx,chat,None)?.as_slice()!=expected{
        return Err(AnsfError::InvalidInput)
    }
    for a in expected{
        if !a.observed{
            insert_artifact_link(tx,&ArtifactLink{
                session_id:chat.into(),entry_id:entry,artifact_id:a.attachment_id.clone(),
                display_name:a.display_name.clone(),media_type:a.media_type.clone(),
                expected_bytes:Some(a.bytes),expected_sha256:Some(a.sha256.clone()),
            })?;
        }
    }
    let changed=tx.execute(
        "UPDATE ansf_image_attachments SET turn_id=?2
         WHERE session_id=?1 AND turn_id IS NULL",params![chat,turn])?;
    if changed!=expected.len(){return Err(AnsfError::InvalidInput)}
    Ok(())
}
fn read_saved(conn:&Connection,chat:&str,turn:&str)->Result<Vec<ImageAttachment>,AnsfError>{
    let entry:Option<i64>=conn.query_row(
        "SELECT source_entry_id FROM ansf_live_chat_events WHERE session_id=?1
         AND user_turn_id=?2 AND kind='user' AND origin='host'
         AND source_entry_id IS NOT NULL",params![chat,turn],|r|r.get(0)).optional()?;
    let entry=entry.ok_or(AnsfError::InvalidInput)?;
    let manifest=read_manifest(conn,chat,Some(turn))?;
    // Independently require the original entry's artifact index to agree.
    // Observations must have no persisted ordinary attachment link.
    for a in &manifest{
        let linked:Option<(String,String,u64,String)>=conn.query_row(
            "SELECT display_name,media_type,expected_bytes,expected_sha256
             FROM ansf_artifact_links WHERE session_id=?1 AND entry_id=?2 AND artifact_id=?3",
            params![chat,entry,a.attachment_id],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        if a.observed {
            if linked.is_some(){return Err(AnsfError::InvalidInput)}
        }else if linked!=Some((a.display_name.clone(),a.media_type.clone(),a.bytes,a.sha256.clone())){
            return Err(AnsfError::InvalidInput)
        }
    }
    Ok(manifest)
}
impl AnsfStore{
    pub fn image_draft(&self,chat:&str)->Result<Vec<ImageAttachment>,AnsfError>{
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        read_manifest(&conn,chat,None)
    }
    /// Host verifies external bytes (or approved memory-only observation)
    /// before inserting. An ordinary duplicate returns its existing exact
    /// draft identity; an observation cannot be relabeled ordinary.
    pub fn admit_draft_image(&self,chat:&str,a:&ImageAttachment)
        ->Result<ImageAttachment,AnsfError>{
        if !valid_chat(chat)||!valid(a){return Err(AnsfError::InvalidInput)}
        let mut conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let tx=conn.transaction()?;
        let mut rows=read_manifest(&tx,chat,None)?;
        if let Some(old)=rows.iter().find(|old|old.sha256==a.sha256){
            if old.observed!=a.observed||old.media_type!=a.media_type||old.bytes!=a.bytes{
                return Err(AnsfError::InvalidInput)
            }
            return Ok(old.clone())
        }
        rows.push(a.clone());
        validate_manifest(&rows)?;
        tx.execute(
            "INSERT INTO ansf_image_attachments
             (session_id,attachment_id,display_name,media_type,bytes,sha256,observed,turn_id)
             VALUES(?1,?2,?3,?4,?5,?6,?7,NULL)",
            params![chat,a.attachment_id,a.display_name,a.media_type,a.bytes,a.sha256,a.observed])?;
        tx.commit()?;
        Ok(a.clone())
    }
    /// Restore a complete ordered ordinary-image batch to an EMPTY draft.
    /// The native caller stages and verifies distinct fresh object handles.
    /// The saved original is never modified; there is no partial restore.
    pub fn restore_image_draft(&self,chat:&str,rows:&[ImageAttachment])
        ->Result<(),AnsfError>{
        if !valid_chat(chat)||rows.is_empty()||rows.iter().any(|a|a.observed){
            return Err(AnsfError::InvalidInput)
        }
        validate_manifest(rows)?;
        let mut conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let tx=conn.transaction()?;
        if !read_manifest(&tx,chat,None)?.is_empty(){return Err(AnsfError::InvalidInput)}
        for a in rows{
            tx.execute(
                "INSERT INTO ansf_image_attachments
                 (session_id,attachment_id,display_name,media_type,bytes,sha256,observed,turn_id)
                 VALUES(?1,?2,?3,?4,?5,?6,0,NULL)",
                params![chat,a.attachment_id,a.display_name,a.media_type,a.bytes,a.sha256])?;
        }
        tx.commit()?;
        Ok(())
    }
    /// Removes a draft reference only; past human turns cannot be detached
    /// or overwritten by a stale UI release. Host handles external cleanup.
    pub fn remove_draft_image(&self,chat:&str,id:&str)->Result<bool,AnsfError>{
        if !valid_chat(chat)||!valid_attachment_id(id){return Err(AnsfError::InvalidInput)}
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        Ok(conn.execute(
            "DELETE FROM ansf_image_attachments WHERE session_id=?1 AND attachment_id=?2
             AND turn_id IS NULL",params![chat,id])?==1)
    }
    pub fn saved_image_manifest(&self,chat:&str,turn:&str)->Result<Vec<ImageAttachment>,AnsfError>{
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        read_saved(&conn,chat,turn)
    }
    /// Scope a UI history read to the exact immutable visible original.
    pub fn image_manifest_for_visible_seq(&self,chat:&str,seq:i64)
        ->Result<Vec<ImageAttachment>,AnsfError>{
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let turn:Option<String>=conn.query_row(
            "SELECT e.user_turn_id FROM ansf_live_chat_events e
             JOIN ansf_visible_chat v ON v.first_seq=e.seq AND v.session_id=e.session_id
             WHERE e.session_id=?1 AND e.seq=?2 AND e.kind='user' AND e.origin='host'
             AND e.user_turn_id IS NOT NULL",params![chat,seq],|r|r.get(0)).optional()?;
        match turn{Some(turn)=>read_saved(&conn,chat,&turn),None=>Ok(vec![])}
    }
    /// Used only to reconcile unpublished/removed bytes in the host-private
    /// store. Unknown scope or unavailable database must never authorize deletion.
    pub fn image_attachment_exists(&self,chat:&str,id:&str)->Result<bool,AnsfError>{
        if !valid_chat(chat)||!valid_attachment_id(id){return Err(AnsfError::InvalidInput)}
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM ansf_image_attachments
             WHERE session_id=?1 AND attachment_id=?2)",params![chat,id],|r|r.get(0))?)
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::ansf_live_chat::{LiveEvent,LiveKind};
    fn image(n:u8)->ImageAttachment{
        ImageAttachment{attachment_id:format!("img-{n:032x}"),display_name:format!("{n}.png"),
            media_type:"image/png".into(),bytes:100,sha256:format!("{n:064x}"),observed:false}
    }
    fn turn(n:u8)->LiveEvent{
        LiveEvent{event_id:format!("e-{n:032x}"),kind:LiveKind::User,text:"inspect".into(),
            user_turn_id:Some(format!("g-{n:032x}")),legacy_turn_id:None,
            generation_id:None,call_id:None,tool_id:None}
    }
    #[test]
    fn order_scope_retry_and_atomic_binding_survive_restart(){
        let db=std::env::temp_dir().join(format!("lu-image-manifest-{}-{}.sqlite",
            std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos()));
        let a=image(7);let b=image(2);
        let e=turn(5);let g=e.user_turn_id.as_deref().unwrap();
        let seq;
        {
            let book=AnsfStore::open(&db).unwrap();
            for chat in ["chat-a","chat-b"]{
                book.create_session(chat,"ws","Chat").unwrap();
                book.start_chapter(chat,"Original").unwrap();
            }
            book.admit_draft_image("chat-a",&a).unwrap();
            book.admit_draft_image("chat-a",&b).unwrap();
            assert_eq!(book.image_draft("chat-a").unwrap(),vec![a.clone(),b.clone()]);
            assert!(book.image_draft("chat-b").unwrap().is_empty());
            assert!(!book.remove_draft_image("chat-b",&a.attachment_id).unwrap());
            assert!(book.append_host_chat_event_with_image_draft("chat-a",&e,&[b.clone(),a.clone()]).is_err());
            assert!(book.authenticated_chat_turn("chat-a",g).unwrap().is_none());
            assert_eq!(book.image_draft("chat-a").unwrap().len(),2);
            assert!(book.append_host_chat_event_with_image_draft("chat-a",&e,&[a.clone()]).is_err());
            seq=book.append_host_chat_event_with_image_draft("chat-a",&e,&[a.clone(),b.clone()])
                .unwrap().seq;
        }
        {
            let book=AnsfStore::open(&db).unwrap();
            assert!(book.image_draft("chat-a").unwrap().is_empty());
            assert_eq!(book.saved_image_manifest("chat-a",g).unwrap(),vec![a.clone(),b.clone()]);
            assert_eq!(book.image_manifest_for_visible_seq("chat-a",seq).unwrap(),vec![a.clone(),b.clone()]);
            assert!(book.image_manifest_for_visible_seq("chat-b",seq).unwrap().is_empty());
            assert!(book.saved_image_manifest("chat-b",g).is_err());
            assert!(!book.remove_draft_image("chat-a",&a.attachment_id).unwrap());
            let entry=book.authenticated_chat_turn("chat-a",g).unwrap().unwrap().1;
            assert_eq!(book.artifact_links_for_entry("chat-a",entry).unwrap().len(),2);
        }
        let _=std::fs::remove_file(db);
    }
    #[test]
    fn aggregate_budget_and_invalid_identity_do_not_mutate_draft(){
        let book=AnsfStore::in_memory().unwrap();
        book.create_session("chat","ws","Chat").unwrap();
        book.start_chapter("chat","Original").unwrap();
        let mut a=image(1);a.bytes=MAX_IMAGE_BYTES as u64;
        let mut b=image(2);b.bytes=MAX_IMAGE_BYTES as u64;
        book.admit_draft_image("chat",&a).unwrap();
        book.admit_draft_image("chat",&b).unwrap();
        assert!(book.admit_draft_image("chat",&image(3)).is_err());
        let mut bad=image(4);bad.attachment_id="../escape".into();
        assert!(book.admit_draft_image("chat",&bad).is_err());
        assert_eq!(book.image_draft("chat").unwrap(),vec![a,b]);
    }
    #[test]
    fn explicit_restore_is_all_or_nothing_and_never_restores_observations(){
        let book=AnsfStore::in_memory().unwrap();
        book.create_session("chat","ws","Chat").unwrap();
        book.start_chapter("chat","Original").unwrap();
        let a=image(1);let b=image(2);
        book.admit_draft_image("chat",&a).unwrap();
        assert!(book.restore_image_draft("chat",&[b.clone()]).is_err());
        assert_eq!(book.image_draft("chat").unwrap(),vec![a.clone()]);
        let event=turn(5);
        book.append_host_chat_event_with_image_draft("chat",&event,&[a.clone()]).unwrap();
        assert!(book.restore_image_draft("chat",&[b.clone(),a.clone()]).is_err());
        assert!(book.image_draft("chat").unwrap().is_empty());
        let mut observed=image(3);observed.observed=true;
        assert!(book.restore_image_draft("chat",&[b.clone(),observed]).is_err());
        book.restore_image_draft("chat",&[b.clone()]).unwrap();
        assert_eq!(book.image_draft("chat").unwrap(),vec![b]);
        assert_eq!(book.saved_image_manifest("chat",event.user_turn_id.as_deref().unwrap()).unwrap(),vec![a]);
    }
    #[test]
    fn duplicate_limits_and_observe_taint_are_enforced(){
        let book=AnsfStore::in_memory().unwrap();
        book.create_session("chat","ws","Chat").unwrap();
        book.start_chapter("chat","Original").unwrap();
        let mut a=image(1);a.observed=true;
        book.admit_draft_image("chat",&a).unwrap();
        let mut same=a.clone();same.attachment_id=image(9).attachment_id;
        assert_eq!(book.admit_draft_image("chat",&same).unwrap(),a);
        same.observed=false;
        assert!(book.admit_draft_image("chat",&same).is_err());
        for n in 2..=4{book.admit_draft_image("chat",&image(n)).unwrap();}
        assert!(book.admit_draft_image("chat",&image(5)).is_err());
        let e=turn(9);
        let saved=book.append_host_chat_event_with_image_draft("chat",&e,&book.image_draft("chat").unwrap()).unwrap();
        assert_eq!(book.artifact_links_for_entry("chat",saved.source_entry_id.unwrap()).unwrap().len(),3);
    }
}
