//! UX-CHAT01: durable, chat-scoped, bounded message reactions and appearance.
 //!
 //! ANSF chat messages are immutable originals. Reactions/themes are derived
 //! local UI metadata, not model prompt text, tool authority, or a revision.
 //! The original saved message's chat and visible sequence are revalidated
 //! against ChatStore before every reaction mutation.
 use std::{collections::BTreeSet,fs,path::{Path,PathBuf}};
 use rusqlite::{params,Connection,OpenFlags,OptionalExtension};
 use serde::{Serialize,Deserialize};
 use crate::chat_store::ChatStore;
 const DATABASE:&str="lu-message-interactions.sqlite";
 const MAX_DB:u64=32*1024*1024;
 const MAX_REACTIONS:i64=4096;
 pub const DEFAULT_BACKGROUND:&str="#151e2b";
 pub const DEFAULT_USER:&str="#315b86";
 pub const DEFAULT_ASSISTANT:&str="#1c2a3b";
 const ALLOWED_REACTIONS:[&str;6]=["👍","❤️","😂","🎉","😮","👀"];
 #[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
 #[serde(rename_all="camelCase",deny_unknown_fields)]
 pub struct ChatAppearance{
     pub background:String,
     pub user_bubble:String,
     pub assistant_bubble:String,
 }
 impl Default for ChatAppearance{
     fn default()->Self{
         Self{
             background:DEFAULT_BACKGROUND.into(),
             user_bubble:DEFAULT_USER.into(),
             assistant_bubble:DEFAULT_ASSISTANT.into(),
         }
     }
 }
 impl ChatAppearance{
     fn valid(&self)->bool{
         [&self.background,&self.user_bubble,&self.assistant_bubble]
             .iter().all(|color|{
                 let bytes=color.as_bytes();
                 bytes.len()==7&&bytes[0]==b'#'
                     &&bytes[1..].iter().all(u8::is_ascii_hexdigit)
             })
     }
 }
 #[derive(Clone,Debug,Serialize,PartialEq,Eq)]
 #[serde(rename_all="camelCase")]
 pub struct MessageReaction{
     pub seq:i64,
     pub emoji:String,
 }
 #[derive(Clone,Debug,Serialize)]
 #[serde(rename_all="camelCase")]
 pub struct ChatDecorations{
     pub appearance:ChatAppearance,
     pub reactions:Vec<MessageReaction>,
 }
 fn good_chat(chat:&str)->bool{
     chat.len()==34&&chat.starts_with("c-")
         &&chat[2..].bytes().all(|b|
             b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
 }
 fn path(root:&Path)->PathBuf{root.join(DATABASE)}
 fn checked(path:&Path)->Result<bool,String>{
     let meta=match fs::symlink_metadata(path){
         Ok(m)=>m,
         Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
         Err(_)=>return Err("Message metadata inaccessible".into())
     };
     if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>MAX_DB{
         return Err("Message metadata must be a bounded regular file".into())
     }
     #[cfg(windows)]
     {
         use std::os::windows::fs::MetadataExt;
         if meta.file_attributes()&0x400!=0{
             return Err("Linked message metadata refused".into())
         }
     }
     Ok(true)
 }
 fn safe_database(root:&Path)->Result<bool,String>{
     let main=path(root);
     let exists=checked(&main)?;
     // Even with no main DB, reject a linked orphan WAL/SHM sidecar
     // before SQLite could initialize the file and follow the suffix.
     for suffix in ["-wal","-shm"]{
         let mut other=main.as_os_str().to_os_string();
         other.push(suffix);
         let other=PathBuf::from(other);
         let present=checked(&other)?;
         if present&&!exists{
             return Err("Orphan message metadata journal sidecar".into())
         }
     }
     Ok(exists)
 }
 pub(crate) fn open_read(root:&Path)->Result<Option<Connection>,String>{
     if !safe_database(root)?{return Ok(None)}
     Connection::open_with_flags(
         path(root),OpenFlags::SQLITE_OPEN_READ_ONLY|
         OpenFlags::SQLITE_OPEN_NO_MUTEX
     ).map(Some).map_err(|_|"Message metadata database unreadable".into())
 }
 pub(crate) fn open_write(root:&Path)->Result<Connection,String>{
     // Don't silently follow a malicious replacement or broken existing DB.
     let _=safe_database(root)?;
     let db=Connection::open(path(root))
         .map_err(|_|"Message metadata database unavailable")?;
     db.pragma_update(None,"journal_mode","WAL")
         .map_err(|_|"Message metadata WAL unavailable")?;
     db.pragma_update(None,"synchronous","FULL")
         .map_err(|_|"Message metadata durability unavailable")?;
     db.execute_batch(
         "CREATE TABLE IF NOT EXISTS lu_message_reactions(
           chat_id TEXT NOT NULL,
           seq INTEGER NOT NULL CHECK(seq>0),
           emoji TEXT NOT NULL,
           updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
           PRIMARY KEY(chat_id,seq)
          );
          CREATE TABLE IF NOT EXISTS lu_chat_appearance(
           chat_id TEXT PRIMARY KEY,
           background TEXT NOT NULL,
           user_bubble TEXT NOT NULL,
           assistant_bubble TEXT NOT NULL,
           updated_at TEXT DEFAULT CURRENT_TIMESTAMP
          );"
     ).map_err(|_|"Cannot initialize message metadata")?;
     Ok(db)
 }
 pub(crate) fn confirm_chat(chat:&str,chats:&ChatStore)->Result<(),String>{
     if !good_chat(chat){return Err("Invalid conversation identity".into())}
     chats.history_window(chat,None,"older",1).map(|_|())
 }
 fn confirm_message(chat:&str,seq:i64,chats:&ChatStore)->Result<(),String>{
     if seq<=0||seq==i64::MAX{
         return Err("Invalid message sequence".into())
     }
     // A bounded indexed lookup of the exact visible message, not
     // an unbounded chat export. Source messages are never overwritten.
     let page=chats.history_window(chat,Some(seq+1),"older",1)?;
     if page.entries.first().map(|m|m.seq)!=Some(seq){
         return Err("Original message not found in selected conversation".into())
     }
     Ok(())
 }
 pub fn read(
     root:&Path,chat:&str,visible_seqs:&[i64],chats:&ChatStore,
 )->Result<ChatDecorations,String>{
     confirm_chat(chat,chats)?;
     if visible_seqs.len()>64||visible_seqs.iter().any(|s|*s<=0){
         return Err("Message metadata request exceeds visible window".into())
     }
     let mut result=ChatDecorations{
         appearance:ChatAppearance::default(),reactions:Vec::new()
     };
     let Some(db)=open_read(root)?else{return Ok(result)};
     let appearance:Option<(String,String,String)>=db.query_row(
         "SELECT background,user_bubble,assistant_bubble
          FROM lu_chat_appearance WHERE chat_id=?1",
         params![chat],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
     ).optional().map_err(|_|"Chat appearance record invalid")?;
     if let Some((background,user_bubble,assistant_bubble))=appearance{
         let styled=ChatAppearance{
             background,user_bubble,assistant_bubble
         };
         if !styled.valid(){return Err("Unsafe stored chat colors".into())}
         result.appearance=styled;
     }
     let visible=visible_seqs.iter().copied().collect::<BTreeSet<_>>();
     if !visible.is_empty(){
         let mut stmt=db.prepare(
             "SELECT seq,emoji FROM lu_message_reactions
              WHERE chat_id=?1 ORDER BY seq DESC LIMIT 4097"
         ).map_err(|_|"Cannot inspect message reactions")?;
         let rows=stmt.query_map(params![chat],|r|Ok((
             r.get::<_,i64>(0)?,r.get::<_,String>(1)?
         ))).map_err(|_|"Cannot read message reactions")?;
         let mut checked=0i64;
         for row in rows{
             checked+=1;
             if checked>MAX_REACTIONS{
                 return Err("Too many message reactions for conversation".into())
             }
             let (seq,emoji)=row.map_err(|_|"Invalid reaction record")?;
             if !ALLOWED_REACTIONS.contains(&emoji.as_str()){
                 return Err("Invalid stored reaction".into())
             }
             if visible.contains(&seq){
                 result.reactions.push(MessageReaction{seq,emoji});
             }
         }
     }
     Ok(result)
 }
 pub fn set_reaction(
     root:&Path,chat:&str,seq:i64,emoji:Option<&str>,chats:&ChatStore,
 )->Result<Option<MessageReaction>,String>{
     confirm_chat(chat,chats)?;
     confirm_message(chat,seq,chats)?;
     if emoji.is_some_and(|e|!ALLOWED_REACTIONS.contains(&e)){
         return Err("Unsupported emoji reaction".into())
     }
     let mut db=open_write(root)?;
     let tx=db.transaction().map_err(|_|"Reaction transaction unavailable")?;
     if let Some(emoji)=emoji{
         let present:Option<String>=tx.query_row(
             "SELECT emoji FROM lu_message_reactions WHERE chat_id=?1 AND seq=?2",
             params![chat,seq],|r|r.get(0)
         ).optional().map_err(|_|"Reaction lookup failed")?;
         if present.is_none(){
             let count:i64=tx.query_row(
                 "SELECT COUNT(*) FROM lu_message_reactions WHERE chat_id=?1",
                 params![chat],|r|r.get(0)
             ).map_err(|_|"Reaction quota unreadable")?;
             if count>=MAX_REACTIONS{
                 return Err("Conversation reaction limit reached".into())
             }
         }
         tx.execute(
             "INSERT INTO lu_message_reactions(chat_id,seq,emoji)
              VALUES(?1,?2,?3)
              ON CONFLICT(chat_id,seq) DO UPDATE
              SET emoji=excluded.emoji,updated_at=CURRENT_TIMESTAMP",
             params![chat,seq,emoji]
         ).map_err(|_|"Cannot update reaction")?;
     }else{
         tx.execute(
             "DELETE FROM lu_message_reactions WHERE chat_id=?1 AND seq=?2",
             params![chat,seq]
         ).map_err(|_|"Cannot clear reaction")?;
     }
     tx.commit().map_err(|_|"Reaction transaction not durable")?;
     Ok(emoji.map(|emoji|MessageReaction{seq,emoji:emoji.into()}))
 }
 pub fn set_appearance(
     root:&Path,chat:&str,appearance:&ChatAppearance,chats:&ChatStore,
 )->Result<ChatAppearance,String>{
     confirm_chat(chat,chats)?;
     if !appearance.valid(){
         return Err("Chat colors must be #RRGGBB hexadecimal".into())
     }
     let db=open_write(root)?;
     db.execute(
         "INSERT INTO lu_chat_appearance(
            chat_id,background,user_bubble,assistant_bubble)
          VALUES(?1,?2,?3,?4)
          ON CONFLICT(chat_id) DO UPDATE
          SET background=excluded.background,
              user_bubble=excluded.user_bubble,
              assistant_bubble=excluded.assistant_bubble,
              updated_at=CURRENT_TIMESTAMP",
         params![chat,appearance.background,appearance.user_bubble,
             appearance.assistant_bubble]
     ).map_err(|_|"Chat appearance could not be saved")?;
     Ok(appearance.clone())
 }
 #[cfg(test)]
 mod tests{
     use super::*;
     use std::time::{SystemTime,UNIX_EPOCH};
     fn fixture()->(PathBuf,ChatStore,String,i64){
         let root=std::env::temp_dir().join(format!(
             "lu-message-interaction-{}-{}",
             std::process::id(),
             SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
         ));
         fs::create_dir_all(&root).unwrap();
         let chats=ChatStore::open(&root).unwrap();
         let chat=chats.create_chat().unwrap().chat_id;
         chats.append_user_with_turn_id(
             &chat,&format!("g-{:032x}",1),"A saved user message"
         ).unwrap();
         let seq=chats.history_window(&chat,None,"older",1)
             .unwrap().entries[0].seq;
         (root,chats,chat,seq)
     }
     #[test]
     fn local_reactions_and_colors_persist_across_full_chat_store_restart(){
         let (root,chats,chat,seq)=fixture();
         assert_eq!(read(&root,&chat,&[seq],&chats).unwrap().reactions.len(),0);
         assert!(!root.join(DATABASE).exists());
         assert_eq!(set_reaction(&root,&chat,seq,Some("❤️"),&chats)
             .unwrap().unwrap().emoji,"❤️");
         let colors=ChatAppearance{
             background:"#233344".into(),
             user_bubble:"#fab405".into(),
             assistant_bubble:"#0a130f".into(),
         };
         set_appearance(&root,&chat,&colors,&chats).unwrap();
         drop(chats);
         let restarted=ChatStore::open(&root).unwrap();
         let display=read(&root,&chat,&[seq],&restarted).unwrap();
         assert_eq!(display.appearance,colors);
         assert_eq!(display.reactions,vec![MessageReaction{
             seq,emoji:"❤️".into()
         }]);
         set_reaction(&root,&chat,seq,None,&restarted).unwrap();
         assert!(read(&root,&chat,&[seq],&restarted).unwrap().reactions.is_empty());
         drop(restarted);fs::remove_dir_all(root).unwrap();
     }
     #[test]
     fn cannot_react_to_unknown_message_or_other_conversation(){
         let (root,chats,chat,seq)=fixture();
         assert!(set_reaction(&root,&chat,seq+100,Some("👍"),&chats).is_err());
         assert!(set_reaction(&root,"c-00000000000000000000000000000002",
             seq,Some("👍"),&chats).is_err());
         assert!(set_reaction(&root,&chat,seq,Some("💊"),&chats).is_err());
         assert!(!root.join(DATABASE).exists());
         drop(chats);fs::remove_dir_all(root).unwrap();
     }
     #[test]
     fn refuses_arbitrary_css_and_unknown_palette_fields(){
         let (root,chats,chat,_)=fixture();
         let appearance=ChatAppearance{
             background:"url(https://unsafe)".into(),
             ..Default::default()
         };
         assert!(set_appearance(&root,&chat,&appearance,&chats).is_err());
         let appearance=ChatAppearance{
             user_bubble:"#gggggg".into(),
             ..Default::default()
         };
         assert!(set_appearance(&root,&chat,&appearance,&chats).is_err());
         assert!(!root.join(DATABASE).exists());
         drop(chats);fs::remove_dir_all(root).unwrap();
     }
     #[cfg(unix)]
     #[test]
     fn linked_metadata_file_is_rejected(){
         use std::os::unix::fs::symlink;
         let (root,chats,chat,seq)=fixture();
         let target=root.join("target.sqlite");
         fs::write(&target,"not a database").unwrap();
         symlink(&target,root.join(DATABASE)).unwrap();
         assert!(read(&root,&chat,&[seq],&chats).is_err());
         assert!(set_reaction(&root,&chat,seq,Some("👍"),&chats).is_err());
         drop(chats);fs::remove_dir_all(root).unwrap();
     }
 }
