//! UX-CHAT02: append-only DISPLAY correction chains and human reply provenance.
 //!
 //! None of these annotations mutate canonical ANSF, llama context, prior
 //! tool authority, independently verified receipts or earlier replies.
 //! Human edits of assistant text are explicitly labeled human corrections.
 //! Reply is a NEW HOST-AUTHENTICATED USER turn, not a model/tool retry.
 use rusqlite::{params,Connection,OptionalExtension};
 use serde::Serialize;
 use sha2::{Digest,Sha256};
 use crate::chat_store::{ChatStore,HistoryWindowEntry};
 use crate::chat_message_interactions::{open_read,open_write,confirm_chat};
 use std::path::Path;
 const MAX_TEXT:usize=16*1024;
 const MAX_VERSIONS:i64=16;
 const MAX_REPLIES:i64=2048;
 #[derive(Clone,Debug,Serialize,PartialEq,Eq)]
 #[serde(rename_all="camelCase")]
 pub struct DisplayVersion{
     pub version:i64,
     pub text:String,
     pub sha256:String,
     pub editor:String,
 }
 #[derive(Clone,Debug,Serialize,PartialEq,Eq)]
 #[serde(rename_all="camelCase")]
 pub struct MessageReplyLink{
     pub reply_seq:i64,
     pub parent_seq:i64,
     pub pinned_parent_version:i64,
     pub pinned_parent_sha256:String,
     pub pinned_parent_excerpt:String,
     pub relationship:&'static str,
 }
 #[derive(Clone,Debug,Serialize)]
 #[serde(rename_all="camelCase")]
 pub struct MessageRevisionView{
     pub seq:i64,
     pub role:String,
     pub original_sha256:String,
     pub original_text:String,
     pub current_version:i64,
     pub current_sha256:String,
     pub current_text:String,
     pub editor:String,
     pub versions:Vec<DisplayVersion>,
     pub reply_to:Option<MessageReplyLink>,
 }
 #[derive(Clone,Debug,Serialize)]
 #[serde(rename_all="camelCase")]
 pub struct MessageRevisionPage{
     pub entries:Vec<MessageRevisionView>,
     pub pending_reply_links:usize,
     pub semantics:&'static str,
 }
 #[derive(Clone,Debug,Serialize)]
 #[serde(rename_all="camelCase")]
 pub struct ReplyReceipt{
     pub reply_seq:i64,
     pub reply_turn_id:String,
     pub parent_seq:i64,
     pub parent_version:i64,
     pub pinned_parent_sha256:String,
     pub status:&'static str,
 }
 fn hash(text:&str)->String{format!("{:x}",Sha256::digest(text.as_bytes()))}
 fn safe_text(text:&str)->bool{
     !text.trim().is_empty() && text.len()<=MAX_TEXT && !text.contains('\0')
 }
 fn safe_sha(sha:&str)->bool{
     sha.len()==64&&sha.bytes().all(|b|
         b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
 }
 fn exact_turn(turn:&str)->bool{
     turn.len()==34&&turn.starts_with("g-")&&turn[2..].bytes().all(|b|
         b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
 }
 fn original(chats:&ChatStore,chat:&str,seq:i64)->Result<HistoryWindowEntry,String>{
     if seq<=0||seq==i64::MAX{return Err("Invalid original message sequence".into())}
     let page=chats.history_window(chat,Some(seq+1),"older",1)?;
     let entry=page.entries.into_iter().next()
         .ok_or("Missing original ANSF message")?;
     if entry.seq!=seq||!matches!(entry.role.as_str(),"user"|"assistant"){
         return Err("Original message not in selected conversation".into())
     }
     Ok(entry)
 }
 fn exists(db:&Connection,name:&str)->Result<bool,String>{
     db.query_row(
         "SELECT EXISTS(SELECT 1 FROM sqlite_master
          WHERE type='table' AND name=?1)",
         params![name],|r|r.get(0)
     ).map_err(|_|"Unreadable message revision schema".into())
 }
 fn ensure_schema(db:&Connection)->Result<(),String>{
     db.execute_batch(
         "CREATE TABLE IF NOT EXISTS lu_message_revisions(
             chat_id TEXT NOT NULL,
             seq INTEGER NOT NULL CHECK(seq>0),
             version INTEGER NOT NULL CHECK(version BETWEEN 1 AND 16),
             original_sha256 TEXT NOT NULL,
             previous_sha256 TEXT NOT NULL,
             sha256 TEXT NOT NULL,
             corrected_text TEXT NOT NULL,
             editor TEXT NOT NULL CHECK(editor='HUMAN_DISPLAY_CORRECTION'),
             created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
             PRIMARY KEY(chat_id,seq,version)
         );
         CREATE TABLE IF NOT EXISTS lu_message_reply_intents(
             chat_id TEXT NOT NULL,
             reply_turn_id TEXT NOT NULL,
             parent_seq INTEGER NOT NULL CHECK(parent_seq>0),
             parent_version INTEGER NOT NULL CHECK(parent_version BETWEEN 0 AND 16),
             parent_sha256 TEXT NOT NULL,
             reply_sha256 TEXT NOT NULL,
             reply_seq INTEGER,
             state TEXT NOT NULL CHECK(state IN ('PREPARED','LINKED')),
             created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
             PRIMARY KEY(chat_id,reply_turn_id),
             UNIQUE(chat_id,reply_seq)
         );
         CREATE INDEX IF NOT EXISTS lu_message_reply_parent
             ON lu_message_reply_intents(chat_id,parent_seq);
         CREATE TRIGGER IF NOT EXISTS lu_message_revision_no_update
             BEFORE UPDATE ON lu_message_revisions
             BEGIN SELECT RAISE(ABORT,'Immutable display revision'); END;
         CREATE TRIGGER IF NOT EXISTS lu_message_revision_no_delete
             BEFORE DELETE ON lu_message_revisions
             BEGIN SELECT RAISE(ABORT,'Immutable display revision'); END;
         CREATE TRIGGER IF NOT EXISTS lu_reply_no_delete
             BEFORE DELETE ON lu_message_reply_intents
             BEGIN SELECT RAISE(ABORT,'Immutable original reply link'); END;
         CREATE TRIGGER IF NOT EXISTS lu_reply_identity_immutable
             BEFORE UPDATE OF chat_id,reply_turn_id,parent_seq,parent_version,
                 parent_sha256,reply_sha256 ON lu_message_reply_intents
             BEGIN SELECT RAISE(ABORT,'Cannot retarget old reply'); END;
         CREATE TRIGGER IF NOT EXISTS lu_reply_state_guard
             BEFORE UPDATE ON lu_message_reply_intents
             WHEN OLD.state!='PREPARED' OR NEW.state!='LINKED'
                 OR OLD.reply_seq IS NOT NULL OR NEW.reply_seq IS NULL
             BEGIN SELECT RAISE(ABORT,'Reply intent cannot be replayed'); END;"
     ).map_err(|_|"Unable to establish append-only message history")?;
     Ok(())
 }
 fn versions(
     db:Option<&Connection>,chat:&str,seq:i64,source:&HistoryWindowEntry,
 )->Result<Vec<DisplayVersion>,String>{
     let Some(db)=db else{return Ok(vec![])};
     if !exists(db,"lu_message_revisions")?{return Ok(vec![])}
     let mut stmt=db.prepare(
         "SELECT version,original_sha256,previous_sha256,sha256,
                 corrected_text,editor
          FROM lu_message_revisions WHERE chat_id=?1 AND seq=?2
          ORDER BY version LIMIT 17"
     ).map_err(|_|"Cannot read message revisions")?;
     let found=stmt.query_map(params![chat,seq],|r|Ok((
         r.get::<_,i64>(0)?,r.get::<_,String>(1)?,
         r.get::<_,String>(2)?,r.get::<_,String>(3)?,
         r.get::<_,String>(4)?,r.get::<_,String>(5)?
     ))).map_err(|_|"Invalid revision query")?;
     let original_sha=hash(&source.text);
     let mut latest_sha=original_sha.clone();
     let mut rows=Vec::new();
     for row in found{
         let (v,origin,previous,sha,text,editor)=row
             .map_err(|_|"Damaged message revision record")?;
         if v!=rows.len() as i64+1||v>MAX_VERSIONS||origin!=original_sha
             ||previous!=latest_sha||!safe_text(&text)||sha!=hash(&text)
             ||editor!="HUMAN_DISPLAY_CORRECTION"{
             return Err("Message correction chain inconsistent with ANSF original".into())
         }
         latest_sha=sha.clone();
         rows.push(DisplayVersion{
             version:v,text,sha256:sha,editor
         });
     }
     Ok(rows)
 }
 fn display_at(
     db:Option<&Connection>,chats:&ChatStore,chat:&str,seq:i64,
     version:i64,expected_sha:&str,
 )->Result<DisplayVersion,String>{
     if !(0..=MAX_VERSIONS).contains(&version)||!safe_sha(expected_sha){
         return Err("Invalid pinned message revision".into())
     }
     let source=original(chats,chat,seq)?;
     let revisions=versions(db,chat,seq,&source)?;
     let pinned=if version==0{
         DisplayVersion{
             version:0,text:source.text.clone(),sha256:hash(&source.text),
             editor:"ORIGINAL_SOURCE".into()
         }
     }else{
         revisions.get((version-1) as usize)
             .ok_or("Pinned correction version missing")?.clone()
     };
     if pinned.sha256!=expected_sha{
         return Err("Pinned source hash does not match original provenance".into())
     }
     Ok(pinned)
 }
 pub fn read(
     root:&Path,chat:&str,seqs:&[i64],chats:&ChatStore
 )->Result<MessageRevisionPage,String>{
     confirm_chat(chat,chats)?;
     if seqs.len()>64||seqs.iter().any(|s|*s<=0||*s==i64::MAX){
         return Err("Maximum 64 valid original messages per revision page".into())
     }
     let db=open_read(root)?;
     let mut entries=Vec::new();
     let mut pending=0usize;
     if let Some(db)=db.as_ref(){
         if exists(db,"lu_message_reply_intents")?{
             pending=db.query_row(
                 "SELECT COUNT(*) FROM lu_message_reply_intents
                  WHERE chat_id=?1 AND state='PREPARED'",
                 params![chat],|r|r.get::<_,i64>(0)
             ).map_err(|_|"Pending reply links unreadable")?
               .try_into().map_err(|_|"Invalid pending reply count")?;
             if pending>MAX_REPLIES as usize{
                 return Err("Too many unresolved reply links".into())
             }
         }
     }
     for seq in seqs{
         let source=original(chats,chat,*seq)?;
         let revisions=versions(db.as_ref(),chat,*seq,&source)?;
         let latest=revisions.last().cloned().unwrap_or_else(||DisplayVersion{
             version:0,text:source.text.clone(),sha256:hash(&source.text),
             editor:"ORIGINAL_SOURCE".into()
         });
         let mut reply_to=None;
         if let Some(db)=db.as_ref(){
             if exists(db,"lu_message_reply_intents")?{
                 let link:Option<(String,i64,i64,String,String,String)>=db.query_row(
                     "SELECT reply_turn_id,parent_seq,parent_version,
                             parent_sha256,reply_sha256,state
                      FROM lu_message_reply_intents
                      WHERE chat_id=?1 AND reply_seq=?2",
                     params![chat,seq],|r|Ok((
                         r.get(0)?,r.get(1)?,r.get(2)?,
                         r.get(3)?,r.get(4)?,r.get(5)?
                     ))
                 ).optional().map_err(|_|"Reply provenance lookup failed")?;
                 if let Some((turn,parent,version,pin,reply_sha,state))=link{
                     let authenticated=chats.original_user_turn_visible_seq(chat,&turn)?
                         .ok_or("Linked reply does not have an original human turn")?;
                     if state!="LINKED"||authenticated.0!=*seq
                         ||hash(&authenticated.1)!=reply_sha{
                         return Err("Linked reply does not match its original human message".into())
                     }
                     let target=display_at(
                         Some(db),chats,chat,parent,version,&pin
                     )?;
                     reply_to=Some(MessageReplyLink{
                         reply_seq:*seq,parent_seq:parent,pinned_parent_version:version,
                         pinned_parent_sha256:pin,
                         pinned_parent_excerpt:target.text.chars().take(240).collect(),
                         relationship:"HOST_AUTHENTICATED_HUMAN_REPLY",
                     });
                 }
             }
         }
         entries.push(MessageRevisionView{
             seq:*seq,role:source.role,original_sha256:hash(&source.text),
             original_text:source.text,
             current_version:latest.version,current_sha256:latest.sha256,
             current_text:latest.text,editor:latest.editor,
             versions,reply_to
         });
     }
     Ok(MessageRevisionPage{
         entries,pending_reply_links:pending,
         semantics:"DISPLAY_CORRECTIONS_ONLY_NO_MODEL_REEXECUTION",
     })
 }
 pub fn edit(
     root:&Path,chat:&str,seq:i64,expected_version:i64,
     expected_sha:&str,text:&str,chats:&ChatStore
 )->Result<MessageRevisionView,String>{
     confirm_chat(chat,chats)?;
     let source=original(chats,chat,seq)?;
     if !safe_text(text)||!safe_sha(expected_sha){
         return Err("Invalid corrected display text or digest".into())
     }
     let mut db=open_write(root)?;
     ensure_schema(&db)?;
     let tx=db.transaction().map_err(|_|"Correction transaction unavailable")?;
     let chain=versions(Some(&tx),chat,seq,&source)?;
     let version=chain.len() as i64;
     let last=chain.last().map(|x|x.sha256.clone())
         .unwrap_or_else(||hash(&source.text));
     if expected_version!=version||expected_sha!=last{
         return Err("Revision changed; reopen latest version before editing".into())
     }
     if version>=MAX_VERSIONS{return Err("Message correction limit reached".into())}
     if text==chain.last().map(|x|x.text.as_str()).unwrap_or(&source.text){
         return Err("No display change to save".into())
     }
     tx.execute(
         "INSERT INTO lu_message_revisions(
            chat_id,seq,version,original_sha256,previous_sha256,
            sha256,corrected_text,editor)
          VALUES(?1,?2,?3,?4,?5,?6,?7,'HUMAN_DISPLAY_CORRECTION')",
         params![chat,seq,version+1,hash(&source.text),last,hash(text),text]
     ).map_err(|_|"Display correction conflict")?;
     tx.commit().map_err(|_|"Unable to commit immutable display revision")?;
     let page=read(root,chat,&[seq],chats)?;
     page.entries.into_iter().next().ok_or("Saved revision missing".into())
 }
 /// Reserve one unique human reply turn and pinned version BEFORE a ChatStore
 /// write. If Lu dies after the ANSF append, explicit reconciliation uses the
 /// same original turn ID and NEVER appends a duplicate human message.
 pub fn prepare_reply(
     root:&Path,chat:&str,parent:i64,version:i64,sha:&str,
     turn:&str,text:&str,chats:&ChatStore
 )->Result<(),String>{
     confirm_chat(chat,chats)?;
     if !exact_turn(turn)||!safe_text(text){
         return Err("Invalid fresh human reply turn".into())
     }
     let mut db=open_write(root)?;
     ensure_schema(&db)?;
     let pinned=display_at(Some(&db),chats,chat,parent,version,sha)?;
     let latest=versions(Some(&db),chat,parent,&original(chats,chat,parent)?)?;
     if version!=latest.len() as i64{
         return Err("Parent was edited; review the latest version before replying".into())
     }
     if pinned.text.is_empty(){return Err("Pinned reply source missing".into())}
     let tx=db.transaction().map_err(|_|"Reply transaction unavailable")?;
     let count:i64=tx.query_row(
         "SELECT COUNT(*) FROM lu_message_reply_intents WHERE chat_id=?1",
         params![chat],|r|r.get(0)
     ).map_err(|_|"Reply count unavailable")?;
     if count>=MAX_REPLIES{return Err("Linked reply limit reached".into())}
     tx.execute(
         "INSERT INTO lu_message_reply_intents(
            chat_id,reply_turn_id,parent_seq,parent_version,
            parent_sha256,reply_sha256,reply_seq,state)
          VALUES(?1,?2,?3,?4,?5,?6,NULL,'PREPARED')",
         params![chat,turn,parent,version,sha,hash(text)]
     ).map_err(|_|"Duplicate or conflicting human reply intent")?;
     tx.commit().map_err(|_|"Reply intent not durable")?;
     Ok(())
 }
 pub fn reconcile_reply(
     root:&Path,chat:&str,turn:&str,chats:&ChatStore
 )->Result<Option<ReplyReceipt>,String>{
     confirm_chat(chat,chats)?;
     if !exact_turn(turn){return Err("Invalid human reply turn".into())}
     let mut db=open_write(root)?;
     ensure_schema(&db)?;
     let tx=db.transaction().map_err(|_|"Reply reconciliation unavailable")?;
     let prepared:Option<(i64,i64,String,String,String,Option<i64>)>=tx.query_row(
         "SELECT parent_seq,parent_version,parent_sha256,reply_sha256,
                 state,reply_seq
          FROM lu_message_reply_intents
          WHERE chat_id=?1 AND reply_turn_id=?2",
         params![chat,turn],|r|Ok((
             r.get(0)?,r.get(1)?,r.get(2)?,
             r.get(3)?,r.get(4)?,r.get(5)?
         ))
     ).optional().map_err(|_|"Reply intent cannot be located")?;
     let Some((parent,version,sha,reply_sha,state,saved_seq))=prepared
         else{return Err("No reviewed reply intent exists")};
     let Some((seq,text))=chats.original_user_turn_visible_seq(chat,turn)?
         else{return Ok(None)};
     if seq<=parent||hash(&text)!=reply_sha{
         return Err("Original human reply differs from prepared intent".into())
     }
     display_at(Some(&tx),chats,chat,parent,version,&sha)?;
     if state=="LINKED"{
         if saved_seq!=Some(seq){return Err("Conflicting reply linkage".into())}
     }else if state=="PREPARED"&&saved_seq.is_none(){
         tx.execute(
             "UPDATE lu_message_reply_intents
              SET reply_seq=?3,state='LINKED'
              WHERE chat_id=?1 AND reply_turn_id=?2
                AND state='PREPARED' AND reply_seq IS NULL",
             params![chat,turn,seq]
         ).map_err(|_|"Unable to reconcile original reply")?;
     }else{return Err("Invalid reply lifecycle".into())}
     tx.commit().map_err(|_|"Reply link not durably committed")?;
     Ok(Some(ReplyReceipt{
         reply_seq:seq,reply_turn_id:turn.into(),parent_seq:parent,
         parent_version:version,pinned_parent_sha256:sha,
         status:"LINKED_ORIGINAL_HUMAN_TURN_NO_MODEL_EXECUTION",
     }))
 }
 /// Read-only view does not retry an uncertain reply. Explicitly inspect
 /// only already-authenticated ANSF turns and link their prepared metadata.
 pub fn reconcile_pending(
     root:&Path,chat:&str,chats:&ChatStore
 )->Result<usize,String>{
     confirm_chat(chat,chats)?;
     let Some(db)=open_read(root)?else{return Ok(0)};
     if !exists(&db,"lu_message_reply_intents")?{return Ok(0)}
     let mut stmt=db.prepare(
         "SELECT reply_turn_id FROM lu_message_reply_intents
          WHERE chat_id=?1 AND state='PREPARED' LIMIT 65"
     ).map_err(|_|"Cannot scan prepared reply links")?;
     let rows=stmt.query_map(params![chat],|r|r.get::<_,String>(0))
         .map_err(|_|"Prepared reply link query failed")?;
     let mut turns=Vec::new();
     for row in rows{
         if turns.len()>=64{return Err("Too many pending reply links".into())}
         turns.push(row.map_err(|_|"Invalid prepared reply turn")?);
     }
     drop(stmt);drop(db);
     let mut recovered=0usize;
     for turn in turns{
         if reconcile_reply(root,chat,&turn,chats)?.is_some(){
             recovered+=1;
         }
     }
     Ok(recovered)
 }


