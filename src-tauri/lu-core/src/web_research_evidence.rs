//! TOOLS-10 citation integrity: persistence and evidence validation, not a search result scraper.
//! The model cannot promote a discovered lead by claiming that it checked the source.
//! Only trusted retrieval adapters supply bibliographic and document observations.
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStatus {
    Discovered,
    MetadataCorroborated,
    DocumentAccessed,
    PassageVerified,
    Inaccessible,
    Mismatch,
}
impl SourceStatus {
    fn code(self) -> &'static str {
        match self {
            Self::Discovered => "DISCOVERED",
            Self::MetadataCorroborated => "METADATA_CORROBORATED",
            Self::DocumentAccessed => "DOCUMENT_ACCESSED",
            Self::PassageVerified => "PASSAGE_VERIFIED",
            Self::Inaccessible => "INACCESSIBLE",
            Self::Mismatch => "MISMATCH",
        }
    }
    fn from_code(code: &str) -> Result<Self, EvidenceError> {
        match code {
            "DISCOVERED" => Ok(Self::Discovered),
            "METADATA_CORROBORATED" => Ok(Self::MetadataCorroborated),
            "DOCUMENT_ACCESSED" => Ok(Self::DocumentAccessed),
            "PASSAGE_VERIFIED" => Ok(Self::PassageVerified),
            "INACCESSIBLE" => Ok(Self::Inaccessible),
            "MISMATCH" => Ok(Self::Mismatch),
            _ => Err(EvidenceError::Invalid),
        }
    }
}
#[derive(Debug)]
pub enum EvidenceError {
    Storage(rusqlite::Error),
    Invalid,
    Missing,
    NotCorroborated,
    NoDocument,
    PassageNotFound,
    MetadataMismatch,
}
impl From<rusqlite::Error> for EvidenceError {
    fn from(value: rusqlite::Error) -> Self { Self::Storage(value) }
}
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: String,
    pub title: String,
    pub author: String,
    pub identifier: String,
}
#[derive(Debug, Clone)]
pub struct TrustedBibliography {
    /// Determined by a host-owned publisher, archive or catalog adapter.
    pub record_url: String,
    pub title: String,
    pub author: String,
    pub identifier: String,
}
#[derive(Debug, Clone)]
pub struct CitationRecord {
    pub id: String,
    pub status: SourceStatus,
    pub title: String,
    pub record_url: Option<String>,
    pub content_sha256: Option<String>,
}
pub struct EvidenceLedger { connection: Connection }
fn bounded(value: &str, cap: usize) -> bool {
    !value.trim().is_empty() && value.len() <= cap && !value.chars().any(char::is_control)
}
fn digest(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}
fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}
impl EvidenceLedger {
    pub fn open(path: &Path) -> Result<Self, EvidenceError> {
        let connection = Connection::open(path)?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
           CREATE TABLE IF NOT EXISTS research_sources(
             id TEXT PRIMARY KEY NOT NULL,
             title TEXT NOT NULL,
             author TEXT NOT NULL,
             identifier TEXT NOT NULL,
             status TEXT NOT NULL,
             record_url TEXT,
             content_sha256 TEXT,
             document_text TEXT
           );
           CREATE TABLE IF NOT EXISTS research_passages(
             source_id TEXT NOT NULL REFERENCES research_sources(id),
             locator TEXT NOT NULL,
             quote TEXT NOT NULL,
             PRIMARY KEY(source_id, locator, quote)
           );
           CREATE TABLE IF NOT EXISTS research_frontier(
             source_id TEXT PRIMARY KEY REFERENCES research_sources(id),
             target_url TEXT NOT NULL,
             state TEXT NOT NULL CHECK(state IN ('QUEUED','IN_PROGRESS','NEEDS_HUMAN','RESOLVED','DENIED')),
             reason TEXT,
             revision INTEGER NOT NULL DEFAULT 0
           );")?;
        Ok(Self { connection })
    }
    pub fn discover(&mut self, candidate: &Candidate) -> Result<(), EvidenceError> {
        if !bounded(&candidate.id, 128) || !bounded(&candidate.title, 1024)
            || !bounded(&candidate.author, 512) || !bounded(&candidate.identifier, 512) {
            return Err(EvidenceError::Invalid);
        }
        // A repeated discovery does not reset a previously checked item.
        self.connection.execute(
            "INSERT OR IGNORE INTO research_sources(id,title,author,identifier,status)
             VALUES (?1,?2,?3,?4,'DISCOVERED')",
            params![candidate.id,candidate.title,candidate.author,candidate.identifier],
        )?;
        Ok(())
    }
    /// Return only the immutable, host-discovered bibliographic identity.
    /// Callers may use this to select a trusted metadata endpoint; the model
    /// must never supply a replacement DOI or promote a citation.
    pub fn discovered_candidate(&self,id:&str)->Result<Candidate,EvidenceError>{
        if !bounded(id,128){return Err(EvidenceError::Invalid);}
        self.connection.query_row(
            "SELECT id,title,author,identifier FROM research_sources WHERE id=?1",
            [id],
            |row|Ok(Candidate{
                id:row.get(0)?,title:row.get(1)?,author:row.get(2)?,
                identifier:row.get(3)?,
            })
        ).optional()?.ok_or(EvidenceError::Missing)
    }
    pub fn status(&self, id: &str) -> Result<SourceStatus, EvidenceError> {
        let raw: Option<String> = self.connection.query_row(
            "SELECT status FROM research_sources WHERE id=?1", [id], |row| row.get(0)
        ).optional()?;
        SourceStatus::from_code(&raw.ok_or(EvidenceError::Missing)?)
    }
    pub fn corroborate(&mut self, id: &str, record: &TrustedBibliography)
        -> Result<(), EvidenceError> {
        if !record.record_url.starts_with("https://") || !bounded(&record.record_url, 2048)
            || !bounded(&record.title, 1024) || !bounded(&record.author, 512)
            || !bounded(&record.identifier, 512) {
            return Err(EvidenceError::Invalid);
        }
        let candidate: Option<(String,String,String)> = self.connection.query_row(
            "SELECT title,author,identifier FROM research_sources WHERE id=?1",
            [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))
        ).optional()?;
        let (title,author,identifier) = candidate.ok_or(EvidenceError::Missing)?;
        if normalize(&title) != normalize(&record.title) || normalize(&author) != normalize(&record.author)
            || normalize(&identifier) != normalize(&record.identifier) {
            self.connection.execute(
                "UPDATE research_sources SET status='MISMATCH' WHERE id=?1", [id])?;
            return Err(EvidenceError::MetadataMismatch);
        }
        self.connection.execute(
            "UPDATE research_sources SET status='METADATA_CORROBORATED', record_url=?2,
             document_text=NULL, content_sha256=NULL WHERE id=?1",
            params![id,record.record_url],
        )?;
        self.connection.execute("DELETE FROM research_passages WHERE source_id=?1", [id])?;
        Ok(())
    }
    pub fn store_accessed_text(&mut self, id: &str, text: &str) -> Result<String, EvidenceError> {
        if self.status(id)? != SourceStatus::MetadataCorroborated {
            return Err(EvidenceError::NotCorroborated);
        }
        if !bounded(text, 4_000_000) { return Err(EvidenceError::Invalid); }
        let hash = digest(text.as_bytes());
        self.connection.execute(
            "UPDATE research_sources SET status='DOCUMENT_ACCESSED',
             document_text=?2,content_sha256=?3 WHERE id=?1",
            params![id,text,hash],
        )?;
        Ok(hash)
    }
    pub fn verify_passage(&mut self, id: &str, locator: &str, quotation: &str)
        -> Result<(), EvidenceError> {
        // Until the retrieval adapter supplies a trusted page/section map,
        // reject user/model-supplied page numbers instead of fabricating locations.
        if locator != "full-text" || !bounded(quotation, 4096) {
            return Err(EvidenceError::Invalid);
        }
        let status = self.status(id)?;
        if status != SourceStatus::DocumentAccessed && status != SourceStatus::PassageVerified {
            return Err(EvidenceError::NoDocument);
        }
        let original: String = self.connection.query_row(
            "SELECT document_text FROM research_sources WHERE id=?1",
            [id], |row| row.get(0)
        )?;
        // Exact Unicode substring only. The host must independently bind locator
        // to a trusted page/section map before representing page numbers as verified.
        if !original.contains(quotation) { return Err(EvidenceError::PassageNotFound); }
        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO research_passages(source_id,locator,quote) VALUES (?1,?2,?3)",
            params![id,locator,quotation],
        )?;
        tx.execute(
            "UPDATE research_sources SET status='PASSAGE_VERIFIED' WHERE id=?1", [id]
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Queue an already-discovered bibliographic lead. The host's network
    /// safety layer must validate and bind origin/DNS before it opens the URL.
    pub fn queue_source(&mut self, id: &str, url: &str) -> Result<(), EvidenceError> {
        self.status(id)?;
        if !url.starts_with("https://") || !bounded(url, 2048) {
            return Err(EvidenceError::Invalid);
        }
        self.connection.execute(
            "INSERT OR IGNORE INTO research_frontier(source_id,target_url,state)
             VALUES (?1,?2,'QUEUED')",
            params![id,url],
        )?;
        Ok(())
    }
    pub fn frontier_state(&self, id: &str) -> Result<String, EvidenceError> {
        self.connection.query_row(
            "SELECT state FROM research_frontier WHERE source_id=?1", [id],
            |row| row.get(0)
        ).optional()?.ok_or(EvidenceError::Missing)
    }
    /// A CAPTCHA or login challenge is an explicit human-assistance state,
    /// not a fetch failure that silently drops the research lead.
    pub fn request_human_assistance(&mut self, id: &str, reason: &str) -> Result<(), EvidenceError> {
        if reason != "CAPTCHA_REQUIRED" && reason != "LOGIN_REQUIRED" {
            return Err(EvidenceError::Invalid);
        }
        let changed=self.connection.execute(
            "UPDATE research_frontier SET state='NEEDS_HUMAN', reason=?2, revision=revision+1
             WHERE source_id=?1 AND state IN ('QUEUED','IN_PROGRESS')",
            params![id,reason],
        )?;
        if changed==0 { return Err(EvidenceError::Invalid); }
        Ok(())
    }
    /// Human completion is asserted by the trusted browser controller after
    /// rechecking the session; this method never solves or bypasses challenges.
    pub fn resume_after_human(&mut self, id: &str) -> Result<(), EvidenceError> {
        let changed=self.connection.execute(
            "UPDATE research_frontier SET state='QUEUED',reason=NULL,revision=revision+1
             WHERE source_id=?1 AND state='NEEDS_HUMAN'", [id]
        )?;
        if changed==0 { return Err(EvidenceError::Invalid); }
        Ok(())
    }
    pub fn deny_source(&mut self, id: &str) -> Result<(), EvidenceError> {
        self.connection.execute(
            "UPDATE research_frontier SET state='DENIED',revision=revision+1 WHERE source_id=?1", [id]
        )?;
        self.block_access(id)
    }
    /// Dispatchers can continue other permitted work while one branch waits
    /// for user interaction. Rows are claimed by an atomic SQLite transition.
    pub fn claim_next_source(&mut self) -> Result<Option<(String,String)>, EvidenceError> {
        let tx=self.connection.transaction()?;
        let next: Option<(String,String)> = tx.query_row(
            "SELECT source_id,target_url FROM research_frontier WHERE state='QUEUED'
             ORDER BY source_id LIMIT 1", [], |row| Ok((row.get(0)?,row.get(1)?))
        ).optional()?;
        if let Some((ref id,_))=next {
            tx.execute(
                "UPDATE research_frontier SET state='IN_PROGRESS',revision=revision+1
                 WHERE source_id=?1 AND state='QUEUED'", [id]
            )?;
        }
        tx.commit()?;
        Ok(next)
    }
    pub fn block_access(&mut self, id: &str) -> Result<(), EvidenceError> {
        if self.status(id)? == SourceStatus::Discovered
            || self.status(id)? == SourceStatus::MetadataCorroborated {
            self.connection.execute("UPDATE research_sources SET status='INACCESSIBLE' WHERE id=?1", [id])?;
        }
        Ok(())
    }
    pub fn record(&self, id: &str) -> Result<CitationRecord, EvidenceError> {
        let record = self.connection.query_row(
            "SELECT title,status,record_url,content_sha256 FROM research_sources WHERE id=?1",
            [id], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,
                           row.get::<_,Option<String>>(2)?,row.get::<_,Option<String>>(3)?))
        ).optional()?.ok_or(EvidenceError::Missing)?;
        Ok(CitationRecord { id:id.into(), title:record.0, status:SourceStatus::from_code(&record.1)?,
             record_url:record.2, content_sha256:record.3 })
    }
    /// Proof an exact quotation was checked against the pinned document.
    /// This does not itself certify the model's interpretation of its meaning.
    pub fn has_verified_passage(&self,id:&str,quote:&str)->Result<bool,EvidenceError> {
        if self.status(id)?!=SourceStatus::PassageVerified {return Ok(false);}
        let stored:Option<i64>=self.connection.query_row(
            "SELECT 1 FROM research_passages WHERE source_id=?1 AND quote=?2 LIMIT 1",
            params![id,quote], |row|row.get(0)
        ).optional()?;
        Ok(stored.is_some())
    }
    /// Final report gate: only records with a real corroborated identifier
    /// AND actual accessed text may be listed as reviewed sources.
    pub fn eligible_for_reviewed_bibliography(&self, id: &str) -> Result<bool, EvidenceError> {
        let record = self.record(id)?;
        Ok(matches!(record.status, SourceStatus::DocumentAccessed | SourceStatus::PassageVerified)
            && record.record_url.is_some() && record.content_sha256.is_some())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn candidate() -> Candidate { Candidate {
        id:"source-1".into(),title:"A Verified Study".into(),
        author:"Jane Doe".into(),identifier:"doi:10.1000/example".into()
    } }
    fn record() -> TrustedBibliography { TrustedBibliography {
        record_url:"https://publisher.example/study".into(),
        title:"A Verified Study".into(),author:"Jane Doe".into(),
        identifier:"doi:10.1000/example".into()
    } }
    #[test]
    fn model_suggestion_never_counts_as_a_reviewed_source() {
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        ledger.discover(&candidate()).unwrap();
        ledger.discover(&candidate()).unwrap();
        assert_eq!(ledger.status("source-1").unwrap(),SourceStatus::Discovered);
        assert!(!ledger.eligible_for_reviewed_bibliography("source-1").unwrap());
        assert!(matches!(ledger.verify_passage("source-1","full-text","invented quote"),Err(EvidenceError::NoDocument)));
    }
    #[test]
    fn genuine_identifier_wrong_title_does_not_verify() {
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        ledger.discover(&candidate()).unwrap();
        let mut wrong=record(); wrong.title="An Unrelated Study".into();
        assert!(matches!(ledger.corroborate("source-1",&wrong),Err(EvidenceError::MetadataMismatch)));
        assert_eq!(ledger.status("source-1").unwrap(),SourceStatus::Mismatch);
        assert!(!ledger.eligible_for_reviewed_bibliography("source-1").unwrap());
    }
    #[test]
    fn real_metadata_is_not_full_text_and_invented_quote_is_rejected() {
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        ledger.discover(&candidate()).unwrap();
        ledger.corroborate("source-1",&record()).unwrap();
        assert!(!ledger.eligible_for_reviewed_bibliography("source-1").unwrap());
        assert!(matches!(ledger.verify_passage("source-1","full-text","quoted"),Err(EvidenceError::NoDocument)));
        let checksum=ledger.store_accessed_text("source-1","Evidence supports a nuanced argument.").unwrap();
        assert_eq!(checksum.len(),64);
        assert!(ledger.eligible_for_reviewed_bibliography("source-1").unwrap());
        assert!(matches!(ledger.verify_passage("source-1","full-text","This is made up."),Err(EvidenceError::PassageNotFound)));
        assert!(matches!(ledger.verify_passage("source-1","p. 99","Evidence supports"),Err(EvidenceError::Invalid)));
        ledger.verify_passage("source-1","full-text","Evidence supports").unwrap();
        assert_eq!(ledger.status("source-1").unwrap(),SourceStatus::PassageVerified);
    }
    #[test]
    fn a_blocked_page_never_gets_promoted() {
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        ledger.discover(&candidate()).unwrap();
        ledger.block_access("source-1").unwrap();
        assert_eq!(ledger.status("source-1").unwrap(),SourceStatus::Inaccessible);
        assert!(!ledger.eligible_for_reviewed_bibliography("source-1").unwrap());
    }
    #[test]
    fn captcha_pauses_one_branch_without_losing_other_sources() {
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        ledger.discover(&candidate()).unwrap();
        let mut second=candidate(); second.id="source-2".into();
        ledger.discover(&second).unwrap();
        ledger.queue_source("source-1","https://archive.example/first").unwrap();
        ledger.queue_source("source-2","https://archive.example/second").unwrap();
        assert_eq!(ledger.claim_next_source().unwrap().unwrap().0,"source-1");
        ledger.request_human_assistance("source-1","CAPTCHA_REQUIRED").unwrap();
        assert_eq!(ledger.frontier_state("source-1").unwrap(),"NEEDS_HUMAN");
        assert_eq!(ledger.claim_next_source().unwrap().unwrap().0,"source-2");
        assert!(ledger.claim_next_source().unwrap().is_none());
        assert!(!ledger.eligible_for_reviewed_bibliography("source-1").unwrap());
        ledger.resume_after_human("source-1").unwrap();
        assert_eq!(ledger.claim_next_source().unwrap().unwrap().0,"source-1");
    }
    #[test]
    fn denied_source_cannot_be_resumed_as_captcha() {
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        ledger.discover(&candidate()).unwrap();
        ledger.queue_source("source-1","https://archive.example/first").unwrap();
        ledger.deny_source("source-1").unwrap();
        assert_eq!(ledger.frontier_state("source-1").unwrap(),"DENIED");
        assert!(ledger.resume_after_human("source-1").is_err());
        assert!(ledger.claim_next_source().unwrap().is_none());
        assert_eq!(ledger.status("source-1").unwrap(),SourceStatus::Inaccessible);
    }

}
