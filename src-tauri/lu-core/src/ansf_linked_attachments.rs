//! Host-selected original-file links, persisted in the existing ANSF book.
//! No source bytes, excerpt bodies, generated copies or ambient grants.
//! Private paths never enter an artifact link or renderer/model view.
use crate::ansf::{AnsfError,AnsfStore};
use crate::ansf_artifact_links::{ArtifactLink,insert_artifact_link};
use rusqlite::{params,Connection,Transaction,OptionalExtension};

pub const MAX_LINKED_DOCUMENTS:usize=4;
pub const MAX_LINKED_FILE_BYTES:u64=8*1024*1024;

#[derive(Clone,PartialEq,Eq)]
pub struct LinkedAttachment{
    pub attachment_id:String,
    pub display_name:String,
    pub media_type:String,
    pub bytes:u64,
    pub sha256:String,
    // HOST ONLY. Deliberately not Serialize/Deserialize; the original path
    // is neither a public source identifier nor an authorization capability.
    pub source_path:String,
    pub reader_format:Option<String>,
    pub selector:String,
    pub offset_chars:u32,
    pub excerpt_sha256:Option<String>,
}
impl std::fmt::Debug for LinkedAttachment{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        f.debug_struct("LinkedAttachment").field("attachment_id",&self.attachment_id)
            .field("media_type",&self.media_type).field("bytes",&self.bytes)
            .field("source_path",&"[host private]").finish_non_exhaustive()
    }
}
fn opaque(s:&str,prefix:&str)->bool{
    s.len()==prefix.len()+32&&s.starts_with(prefix)&&
    s[prefix.len()..].bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn digest(s:&str)->bool{s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))}
