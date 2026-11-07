//! Direct, stable ANSF source references. Known IDs bypass FTS and chapter search.
//!
//! A source ID is a *locator*, never tool authority. The host must authenticate
//! both the active conversation and the source book before opening anything.
//! Human navigation remains Library -> Book -> Chapter -> Entry as before.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD,Engine as _};
use crate::{
    ansf::{AnsfStore,Entry},
    ansf_library::{AnsfLibrary,BookIndex},
};

const PREFIX:&str="ansf:v1:";
const MAX_SOURCE_ID:usize=224;
pub const MAX_DIRECT_CONTENT_BYTES:usize=64*1024;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AnsfSourceId {
    session_id:String,
    entry_id:i64,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum DirectReadError {
    InvalidId,
    InvalidBudget,
    AccessDenied,
    NotIndexed,
    OpenUnavailable,
    EntryMissing,
    ContentTooLarge,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct DirectSourceData {
    pub source_id:String,
    pub source_session_id:String,
    pub source_message_id:String,
    /// A source role is historical metadata, not the current prompt role.
    pub source_role:String,
    pub summary:String,
    pub content:String,
}
/// The trusted host makes this principal/task-aware decision. The model,
/// retrieved text and Library index MUST NOT implement this grant.
pub trait HostSourceReadAccess {
    fn may_read(
        &self,
        active_conversation:&str,
        source_conversation:&str,
        source_workspace:&str,
    )->bool;
}
fn valid_session(id:&str)->bool{
    !id.trim().is_empty() && id.trim()==id &&
    id.len()<=128 && !id.contains('\0')
}
impl AnsfSourceId {
    pub fn new(session_id:impl Into<String>,entry_id:i64)->Result<Self,DirectReadError>{
        let session_id=session_id.into();
        if !valid_session(&session_id)||entry_id<=0{
            return Err(DirectReadError::InvalidId);
        }
        Ok(Self{session_id,entry_id})
    }
    pub fn parse(raw:&str)->Result<Self,DirectReadError>{
        if raw.len()>MAX_SOURCE_ID||!raw.starts_with(PREFIX){
            return Err(DirectReadError::InvalidId);
        }
        let tail=&raw[PREFIX.len()..];
        let (encoded,ordinal)=tail.split_once(':')
            .ok_or(DirectReadError::InvalidId)?;
        if encoded.is_empty()||ordinal.is_empty()||
           (ordinal.len()>1 && ordinal.starts_with('0'))||
           !ordinal.bytes().all(|b|b.is_ascii_digit()){
            return Err(DirectReadError::InvalidId);
        }
        let decoded=URL_SAFE_NO_PAD.decode(encoded)
            .map_err(|_|DirectReadError::InvalidId)?;
        let session=String::from_utf8(decoded)
            .map_err(|_|DirectReadError::InvalidId)?;
        let entry_id=ordinal.parse::<i64>().map_err(|_|DirectReadError::InvalidId)?;
        let value=Self::new(session,entry_id)?;
        // Canonical formatting disallows alternate encodings and aliases.
        if value.encode()!=raw{return Err(DirectReadError::InvalidId);}
        Ok(value)
    }
    pub fn encode(&self)->String{
        format!("{PREFIX}{}:{}",URL_SAFE_NO_PAD.encode(self.session_id.as_bytes()),self.entry_id)
    }
    pub fn session_id(&self)->&str{&self.session_id}
    pub fn entry_id(&self)->i64{self.entry_id}
}
impl Entry {
    /// Mint the globally unambiguous locator from the original session and
    /// local SQLite entry ID; stable across Library re-index and relocation.
    pub fn stable_source_id(&self)->Result<String,DirectReadError>{
        Ok(AnsfSourceId::new(self.session_id.clone(),self.id)?.encode())
    }
}
/// One host-callable operation for a KNOWN source ID; the installed model
/// interface is a separate integration gate. Internally it requires
/// only indexed book-identity and exact entry lookups, not three searches.
///
/// The opener is supplied by a trusted host, which must confine the original
/// book under its authenticated root and reject symlinks/reparse traversal.
/// This Core helper deliberately does not turn Library metadata into a path.
pub fn resolve_known_source(
    source_id:&str,
    active_conversation:&str,
    library:&AnsfLibrary,
    access:&dyn HostSourceReadAccess,
    open_authorized_book:impl FnOnce(&BookIndex)->Result<AnsfStore,DirectReadError>,
    max_content_bytes:usize,
)->Result<DirectSourceData,DirectReadError>{
    if max_content_bytes==0 || max_content_bytes>MAX_DIRECT_CONTENT_BYTES ||
       !valid_session(active_conversation){
        return Err(DirectReadError::InvalidBudget);
    }
    let id=AnsfSourceId::parse(source_id)?;
    let book=library.get_book(id.session_id())
        .map_err(|_|DirectReadError::OpenUnavailable)?
        .ok_or(DirectReadError::NotIndexed)?;
    // Index metadata is never a claim of permission. Check before opening.
    if !access.may_read(active_conversation,id.session_id(),&book.workspace_id){
        return Err(DirectReadError::AccessDenied)
    }
    if book.session_id!=id.session_id(){
        return Err(DirectReadError::OpenUnavailable)
    }
    let db=open_authorized_book(&book)?;
    let entry=db.read_entry(id.session_id(),id.entry_id())
        .map_err(|_|DirectReadError::OpenUnavailable)?
        .ok_or(DirectReadError::EntryMissing)?;
    if entry.content.len()>max_content_bytes{
        return Err(DirectReadError::ContentTooLarge)
    }
    Ok(DirectSourceData{
        source_id:source_id.to_owned(),
        source_session_id:entry.session_id,
        source_message_id:entry.message_id,
        source_role:entry.role,
        summary:entry.summary,
        content:entry.content,
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::ansf::EntryRole;
    use std::{fs,path::PathBuf,time::{SystemTime,UNIX_EPOCH}};
    struct Gate(bool);
    impl HostSourceReadAccess for Gate{
        fn may_read(&self,_active:&str,_source:&str,_workspace:&str)->bool{self.0}
    }
    fn test_root()->PathBuf{
        std::env::temp_dir().join(format!("lu-ansf-direct-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()))
    }
    #[test]
    fn canonical_id_round_trip_and_rejects_aliases(){
        let original=AnsfSourceId::new("c-0123456789abcdef","1".parse::<i64>().unwrap()).unwrap();
        let value=original.encode();
        assert_eq!(AnsfSourceId::parse(&value).unwrap(),original);
        assert!(AnsfSourceId::parse(&format!("{value}:extra")).is_err());
        assert!(AnsfSourceId::parse(&value.replace(":1",":01")).is_err());
        assert!(AnsfSourceId::new("bad\0session",1).is_err());
        assert!(AnsfSourceId::new("c-1",0).is_err());
        assert!(AnsfSourceId::parse("ansf:v1:!!!:1").is_err());
    }
    #[test]
    fn direct_source_read_survives_restart_and_denies_other_chat(){
        let root=test_root();
        let book_rel="workspaces/lu/sessions/c-memory.sqlite";
        let book_path=root.join(book_rel);
        fs::create_dir_all(book_path.parent().unwrap()).unwrap();
        let session="c-memory";
        let source_id;
        {
            let book=AnsfStore::open(&book_path).unwrap();
            book.create_session(session,"lu","Conversation").unwrap();
            book.start_chapter(session,"Design").unwrap();
            let id=book.append_entry(session,"turn-1",EntryRole::User,
                "Original decision","ANSF holds the original text").unwrap();
            source_id=book.read_entry(session,id).unwrap().unwrap().stable_source_id().unwrap();
        }
        let library=AnsfLibrary::open(&root.join("library.db")).unwrap();
        library.index_book(&BookIndex{
            session_id:session.into(),workspace_id:"lu".into(),
            relative_path:book_rel.into(),title:"Conversation".into(),summary:"ANSF".into(),
        }).unwrap();
        let resolve=|allow:bool,budget:usize|{
            resolve_known_source(&source_id,"c-current",&library,&Gate(allow),
                |book|AnsfStore::open(&root.join(&book.relative_path))
                    .map_err(|_|DirectReadError::OpenUnavailable),budget)
        };
        let hit=resolve(true,1024).unwrap();
        assert_eq!(hit.content,"ANSF holds the original text");
        assert_eq!(hit.source_role,"user");
        assert_eq!(resolve(false,1024),Err(DirectReadError::AccessDenied));
        assert_eq!(resolve(true,4),Err(DirectReadError::ContentTooLarge));
        assert_eq!(resolve_known_source(
            &AnsfSourceId::new(session,900).unwrap().encode(),"c-current",
            &library,&Gate(true),|book|
                AnsfStore::open(&root.join(&book.relative_path))
                    .map_err(|_|DirectReadError::OpenUnavailable),1024
        ),Err(DirectReadError::EntryMissing));
        drop(library);
        fs::remove_dir_all(root).unwrap();
    }
}