#[cfg(test)]
mod tests{
    use super::*;
    use std::{fs,path::PathBuf,time::{SystemTime,UNIX_EPOCH}};
    fn id(prefix:&str,n:u64)->String{format!("{prefix}{n:032x}")}
    fn fixture()->(PathBuf,ChatStore,String,i64){
        let root=std::env::temp_dir().join(format!(
            "lu-ux-chat02-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        chats.append_user_with_turn_id(
            &chat,&id("g-",1),"Original user request, with tool history"
        ).unwrap();
        let seq=chats.history_window(&chat,None,"older",1)
            .unwrap().entries[0].seq;
        (root,chats,chat,seq)
    }
    #[test]
    fn immutable_original_and_append_only_display_edits_survive_restart(){
        let (root,chats,chat,seq)=fixture();
        let original=original(&chats,&chat,seq).unwrap();
        let base=hash(&original.text);
        let first=edit(&root,&chat,seq,0,&base,
            "Human-edited request (display correction)",&chats).unwrap();
        assert_eq!(first.current_version,1);
        assert_eq!(first.editor,"HUMAN_DISPLAY_CORRECTION");
        assert_eq!(first.original_text,original.text);
        let second=edit(&root,&chat,seq,1,&first.current_sha256,
            "Second human display correction",&chats).unwrap();
        assert_eq!(second.current_version,2);
        assert_eq!(second.versions.len(),2);
        assert!(edit(&root,&chat,seq,0,&base,
            "Stale overwriting editor",&chats).is_err());
        // Corrected display MUST NOT change canonical ANSF original.
        assert_eq!(original(&chats,&chat,seq).unwrap().text,
            "Original user request, with tool history");
        let db=open_read(&root).unwrap().unwrap();
        assert!(db.execute(
            "UPDATE lu_message_revisions SET corrected_text='forged'
             WHERE chat_id=?1 AND seq=?2 AND version=1",
            params![chat,seq]
        ).is_err());
        drop(db);drop(chats);
        let restarted=ChatStore::open(&root).unwrap();
        let saved=read(&root,&chat,&[seq],&restarted).unwrap();
        assert_eq!(saved.entries.len(),1);
        assert_eq!(saved.entries[0].current_version,2);
        assert_eq!(saved.entries[0].original_sha256,base);
        assert_eq!(saved.entries[0].original_text,original.text);
        drop(restarted);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn linked_reply_pins_parent_version_and_original_authenticated_turn(){
        let (root,chats,chat,parent)=fixture();
        let v1=edit(&root,&chat,parent,0,
            &hash("Original user request, with tool history"),
            "Display correction before reply",&chats).unwrap();
        let turn=id("g-",2);
        prepare_reply(&root,&chat,parent,1,&v1.current_sha256,
            &turn,"New human message in reply to corrected display",&chats).unwrap();
        assert!(reconcile_reply(&root,&chat,&turn,&chats).unwrap().is_none());
        chats.append_user_with_turn_id(&chat,&turn,
            "New human message in reply to corrected display").unwrap();
        let linked=reconcile_reply(&root,&chat,&turn,&chats)
            .unwrap().unwrap();
        assert_eq!(linked.parent_version,1);
        assert_eq!(linked.parent_seq,parent);
        assert!(linked.reply_seq>parent);
        assert_eq!(reconcile_reply(&root,&chat,&turn,&chats)
            .unwrap().unwrap().reply_seq,linked.reply_seq);
        // A later display edit never moves the reply's pinned citation.
        let _v2=edit(&root,&chat,parent,1,&v1.current_sha256,
            "New display correction after reply",&chats).unwrap();
        let page=read(&root,&chat,&[linked.reply_seq],&chats).unwrap();
        let pinned=page.entries[0].reply_to.as_ref().unwrap();
        assert_eq!(pinned.pinned_parent_version,1);
        assert_eq!(pinned.pinned_parent_sha256,v1.current_sha256);
        assert_eq!(pinned.pinned_parent_excerpt,
            "Display correction before reply");
        let db=open_read(&root).unwrap().unwrap();
        assert!(db.execute(
            "UPDATE lu_message_reply_intents SET parent_seq=?1
             WHERE chat_id=?2 AND reply_turn_id=?3",
            params![999_i64,chat,turn]
        ).is_err());
        drop(db);drop(chats);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn crash_between_prepare_and_original_message_reconciles_without_reposting(){
        let (root,chats,chat,parent)=fixture();
        let target=hash("Original user request, with tool history");
        let turn=id("g-",3);
        prepare_reply(&root,&chat,parent,0,&target,&turn,
            "A crash-recovered human reply",&chats).unwrap();
        assert_eq!(reconcile_pending(&root,&chat,&chats).unwrap(),0);
        assert_eq!(read(&root,&chat,&[parent],&chats).unwrap().pending_reply_links,1);
        // ANSF is durable, yet the link was not finalized before restart.
        chats.append_user_with_turn_id(&chat,&turn,
            "A crash-recovered human reply").unwrap();
        drop(chats);
        let reopened=ChatStore::open(&root).unwrap();
        assert_eq!(reconcile_pending(&root,&chat,&reopened).unwrap(),1);
        assert_eq!(reconcile_pending(&root,&chat,&reopened).unwrap(),0);
        let (seq,original_text)=reopened.original_user_turn_visible_seq(
            &chat,&turn).unwrap().unwrap();
        assert_eq!(original_text,"A crash-recovered human reply");
        assert_eq!(read(&root,&chat,&[seq],&reopened).unwrap().entries[0]
            .reply_to.as_ref().unwrap().parent_seq,parent);
        drop(reopened);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn wrong_chat_unknown_message_stale_parent_and_wrong_turn_are_refused(){
        let (root,chats,chat,parent)=fixture();
        let other=chats.create_chat().unwrap().chat_id;
        let old=hash("Original user request, with tool history");
        assert!(edit(&root,&other,parent,0,&old,
            "forged cross-chat edit",&chats).is_err());
        assert!(edit(&root,&chat,parent+999,0,&old,
            "unknown sequence edit",&chats).is_err());
        let current=edit(&root,&chat,parent,0,&old,
            "First correction",&chats).unwrap();
        assert!(prepare_reply(&root,&chat,parent,0,&old,&id("g-",4),
            "Stale pinned target",&chats).is_err());
        assert!(prepare_reply(&root,&other,parent,1,&current.current_sha256,
            &id("g-",4),"Cross-chat target",&chats).is_err());
        prepare_reply(&root,&chat,parent,1,&current.current_sha256,
            &id("g-",5),"Expected text",&chats).unwrap();
        chats.append_user_with_turn_id(&chat,&id("g-",5),
            "Different text that was actually committed").unwrap();
        assert!(reconcile_reply(&root,&chat,&id("g-",5),&chats).is_err());
        drop(chats);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_version_text_and_missing_db_never_become_a_model_action(){
        let (root,chats,chat,seq)=fixture();
        assert!(!root.join("lu-message-interactions.sqlite").exists());
        let empty=read(&root,&chat,&[seq],&chats).unwrap();
        assert_eq!(empty.entries[0].current_version,0);
        assert!(!root.join("lu-message-interactions.sqlite").exists());
        assert!(edit(&root,&chat,seq,0,&hash(
            "Original user request, with tool history"),"", &chats).is_err());
        assert!(edit(&root,&chat,seq,0,&hash(
            "Original user request, with tool history"),
            &"x".repeat(16*1024+1), &chats).is_err());
        assert!(prepare_reply(&root,&chat,seq,99,&hash(
            "Original user request, with tool history"),
            &id("g-",9),"Invalid version",&chats).is_err());
        drop(chats);fs::remove_dir_all(root).unwrap();
    }
}
