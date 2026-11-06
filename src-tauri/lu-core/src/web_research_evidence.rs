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
/// Browser text is a host-observed, untrusted website snapshot. It is NOT
/// corroborated bibliographic metadata or a reviewed academic citation.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct BrowserEvidenceRecord {
    pub generation_id:String,
    pub call_id:String,
    pub url:String,
    pub observation_kind:String,
    pub observed_sha256:String,
    pub excerpt:String,
    pub excerpt_truncated:bool,
    pub observed_text_bytes:usize,
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
           CREATE TABLE IF NOT EXISTS research_browser_observations(
             generation_id TEXT NOT NULL,
             call_id TEXT NOT NULL,
             source_url TEXT NOT NULL,
             observation_kind TEXT NOT NULL
               CHECK(observation_kind IN ('READ','ACCESSIBILITY')),
             observed_sha256 TEXT NOT NULL,
             excerpt TEXT NOT NULL,
             excerpt_truncated INTEGER NOT NULL
               CHECK(excerpt_truncated IN (0,1)),
             observed_text_bytes INTEGER NOT NULL,
             PRIMARY KEY(generation_id,call_id)
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
    /// Called ONLY after a trusted Browser worker's read/accessibility
    /// response has been admitted by the host's active per-turn grant and
    /// post-redirect HTTPS origin scope. Never accept a model-supplied page,
    /// URL, hash or citation status as a replacement for that response.
    ///
    /// The excerpt is bounded; the digest covers the full observed text.
    /// This cannot promote any academic candidate to DOCUMENT_ACCESSED or
    /// PASSAGE_VERIFIED. Identity is immutable per (generation, native call).
    pub fn observe_browser_text(
        &mut self,generation_id:&str,call_id:&str,source_url:&str,
        kind:&str,text:&str,worker_truncated:bool
    )->Result<BrowserEvidenceRecord,EvidenceError>{
        let safe_id=|value:&str,prefix:&str|{
            value.starts_with(prefix)&&value.len()<=128
                &&value.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
        };
        if !safe_id(generation_id,"g-")||!safe_id(call_id,"browser-call-")
            ||!bounded(source_url,2048)||!source_url.starts_with("https://")
            ||source_url.chars().any(char::is_whitespace)
            ||!matches!(kind,"READ"|"ACCESSIBILITY")
            ||text.trim().is_empty()||text.len()>524_288
            ||text.chars().any(|c|c=='\0'||(c.is_control()&&c!='\n'&&c!='\r'&&c!='\t')){
            return Err(EvidenceError::Invalid);
        }
        let excerpt:String=text.chars().take(8192).collect();
        let excerpt_truncated=worker_truncated||excerpt.len()<text.len();
        let sha=digest(text.as_bytes());
        let expected=BrowserEvidenceRecord{
            generation_id:generation_id.into(),call_id:call_id.into(),
            url:source_url.into(),observation_kind:kind.into(),
            observed_sha256:sha,excerpt,
            excerpt_truncated,observed_text_bytes:text.len(),
        };
        // INSERT OR IGNORE only provides idempotency when an existing receipt
        // matches *every* field. A reused call id cannot rewrite history.
        self.connection.execute("INSERT OR IGNORE INTO research_browser_observations
            (generation_id,call_id,source_url,observation_kind,observed_sha256,
             excerpt,excerpt_truncated,observed_text_bytes)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![generation_id,call_id,source_url,kind,
                &expected.observed_sha256,&expected.excerpt,
                i64::from(excerpt_truncated),text.len() as i64])?;
        let stored=self.browser_observation(generation_id,call_id)?;
        if stored!=expected{return Err(EvidenceError::Invalid);}
        Ok(stored)
    }
    /// Inspect only the exact bounded snapshot admitted by the native host,
    /// never live page state or model prose.
    pub fn browser_observation(
        &self,generation_id:&str,call_id:&str
    )->Result<BrowserEvidenceRecord,EvidenceError>{
        if !generation_id.starts_with("g-")||!call_id.starts_with("browser-call-")
            ||generation_id.len()>128||call_id.len()>128
            ||generation_id.bytes().any(|b|!b.is_ascii_alphanumeric()&&b!=b'-'&&b!=b'_')
            ||call_id.bytes().any(|b|!b.is_ascii_alphanumeric()&&b!=b'-'&&b!=b'_'){
            return Err(EvidenceError::Invalid);
        }
        self.connection.query_row(
            "SELECT source_url,observation_kind,observed_sha256,excerpt,
             excerpt_truncated,observed_text_bytes FROM research_browser_observations
             WHERE generation_id=?1 AND call_id=?2",
            params![generation_id,call_id],|row|Ok(BrowserEvidenceRecord{
                generation_id:generation_id.into(),call_id:call_id.into(),
                url:row.get(0)?,observation_kind:row.get(1)?,
                observed_sha256:row.get(2)?,excerpt:row.get(3)?,
                excerpt_truncated:row.get::<_,i64>(4)?==1,
                observed_text_bytes:row.get::<_,i64>(5)? as usize,
            })
        ).optional()?.ok_or(EvidenceError::Missing)
    }
    /// Read a bounded, most-recent-first list of observations for one
    /// already chat-scoped SQLite ledger. Never join this with the
    /// bibliographic research_sources table or inherit its citation status.
    pub fn recent_browser_observations(
        &self,limit:usize
    )->Result<Vec<BrowserEvidenceRecord>,EvidenceError>{
        if !(1..=32).contains(&limit){return Err(EvidenceError::Invalid);}
        let mut stmt=self.connection.prepare(
            "SELECT generation_id,call_id,source_url,observation_kind,
             observed_sha256,excerpt,excerpt_truncated,observed_text_bytes
             FROM research_browser_observations ORDER BY rowid DESC LIMIT ?1"
        )?;
        let mapped=stmt.query_map([limit as i64],|row|Ok(BrowserEvidenceRecord{
            generation_id:row.get(0)?,call_id:row.get(1)?,
            url:row.get(2)?,observation_kind:row.get(3)?,
            observed_sha256:row.get(4)?,excerpt:row.get(5)?,
            excerpt_truncated:row.get::<_,i64>(6)?==1,
            observed_text_bytes:row.get::<_,i64>(7)? as usize,
        }))?;
        Ok(mapped.collect::<Result<_,_>>()?)
    }
    /// Exact quotation appears in retained Browser excerpt, not a validated
    /// publication. The returned boolean MUST NOT be treated as a
    /// bibliographic PASSAGE_VERIFIED status.
    pub fn has_browser_excerpt(&self,generation_id:&str,call_id:&str,quotation:&str)
      ->Result<bool,EvidenceError>{
        if !bounded(quotation,4096){return Err(EvidenceError::Invalid);}
        Ok(self.browser_observation(generation_id,call_id)?
            .excerpt.contains(quotation))
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
    fn discovered_candidate_is_exact_identity_and_cannot_mint_a_new_doi(){
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        assert!(matches!(ledger.discovered_candidate("source-1"),Err(EvidenceError::Missing)));
        ledger.discover(&candidate()).unwrap();
        let stored=ledger.discovered_candidate("source-1").unwrap();
        assert_eq!(stored.identifier,"doi:10.1000/example");
        assert_eq!(stored.title,"A Verified Study");
        assert_eq!(stored.author,"Jane Doe");
        assert!(matches!(ledger.discovered_candidate("source-1\\nOTHER"),Err(EvidenceError::Invalid)));
        assert!(matches!(ledger.discovered_candidate("source-2"),Err(EvidenceError::Missing)));
        assert!(!ledger.eligible_for_reviewed_bibliography("source-1").unwrap());
    }
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


#[cfg(test)]
mod browser_evidence_tests {
    use super::*;
    #[test]
    fn browser_reads_are_immutable_observations_not_bibliographic_verification(){
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        ledger.discover(&Candidate{
            id:"paper-1".into(),title:"Study".into(),
            author:"Author".into(),identifier:"10.1234/example".into()
        }).unwrap();
        let captured=ledger.observe_browser_text("g-1","browser-call-1",
            "https://archive.example/article","READ","A genuine extracted passage",false).unwrap();
        assert_eq!(captured.observed_sha256,digest(b"A genuine extracted passage"));
        assert!(ledger.has_browser_excerpt("g-1","browser-call-1","extracted passage").unwrap());
        assert!(!ledger.has_browser_excerpt("g-1","browser-call-1","fabricated").unwrap());
        assert!(!ledger.eligible_for_reviewed_bibliography("paper-1").unwrap());
        assert_eq!(ledger.status("paper-1").unwrap(),SourceStatus::Discovered);
        assert_eq!(ledger.recent_browser_observations(16).unwrap(),vec![captured.clone()]);
        assert!(ledger.recent_browser_observations(0).is_err());
        assert_eq!(ledger.observe_browser_text("g-1","browser-call-1",
            "https://archive.example/article","READ","A genuine extracted passage",false)
            .unwrap(),captured);
        assert!(matches!(ledger.observe_browser_text("g-1","browser-call-1",
            "https://archive.example/changed","READ","A genuine extracted passage",false),
            Err(EvidenceError::Invalid)));
        assert!(matches!(ledger.browser_observation("g-2","browser-call-1"),
            Err(EvidenceError::Missing)));
    }
    #[test]
    fn browser_observations_survive_restart_without_gaining_citation_authority(){
        let path=std::env::temp_dir().join(format!("lu-browser-evidence-{}-{}.sqlite",
            std::process::id(),std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        {
            let mut ledger=EvidenceLedger::open(&path).unwrap();
            let saved=ledger.observe_browser_text("g-persist","browser-call-persist",
                "https://public.example/landing","READ",
                "Observed document text, not a corroborated paper",false).unwrap();
            assert_eq!(saved.observed_text_bytes,46);
        }
        {
            let ledger=EvidenceLedger::open(&path).unwrap();
            let saved=ledger.browser_observation("g-persist","browser-call-persist").unwrap();
            assert!(saved.excerpt.contains("Observed document text"));
            assert_eq!(ledger.recent_browser_observations(24).unwrap().len(),1);
            assert!(matches!(ledger.status("g-persist"),Err(EvidenceError::Missing)));
        }
        let _=std::fs::remove_file(&path);
        let _=std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _=std::fs::remove_file(path.with_extension("sqlite-shm"));
    }
    #[test]
    fn browser_receipts_reject_unowned_and_non_read_claims(){
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        for (generation,call,url,kind,text) in [
            ("wrong","browser-call-1","https://example.org","READ","Text"),
            ("g-1","call-1","https://example.org","READ","Text"),
            ("g-1","browser-call-1","http://example.org","READ","Text"),
            ("g-1","browser-call-1","https://example.org","NAVIGATED","Text"),
            ("g-1","browser-call-1","https://example.org","READ",""),
            ("g-1","browser-call-1","https://example.org","READ","Bad\0Text"),
            ("g-1","browser-call-1","https://example.org\nbad","READ","Text"),
        ]{
            assert!(matches!(ledger.observe_browser_text(
                generation,call,url,kind,text,false),Err(EvidenceError::Invalid)));
        }
        let long="X".repeat(9000);
        let saved=ledger.observe_browser_text("g-2","browser-call-2",
            "https://archive.example/long","ACCESSIBILITY",&long,true).unwrap();
        assert_eq!(saved.excerpt.chars().count(),8192);
        assert!(saved.excerpt_truncated);
        assert_eq!(saved.observed_text_bytes,9000);
        assert!(!ledger.has_browser_excerpt("g-2","browser-call-2","Not present").unwrap());
    }
}
