//! ANSF multimodal source REFERENCES ONLY: never copies images/video/audio
//! into conversation SQLite. Artifact storage and external-file permission
//! belong to the trusted host's separate file/artifact layer.
//!
//! ANSF owns the conversation-to-artifact link and optional expected identity.
//! A host resolves an opaque artifact ID and supplies an already-open file
//! handle for bounded presence and integrity checking.
use crate::ansf::{AnsfError,AnsfStore};
use rusqlite::{params,Connection,OptionalExtension};
use sha2::{Digest,Sha256};
use std::{fs::File,io::{Read,Seek,SeekFrom}};

const MAX_NAME:usize=256;
const MAX_KIND:usize=128;
const MAX_ID:usize=128;
const MAX_LINKS_PER_ENTRY:usize=128;
pub const MAX_INLINE_HASH_BYTES:u64=64*1024*1024;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ArtifactLink {
    pub session_id:String,
    pub entry_id:i64,
    /// Opaque host-issued key in a separate artifact registry or OS picker
    /// handle; never a model-controlled absolute path or executable URL.
    pub artifact_id:String,
    pub display_name:String,
    pub media_type:String,
    pub expected_bytes:Option<u64>,
    pub expected_sha256:Option<String>,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ArtifactPresence {
    /// The host opened a real file and checked accessible metadata, but did
    /// not establish byte-level identity (e.g., huge files or absent hash).
    PresentUnverified,
    /// An authorized file handle's complete bytes matched the expected hash
    /// within the explicit work limit.
    ContentVerified,
    /// Declared size or complete content hash disagrees.
    Changed,
}
fn valid_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=MAX_ID&&
    s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn valid_hash(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn validate(link:&ArtifactLink)->bool{
    valid_id(&link.session_id) &&
    valid_id(&link.artifact_id) &&
    link.entry_id>0 &&
    !link.display_name.trim().is_empty() &&
    link.display_name.len()<=MAX_NAME &&
    !link.display_name.chars().any(|c|matches!(c,'\0'|'\n'|'\r'|'/'|'\\')) &&
    !link.media_type.trim().is_empty() &&
    link.media_type.len()<=MAX_KIND &&
    !link.media_type.contains(['\0','\n','\r']) &&
    link.expected_bytes.is_none_or(|n|n<=i64::MAX as u64) &&
    link.expected_sha256.as_deref().is_none_or(valid_hash)
}
pub(crate) fn initialize_artifact_schema(conn:&Connection)->Result<(),AnsfError>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_artifact_links(
           session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
           entry_id INTEGER NOT NULL REFERENCES ansf_entries(id),
           artifact_id TEXT NOT NULL,
           display_name TEXT NOT NULL,
           media_type TEXT NOT NULL,
           expected_bytes INTEGER,
           expected_sha256 TEXT,
           PRIMARY KEY(session_id,entry_id,artifact_id)
         );
         CREATE INDEX IF NOT EXISTS ansf_artifact_links_by_artifact
           ON ansf_artifact_links(artifact_id,session_id,entry_id);"
    )?;
    Ok(())
}
impl AnsfStore {
    /// Append a reference to an EXISTING exact ANSF conversation entry.
    /// Duplicate replay is idempotent but an altered reference is refused.
    /// This function never opens, copies, downloads or executes an artifact.
    pub fn append_artifact_link(&self,link:&ArtifactLink)->Result<(),AnsfError>{
        if !validate(link){return Err(AnsfError::InvalidInput)}
        let mut conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let tx=conn.transaction()?;
        let actual_session:Option<String>=tx.query_row(
            "SELECT session_id FROM ansf_entries WHERE id=?1",
            [link.entry_id],
            |r|r.get(0),
        ).optional()?;
        if actual_session.as_deref()!=Some(link.session_id.as_str()){
            return Err(AnsfError::InvalidInput)
        }
        let existing:Option<ArtifactLink>=tx.query_row(
            "SELECT session_id,entry_id,artifact_id,display_name,media_type,
                    expected_bytes,expected_sha256
             FROM ansf_artifact_links
             WHERE session_id=?1 AND entry_id=?2 AND artifact_id=?3",
            params![link.session_id,link.entry_id,link.artifact_id],
            map_link,
        ).optional()?;
        if let Some(previous)=existing{
            if previous==*link{return Ok(())}
            return Err(AnsfError::InvalidInput)
        }
        let count:i64=tx.query_row(
            "SELECT COUNT(*) FROM ansf_artifact_links
             WHERE session_id=?1 AND entry_id=?2",
            params![link.session_id,link.entry_id],|r|r.get(0))?;
        if count>=MAX_LINKS_PER_ENTRY as i64{return Err(AnsfError::InvalidInput);}
        tx.execute(
            "INSERT INTO ansf_artifact_links
             (session_id,entry_id,artifact_id,display_name,media_type,
              expected_bytes,expected_sha256)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![link.session_id,link.entry_id,link.artifact_id,
                    link.display_name,link.media_type,
                    link.expected_bytes.map(|n|n as i64),link.expected_sha256],
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Only returns metadata and opaque IDs. The trusted host independently
    /// authorizes and probes the referenced external object when needed.
    pub fn artifact_links_for_entry(
        &self,session_id:&str,entry_id:i64
    )->Result<Vec<ArtifactLink>,AnsfError>{
        if !valid_id(session_id)||entry_id<=0{return Err(AnsfError::InvalidInput)}
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let mut stmt=conn.prepare(
            "SELECT session_id,entry_id,artifact_id,display_name,media_type,
                    expected_bytes,expected_sha256
             FROM ansf_artifact_links
             WHERE session_id=?1 AND entry_id=?2 ORDER BY artifact_id LIMIT 128"
        )?;
        let rows=stmt.query_map(params![session_id,entry_id],map_link)?;
        rows.collect::<Result<Vec<_>,_>>().map_err(Into::into)
    }
}
fn map_link(r:&rusqlite::Row<'_>)->rusqlite::Result<ArtifactLink>{
    let bytes:Option<i64>=r.get(5)?;
    Ok(ArtifactLink{
        session_id:r.get(0)?,entry_id:r.get(1)?,artifact_id:r.get(2)?,
        display_name:r.get(3)?,media_type:r.get(4)?,
        expected_bytes:bytes.map(|n|n as u64),expected_sha256:r.get(6)?,
    })
}
/// Verify a trusted, already-open file. The caller must have independently
/// authorized its origin/ownership and rejected unsafe path traversal.
/// Passing a file descriptor/handle avoids any model-controlled path opens.
///
/// Size and accessibility are cheap presence tests. Complete hashing is
/// explicit and bounded; an unverified large file is not called "verified".
/// For terabyte objects schedule independent streamed hashing in a worker.
pub fn probe_granted_file(
    link:&ArtifactLink,file:&mut File,max_hash_bytes:u64,
)->std::io::Result<ArtifactPresence>{
    if !validate(link){
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,
            "Invalid ANSF artifact reference"))
    }
    let meta=file.metadata()?;
    if !meta.is_file() || link.expected_bytes.is_some_and(|n|n!=meta.len()){
        return Ok(ArtifactPresence::Changed)
    }
    if max_hash_bytes==0||max_hash_bytes>MAX_INLINE_HASH_BYTES||
       meta.len()>max_hash_bytes||link.expected_sha256.is_none(){
        return Ok(ArtifactPresence::PresentUnverified)
    }
    let initial_offset=file.stream_position()?;
    file.seek(SeekFrom::Start(0))?;
    let mut hash=Sha256::new();
    let mut buffer=[0u8;64*1024];
    let mut remaining=meta.len();
    let verified=(||->std::io::Result<bool>{
        while remaining>0{
            let want=(remaining as usize).min(buffer.len());
            let n=file.read(&mut buffer[..want])?;
            if n==0{return Ok(false)}
            hash.update(&buffer[..n]);
            remaining-=n as u64;
        }
        Ok(file.metadata()?.len()==meta.len())
    })();
    // Leave caller's file cursor where it was, including on hash failures.
    let restore=file.seek(SeekFrom::Start(initial_offset));
    let coherent=verified?;
    restore?;
    if !coherent || format!("{:x}",hash.finalize())!=link.expected_sha256.as_deref().unwrap_or("") {
        Ok(ArtifactPresence::Changed)
    } else {
        Ok(ArtifactPresence::ContentVerified)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ansf::EntryRole;
    use std::{fs,io::Write,time::{SystemTime,UNIX_EPOCH}};
    #[test]
    fn link_is_metadata_only_and_file_presence_is_separate(){
        let root=std::env::temp_dir().join(format!("lu-ansf-artifacts-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let book=AnsfStore::open(&root.join("book.sqlite")).unwrap();
        book.create_session("session","lu","Conversation").unwrap();
        book.create_session("other","lu","Other").unwrap();
        book.start_chapter("session","Context").unwrap();
        book.start_chapter("other","Context").unwrap();
        let entry=book.append_entry("session","m-1",EntryRole::User,
            "A video artifact","See referenced movie, do not embed it").unwrap();
        let file_path=root.join("film.mp4");
        let bytes=b"test-small-media";
        fs::write(&file_path,bytes).unwrap();
        let expected=format!("{:x}",Sha256::digest(bytes));
        let link=ArtifactLink{
            session_id:"session".into(),entry_id:entry,
            artifact_id:"asset-123".into(),display_name:"film.mp4".into(),
            media_type:"video/mp4".into(),expected_bytes:Some(bytes.len() as u64),
            expected_sha256:Some(expected),
        };
        book.append_artifact_link(&link).unwrap();
        book.append_artifact_link(&link).unwrap();
        assert_eq!(book.artifact_links_for_entry("session",entry).unwrap(),vec![link.clone()]);
        assert_eq!(book.artifact_links_for_entry("other",entry).unwrap(),vec![]);
        let mut bad=link.clone();bad.media_type="application/x-forged".into();
        assert!(book.append_artifact_link(&bad).is_err());
        bad=link.clone();bad.session_id="other".into();
        assert!(book.append_artifact_link(&bad).is_err());
        let source=book.read_entry("session",entry).unwrap().unwrap();
        assert_eq!(source.content,"See referenced movie, do not embed it");
        let mut opened=File::open(&file_path).unwrap();
        assert_eq!(probe_granted_file(&link,&mut opened,0).unwrap(),
            ArtifactPresence::PresentUnverified);
        assert_eq!(probe_granted_file(&link,&mut opened,1024).unwrap(),
            ArtifactPresence::ContentVerified);
        drop(opened);
        let mut changed=File::create(&file_path).unwrap();
        changed.write_all(b"altered").unwrap();drop(changed);
        let mut opened=File::open(&file_path).unwrap();
        assert_eq!(probe_granted_file(&link,&mut opened,1024).unwrap(),
            ArtifactPresence::Changed);
        drop(opened);drop(book);
        fs::remove_dir_all(root).unwrap();
    }
}