fn chat_ok(s:&str)->bool{!s.is_empty()&&s.len()<=128&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')}
pub fn valid_linked_attachment_id(id:&str)->bool{opaque(id,"doc-")||opaque(id,"img-")}
fn valid(a:&LinkedAttachment)->bool{
    valid_linked_attachment_id(&a.attachment_id)&&
    !a.display_name.trim().is_empty()&&a.display_name.len()<=200&&
    !a.display_name.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'|'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'))&&
    (1..=MAX_LINKED_FILE_BYTES).contains(&a.bytes)&&digest(&a.sha256)&&
    a.source_path.len()<=16*1024&&!a.source_path.chars().any(char::is_control)&&
    std::path::Path::new(&a.source_path).is_absolute()&&
    !std::path::Path::new(&a.source_path).components().any(|c|matches!(c,std::path::Component::ParentDir))&&
    (if a.attachment_id.starts_with("doc-"){
        a.media_type=="application/octet-stream"&&
        a.reader_format.as_deref().is_some_and(|v|matches!(v,"PlainText"|"Markdown"|"Csv"|"Json"|"Xml"|"Html"|"SourceCode"|"Pdf"|"Docx"|"Xlsx"|"Pptx"|"Ods"|"Odp"|"Odt"|"Rtf"|"Epub"))&&
        a.selector.len()<=160&&!a.selector.chars().any(char::is_control)&&
        a.offset_chars<=8*1024*1024&&a.excerpt_sha256.as_deref().is_some_and(digest)
    }else{
        matches!(a.media_type.as_str(),"image/png"|"image/jpeg")&&
        a.reader_format.is_none()&&a.selector.is_empty()&&a.offset_chars==0&&a.excerpt_sha256.is_none()
    })
}
fn validate(rows:&[LinkedAttachment])->Result<(),AnsfError>{
    if rows.len()>8{return Err(AnsfError::InvalidInput)}
    let mut ids=std::collections::HashSet::new();
    let(mut docs,mut images,mut image_bytes)=(0usize,0usize,0u64);
    for a in rows{
        if !valid(a)||!ids.insert(&a.attachment_id){return Err(AnsfError::InvalidInput)}
        if a.attachment_id.starts_with("doc-"){docs+=1}else{images+=1;image_bytes+=a.bytes;}
    }
    if docs>MAX_LINKED_DOCUMENTS||images>4||image_bytes>16*1024*1024{return Err(AnsfError::InvalidInput)}
    Ok(())
}

/// Shared total bound across both historical and linked image manifests.
pub(crate) fn validate_combined(legacy:&[crate::ansf_image_attachments::ImageAttachment],
    links:&[LinkedAttachment])->Result<(),AnsfError>{
    validate(links)?;
    let images=links.iter().filter(|a|a.attachment_id.starts_with("img-"));
    if legacy.len()+images.clone().count()>crate::multimodal_saved_turn::MAX_IMAGES||
       legacy.iter().map(|a|a.bytes).sum::<u64>()+
          images.map(|a|a.bytes).sum::<u64>()>crate::multimodal_saved_turn::MAX_TOTAL_IMAGE_BYTES as u64||
       legacy.iter().any(|a|links.iter().any(|b|a.attachment_id==b.attachment_id)){
        return Err(AnsfError::InvalidInput)
    }
    Ok(())
}

pub(crate) fn initialize_linked_attachment_schema(conn:&Connection)->Result<(),AnsfError>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_linked_attachments(
            seq INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
            attachment_id TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL, media_type TEXT NOT NULL,
            bytes INTEGER NOT NULL CHECK(bytes>0 AND bytes<=8388608),
            sha256 TEXT NOT NULL CHECK(length(sha256)=64),
            source_path TEXT NOT NULL CHECK(length(source_path)>0 AND length(source_path)<=16384),
            reader_format TEXT, selector TEXT NOT NULL CHECK(length(selector)<=160),
            offset_chars INTEGER NOT NULL CHECK(offset_chars>=0 AND offset_chars<=8388608),
            excerpt_sha256 TEXT CHECK(excerpt_sha256 IS NULL OR length(excerpt_sha256)=64),
            turn_id TEXT
         );
         CREATE INDEX IF NOT EXISTS ansf_linked_attachment_manifest
           ON ansf_linked_attachments(session_id,turn_id,seq);
         CREATE TRIGGER IF NOT EXISTS ansf_linked_saved_no_update
           BEFORE UPDATE ON ansf_linked_attachments WHEN OLD.turn_id IS NOT NULL
           BEGIN SELECT RAISE(ABORT,'saved attachment links are immutable'); END;
         CREATE TRIGGER IF NOT EXISTS ansf_linked_saved_no_delete
           BEFORE DELETE ON ansf_linked_attachments WHEN OLD.turn_id IS NOT NULL
           BEGIN SELECT RAISE(ABORT,'saved attachment links are immutable'); END;"
    )?;
    Ok(())
}
pub(crate) fn read_manifest(conn:&Connection,chat:&str,turn:Option<&str>)
    ->Result<Vec<LinkedAttachment>,AnsfError>{
    if !chat_ok(chat)||turn.is_some_and(|t|!opaque(t,"g-")){return Err(AnsfError::InvalidInput)}
    let mut stmt=conn.prepare(
        "SELECT attachment_id,display_name,media_type,bytes,sha256,source_path,reader_format,
                selector,offset_chars,excerpt_sha256
         FROM ansf_linked_attachments WHERE session_id=?1 AND turn_id IS ?2
         ORDER BY seq LIMIT 9")?;
    let rows=stmt.query_map(params![chat,turn],|r|Ok(LinkedAttachment{
        attachment_id:r.get(0)?,display_name:r.get(1)?,media_type:r.get(2)?,
        bytes:r.get(3)?,sha256:r.get(4)?,source_path:r.get(5)?,reader_format:r.get(6)?,
        selector:r.get(7)?,offset_chars:r.get(8)?,excerpt_sha256:r.get(9)?,
    }))?.collect::<Result<Vec<_>,_>>()?;
    validate(&rows)?;Ok(rows)
}
fn insert_draft(tx:&Transaction<'_>,chat:&str,a:&LinkedAttachment)->Result<(),AnsfError>{
    tx.execute("INSERT INTO ansf_linked_attachments(session_id,attachment_id,display_name,media_type,
        bytes,sha256,source_path,reader_format,selector,offset_chars,excerpt_sha256,turn_id)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,NULL)",
        params![chat,a.attachment_id,a.display_name,a.media_type,a.bytes,a.sha256,a.source_path,
            a.reader_format,a.selector,a.offset_chars,a.excerpt_sha256])?;
    Ok(())
}
pub(crate) fn bind_linked_draft(tx:&Transaction<'_>,chat:&str,turn:&str,entry:i64,
    expected:&[LinkedAttachment])->Result<(),AnsfError>{
    if !opaque(turn,"g-")||entry<=0||crate::ansf_image_attachments::image_submission_rejected(tx,chat,turn)?||
       read_manifest(tx,chat,None)?.as_slice()!=expected{return Err(AnsfError::InvalidInput)}
    for a in expected{
        insert_artifact_link(tx,&ArtifactLink{
            session_id:chat.into(),entry_id:entry,artifact_id:a.attachment_id.clone(),
            display_name:a.display_name.clone(),media_type:a.media_type.clone(),
            expected_bytes:Some(a.bytes),expected_sha256:Some(a.sha256.clone()),
        })?;
    }
    let n=tx.execute("UPDATE ansf_linked_attachments SET turn_id=?2
        WHERE session_id=?1 AND turn_id IS NULL",params![chat,turn])?;
    if n!=expected.len(){return Err(AnsfError::InvalidInput)}
    Ok(())
}
fn read_saved(conn:&Connection,chat:&str,turn:&str)->Result<Vec<LinkedAttachment>,AnsfError>{
    let entry:Option<i64>=conn.query_row(
        "SELECT source_entry_id FROM ansf_live_chat_events WHERE session_id=?1
         AND user_turn_id=?2 AND kind='user' AND origin='host' AND source_entry_id IS NOT NULL",
        params![chat,turn],|r|r.get(0)).optional()?;
    let entry=entry.ok_or(AnsfError::InvalidInput)?;
    let rows=read_manifest(conn,chat,Some(turn))?;
    for a in &rows{
        let original:Option<(String,String,u64,String)>=conn.query_row(
            "SELECT display_name,media_type,expected_bytes,expected_sha256 FROM ansf_artifact_links
             WHERE session_id=?1 AND entry_id=?2 AND artifact_id=?3",
            params![chat,entry,a.attachment_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        if original!=Some((a.display_name.clone(),a.media_type.clone(),a.bytes,a.sha256.clone())){
            return Err(AnsfError::InvalidInput)
        }
    }
    Ok(rows)
}
impl AnsfStore{
    pub fn linked_draft(&self,chat:&str)->Result<Vec<LinkedAttachment>,AnsfError>{
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;read_manifest(&conn,chat,None)
    }
    /// Only the native host calls this after a real operator selects and
    /// verifies the original source. A duplicate exact section is idempotent.
    pub fn admit_linked_draft(&self,chat:&str,a:&LinkedAttachment)->Result<LinkedAttachment,AnsfError>{
        self.admit_linked_drafts(chat,std::slice::from_ref(a))?
            .into_iter().next().ok_or(AnsfError::InvalidInput)
    }
    /// Batch admission is atomic. Exact repeated sections resolve to their
    /// existing identity, while a changed file requires explicit removal.
    pub fn admit_linked_drafts(&self,chat:&str,candidates:&[LinkedAttachment])
        ->Result<Vec<LinkedAttachment>,AnsfError>{
        if !chat_ok(chat)||candidates.is_empty()||candidates.len()>8||
            candidates.iter().any(|a|!valid(a)){return Err(AnsfError::InvalidInput)}
        let mut conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;let tx=conn.transaction()?;
        let mut rows=read_manifest(&tx,chat,None)?;
        let legacy=crate::ansf_image_attachments::read_manifest(&tx,chat,None)?;
        let mut admitted=Vec::new();
        for a in candidates{
            if let Some(old)=rows.iter().find(|old|old.source_path==a.source_path&&old.selector==a.selector&&
                old.offset_chars==a.offset_chars&&old.media_type==a.media_type){
                let mut expected=a.clone();expected.attachment_id=old.attachment_id.clone();
                if *old!=expected{return Err(AnsfError::InvalidInput)}
                admitted.push(old.clone());
            }else{
                rows.push(a.clone());validate_combined(&legacy,&rows)?;
                insert_draft(&tx,chat,a)?;admitted.push(a.clone());
            }
        }
        tx.commit()?;Ok(admitted)
    }
    /// Metadata-only removal never opens, deletes or moves a filesystem file.
    pub fn remove_linked_draft(&self,chat:&str,id:&str)->Result<bool,AnsfError>{
        if !chat_ok(chat)||!valid_linked_attachment_id(id){return Err(AnsfError::InvalidInput)}
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        Ok(conn.execute("DELETE FROM ansf_linked_attachments WHERE session_id=?1 AND attachment_id=?2
            AND turn_id IS NULL",params![chat,id])?==1)
    }
    /// Only a host-reverified original saved link batch may be restored.
    /// Historical copied images are not relabeled as original-file links.
    pub fn restore_linked_draft(&self,chat:&str,seq:i64,rows:&[LinkedAttachment])->Result<(),AnsfError>{
        if !chat_ok(chat)||seq<=0||rows.is_empty(){return Err(AnsfError::InvalidInput)}
        validate(rows)?;
        let mut conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;let tx=conn.transaction()?;
        if !read_manifest(&tx,chat,None)?.is_empty()||
           !crate::ansf_image_attachments::read_manifest(&tx,chat,None)?.is_empty(){
            return Err(AnsfError::InvalidInput)
        }
        let turn:String=tx.query_row("SELECT e.user_turn_id FROM ansf_live_chat_events e
            JOIN ansf_visible_chat v ON v.first_seq=e.seq AND v.session_id=e.session_id
            WHERE e.session_id=?1 AND e.seq=?2 AND e.kind='user' AND e.origin='host'
            AND e.user_turn_id IS NOT NULL",params![chat,seq],|r|r.get(0))?;
        if !crate::ansf_image_attachments::read_manifest(&tx,chat,Some(&turn))?.is_empty(){
            return Err(AnsfError::InvalidInput)
        }
        let original=read_saved(&tx,chat,&turn)?;
        if original.len()!=rows.len(){return Err(AnsfError::InvalidInput)}
        for (old,new) in original.iter().zip(rows){
            let mut expected=old.clone();expected.attachment_id=new.attachment_id.clone();
            if &expected!=new||old.attachment_id==new.attachment_id||
                tx.query_row("SELECT EXISTS(SELECT 1 FROM ansf_image_attachments WHERE attachment_id=?1)",
                    [&new.attachment_id],|r|r.get::<_,bool>(0))?{
                return Err(AnsfError::InvalidInput)
            }
        }
        for a in rows{insert_draft(&tx,chat,a)?;}
        tx.commit()?;Ok(())
    }
    pub fn saved_linked_manifest(&self,chat:&str,turn:&str)->Result<Vec<LinkedAttachment>,AnsfError>{
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;read_saved(&conn,chat,turn)
    }
    pub fn linked_manifest_for_visible_seq(&self,chat:&str,seq:i64)->Result<Vec<LinkedAttachment>,AnsfError>{
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let turn:Option<String>=conn.query_row(
            "SELECT e.user_turn_id FROM ansf_live_chat_events e
             JOIN ansf_visible_chat v ON v.first_seq=e.seq AND v.session_id=e.session_id
             WHERE e.session_id=?1 AND e.seq=?2 AND e.kind='user' AND e.origin='host'
             AND e.user_turn_id IS NOT NULL",params![chat,seq],|r|r.get(0)).optional()?;
        match turn{Some(turn)=>read_saved(&conn,chat,&turn),None=>Ok(vec![])}
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::ansf_live_chat::{LiveEvent,LiveKind};
    fn db()->std::path::PathBuf{
        std::env::temp_dir().join(format!("lu-linked-manifest-{}-{}.sqlite",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    }
    fn doc(n:u8)->LinkedAttachment{
        LinkedAttachment{attachment_id:format!("doc-{n:032x}"),display_name:format!("{n}.txt"),
            media_type:"application/octet-stream".into(),bytes:8,sha256:format!("{n:064x}"),
            source_path:std::env::temp_dir().join(format!("original-{n}.txt")).to_str().unwrap().into(),
            reader_format:Some("PlainText".into()),selector:String::new(),offset_chars:0,
            excerpt_sha256:Some(format!("{n:064x}"))}
    }
    fn turn(n:u8)->LiveEvent{
        LiveEvent{event_id:format!("e-{n:032x}"),kind:LiveKind::User,text:"Read my files".into(),
            user_turn_id:Some(format!("g-{n:032x}")),legacy_turn_id:None,
            generation_id:None,call_id:None,tool_id:None}
    }
    #[test]
    fn original_links_are_ordered_atomic_chat_scoped_and_survive_reopen(){
        let path=db();let a=doc(1);let b=doc(2);let event=turn(3);
        let gid=event.user_turn_id.as_deref().unwrap();let seq;
        {
            let book=AnsfStore::open(&path).unwrap();
            for chat in ["chat-a","chat-b"]{
                book.create_session(chat,"ws","Chat").unwrap();
                book.start_chapter(chat,"Original").unwrap();
            }
            book.admit_linked_draft("chat-a",&a).unwrap();book.admit_linked_draft("chat-a",&b).unwrap();
            assert!(book.linked_draft("chat-b").unwrap().is_empty());
            assert!(!book.remove_linked_draft("chat-b",&a.attachment_id).unwrap());
            assert!(book.append_host_chat_event_with_attachment_drafts("chat-a",&event,&[],&[b.clone(),a.clone()]).is_err());
            assert!(book.authenticated_chat_turn("chat-a",gid).unwrap().is_none());
            assert!(book.append_host_chat_event_with_image_draft("chat-a",&event,&[]).is_err());
            assert_eq!(book.linked_draft("chat-a").unwrap(),vec![a.clone(),b.clone()]);
            seq=book.append_host_chat_event_with_attachment_drafts("chat-a",&event,&[],&[a.clone(),b.clone()]).unwrap().seq;
        }
        {
            let book=AnsfStore::open(&path).unwrap();
            assert!(book.linked_draft("chat-a").unwrap().is_empty());
            assert_eq!(book.saved_linked_manifest("chat-a",gid).unwrap(),vec![a.clone(),b.clone()]);
            assert_eq!(book.linked_manifest_for_visible_seq("chat-a",seq).unwrap(),vec![a.clone(),b.clone()]);
            assert!(book.linked_manifest_for_visible_seq("chat-b",seq).unwrap().is_empty());
            assert!(!book.remove_linked_draft("chat-a",&a.attachment_id).unwrap());
            let conn=book.conn.lock().unwrap();
            assert!(conn.execute("UPDATE ansf_linked_attachments SET source_path='moved' WHERE attachment_id=?1",[&a.attachment_id]).is_err());
            assert!(conn.execute("DELETE FROM ansf_linked_attachments WHERE attachment_id=?1",[&a.attachment_id]).is_err());
            let columns:Vec<String>=conn.prepare("PRAGMA table_info(ansf_linked_attachments)").unwrap()
                .query_map([],|r|r.get(1)).unwrap().collect::<Result<_,_>>().unwrap();
            assert!(!columns.iter().any(|x|matches!(x.as_str(),"content"|"body"|"file_bytes"|"excerpt")));
        }
        let _=std::fs::remove_file(path);
    }
    #[test]
    fn linked_document_and_existing_image_commit_or_rollback_together(){
        use crate::ansf_image_attachments::ImageAttachment;
        let path=db();let book=AnsfStore::open(&path).unwrap();
        book.create_session("chat-a","ws","Chat").unwrap();book.start_chapter("chat-a","Original").unwrap();
        let a=doc(9);let image=ImageAttachment{attachment_id:format!("img-{:032x}",10),
            display_name:"photo.png".into(),media_type:"image/png".into(),bytes:32,
            sha256:format!("{:064x}",10),observed:false};
        book.admit_linked_draft("chat-a",&a).unwrap();book.admit_draft_image("chat-a",&image).unwrap();
        let event=turn(11);let gid=event.user_turn_id.as_deref().unwrap();
        assert!(book.append_host_chat_event_with_attachment_drafts("chat-a",&event,&[image.clone()],&[]).is_err());
        assert_eq!(book.image_draft("chat-a").unwrap(),vec![image.clone()]);
        assert_eq!(book.linked_draft("chat-a").unwrap(),vec![a.clone()]);
        assert!(book.authenticated_chat_turn("chat-a",gid).unwrap().is_none());
        book.append_host_chat_event_with_attachment_drafts("chat-a",&event,&[image.clone()],&[a.clone()]).unwrap();
        assert_eq!(book.saved_image_manifest("chat-a",gid).unwrap(),vec![image]);
        assert_eq!(book.saved_linked_manifest("chat-a",gid).unwrap(),vec![a]);
        drop(book);let _=std::fs::remove_file(path);
    }
    #[test]
    fn cp02_linked_recovery_and_restore_are_ordered_and_immutable(){
        let path=db();let a=doc(20);let b=doc(21);let event=turn(22);let next=turn(23);
        let gid=event.user_turn_id.as_deref().unwrap();let next_gid=next.user_turn_id.as_deref().unwrap();
        let refs=vec![a.attachment_id.clone(),b.attachment_id.clone()];
        {
            let book=AnsfStore::open(&path).unwrap();
            for chat in ["chat-a","chat-b"]{
                book.create_session(chat,"ws","Chat").unwrap();book.start_chapter(chat,"Original").unwrap();
            }
            book.admit_linked_draft("chat-a",&a).unwrap();book.admit_linked_draft("chat-a",&b).unwrap();
            assert!(book.stage_attachment_ui_submission("chat-a",gid,"snapshot",&[],&[refs[1].clone(),refs[0].clone()]).is_err());
            assert!(book.stage_attachment_ui_submission("chat-b",gid,"snapshot",&[],&refs).is_err());
            book.stage_attachment_ui_submission("chat-a",gid,"snapshot",&[],&refs).unwrap();
            book.stage_attachment_ui_submission("chat-a",gid,"snapshot",&[],&refs).unwrap();
            assert!(book.stage_attachment_ui_submission("chat-a",next_gid,"next",&[],&refs).is_err());
            assert!(!book.clear_image_ui_submission("chat-a",gid).unwrap());
        }
        {
            let book=AnsfStore::open(&path).unwrap();
            assert_eq!(book.image_ui_submission("chat-a").unwrap(),Some((gid.into(),"snapshot".into())));
            book.reject_image_submission("chat-a",gid).unwrap();
            assert!(book.stage_attachment_ui_submission("chat-a",gid,"snapshot",&[],&refs).is_err());
            book.stage_attachment_ui_submission("chat-a",next_gid,"next",&[],&refs).unwrap();
            let seq=book.append_host_chat_event_with_attachment_drafts("chat-a",&next,&[],&[a.clone(),b.clone()]).unwrap().seq;
            assert!(book.clear_image_ui_submission("chat-a",next_gid).unwrap());
            let mut first=a.clone();first.attachment_id=format!("doc-{:032x}",24);
            let mut second=b.clone();second.attachment_id=format!("doc-{:032x}",25);
            assert!(book.restore_linked_draft("chat-b",seq,&[first.clone(),second.clone()]).is_err());
            assert!(book.restore_linked_draft("chat-a",seq,&[second.clone(),first.clone()]).is_err());
            assert!(book.linked_draft("chat-a").unwrap().is_empty());
            book.restore_linked_draft("chat-a",seq,&[first.clone(),second.clone()]).unwrap();
            assert_eq!(book.linked_draft("chat-a").unwrap(),vec![first,second]);
            assert_eq!(book.saved_linked_manifest("chat-a",next_gid).unwrap(),vec![a,b]);
        }
        let _=std::fs::remove_file(path);
    }
    #[test]
    fn cp02_batch_failure_rolls_back_all_new_links(){
        let path=db();let book=AnsfStore::open(&path).unwrap();
        book.create_session("chat-a","ws","Chat").unwrap();book.start_chapter("chat-a","Original").unwrap();
        let a=doc(1);let b=doc(2);let mut changed=b.clone();changed.sha256="a".repeat(64);
        assert!(book.admit_linked_drafts("chat-a",&[a.clone(),b.clone(),changed]).is_err());
        assert!(book.linked_draft("chat-a").unwrap().is_empty());
        assert_eq!(book.admit_linked_drafts("chat-a",&[a.clone(),b.clone()]).unwrap(),vec![a.clone(),b.clone()]);
        assert_eq!(book.admit_linked_drafts("chat-a",&[a.clone()]).unwrap(),vec![a]);
        assert_eq!(book.linked_draft("chat-a").unwrap().len(),2);
        drop(book);let _=std::fs::remove_file(path);
    }
    #[test]
    fn cp02_combined_image_limit_cannot_be_evaded_with_two_manifests(){
        use crate::ansf_image_attachments::ImageAttachment;
        let path=db();let book=AnsfStore::open(&path).unwrap();
        book.create_session("chat-a","ws","Chat").unwrap();book.start_chapter("chat-a","Original").unwrap();
        for n in 1..=4u8{
            let a=ImageAttachment{attachment_id:format!("img-{n:032x}"),display_name:format!("{n}.png"),
                media_type:"image/png".into(),bytes:32,sha256:format!("{n:064x}"),observed:false};
            book.admit_draft_image("chat-a",&a).unwrap();
        }
        let mut linked=doc(9);linked.attachment_id=format!("img-{:032x}",9);
        linked.media_type="image/png".into();linked.reader_format=None;linked.excerpt_sha256=None;
        assert!(book.admit_linked_draft("chat-a",&linked).is_err());
        assert!(book.linked_draft("chat-a").unwrap().is_empty());
        drop(book);let _=std::fs::remove_file(path);
    }
    #[test]
    fn refused_nonce_cannot_consume_linked_draft_and_metadata_removal_never_deletes_source(){
        let path=db();let book=AnsfStore::open(&path).unwrap();
        book.create_session("chat-a","ws","Chat").unwrap();book.start_chapter("chat-a","Original").unwrap();
        let mut a=doc(7);let file=path.with_extension("original.txt");
        std::fs::write(&file,b"original").unwrap();a.source_path=file.to_str().unwrap().into();
        book.admit_linked_draft("chat-a",&a).unwrap();let event=turn(8);
        book.reject_image_submission("chat-a",event.user_turn_id.as_deref().unwrap()).unwrap();
        assert!(book.append_host_chat_event_with_attachment_drafts("chat-a",&event,&[],&[a.clone()]).is_err());
        assert!(book.remove_linked_draft("chat-a",&a.attachment_id).unwrap());
        assert_eq!(std::fs::read(&file).unwrap(),b"original");
        drop(book);let _=std::fs::remove_file(file);let _=std::fs::remove_file(path);
    }
}
