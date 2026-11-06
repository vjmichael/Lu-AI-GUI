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
/// Evidence of one exact quotation in the retained Browser snapshot.
/// A publisher-linked match proves an independently corroborated catalog
/// record pointed to that *exact URL*, not that a full paper was downloaded.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct BrowserPassageProof {
    pub generation_id:String,
    pub call_id:String,
    pub source_url:String,
    pub observed_sha256:String,
    pub exact_quote:String,
    pub source_id:Option<String>,
    pub publisher_link_matched:bool,
}
/// Immutable response extraction receipt. Only complete, directly
/// catalog-linked PDF text can promote DOCUMENT_ACCESSED. Other content
/// remains a host-observed document candidate.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct DocumentRetrievalProof {
    pub source_id:String,
    pub catalog_url:String,
    pub final_url:String,
    pub raw_sha256:String,
    pub text_sha256:String,
    pub chain_sha256:String,
    pub format:String,
    pub truncated:bool,
    pub document_accessed:bool,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ResearchCampaignStatus {
    pub id:String,
    pub query:String,
    pub state:String,
    pub next_page:u32,
    pub candidate_ceiling:u32,
    pub distinct_dois:u32,
    pub completed_pages:u32,
    pub blocked_reason:Option<String>,
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
           CREATE TABLE IF NOT EXISTS research_campaigns(
             id TEXT PRIMARY KEY,
             query TEXT NOT NULL,
             candidate_ceiling INTEGER NOT NULL CHECK(candidate_ceiling BETWEEN 1 AND 120),
             next_page INTEGER NOT NULL CHECK(next_page BETWEEN 1 AND 6),
             state TEXT NOT NULL CHECK(state IN ('READY','RUNNING','PAUSED','BLOCKED','DONE','CANCELLED')),
             blocked_reason TEXT
           );
           CREATE TABLE IF NOT EXISTS research_campaign_pages(
             campaign_id TEXT NOT NULL REFERENCES research_campaigns(id),
             page INTEGER NOT NULL,
             response_sha256 TEXT NOT NULL,
             PRIMARY KEY(campaign_id,page)
           );
           CREATE TABLE IF NOT EXISTS research_campaign_leads(
             campaign_id TEXT NOT NULL REFERENCES research_campaigns(id),
             doi TEXT NOT NULL,
             source_id TEXT NOT NULL REFERENCES research_sources(id),
             first_page INTEGER NOT NULL,
             PRIMARY KEY(campaign_id,doi)
           );
           CREATE TABLE IF NOT EXISTS research_campaign_source_review(
             campaign_id TEXT NOT NULL REFERENCES research_campaigns(id),
             source_id TEXT NOT NULL REFERENCES research_sources(id),
             stage TEXT NOT NULL CHECK(stage IN (
                 'METADATA_CORROBORATED','DOCUMENT_ACCESSED',
                 'METADATA_MISMATCH','DOCUMENT_UNAVAILABLE','ERROR'
             )),
             reason_code TEXT NOT NULL,
             PRIMARY KEY(campaign_id,source_id)
           );
           CREATE TABLE IF NOT EXISTS research_document_observations(
             source_id TEXT NOT NULL REFERENCES research_sources(id),
             chain_sha256 TEXT NOT NULL,
             catalog_url TEXT NOT NULL,
             final_url TEXT NOT NULL,
             raw_sha256 TEXT NOT NULL,
             extracted_sha256 TEXT NOT NULL,
             format TEXT NOT NULL CHECK(format IN ('PDF','HTML')),
             truncated INTEGER NOT NULL CHECK(truncated IN (0,1)),
             document_accessed INTEGER NOT NULL CHECK(document_accessed IN (0,1)),
             PRIMARY KEY(source_id,chain_sha256)
           );
           CREATE TABLE IF NOT EXISTS research_publisher_links(
             source_id TEXT PRIMARY KEY NOT NULL REFERENCES research_sources(id),
             publisher_url TEXT NOT NULL,
             catalog_response_sha256 TEXT NOT NULL
           );
           CREATE TABLE IF NOT EXISTS research_browser_passage_proofs(
             generation_id TEXT NOT NULL,
             call_id TEXT NOT NULL,
             source_id TEXT NOT NULL,
             quote TEXT NOT NULL,
             observed_sha256 TEXT NOT NULL,
             publisher_link_matched INTEGER NOT NULL CHECK(publisher_link_matched IN (0,1)),
             PRIMARY KEY(generation_id,call_id,source_id,quote)
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
        let (original,pinned_sha):(String,String)=self.connection.query_row(
            "SELECT document_text,content_sha256 FROM research_sources WHERE id=?1",
            [id], |row| Ok((row.get(0)?,row.get(1)?))
        )?;
        // Recheck the immutable content digest, even for already verified
        // documents. A stale/tampered SQLite body cannot prove a passage.
        if digest(original.as_bytes())!=pinned_sha {
            return Err(EvidenceError::NoDocument);
        }
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
    /// Called only by the host after separately fetching and validating the
    /// independent Crossref work record. A DOI resolver redirect is NOT
    /// evidence of a publisher match: the actual Browser URL must equal this
    /// exact catalog-provided HTTPS URL for a linked passage proof.
    pub fn bind_crossref_publisher_url(
        &mut self,source_id:&str,publisher_url:&str,catalog_sha256:&str
    )->Result<(),EvidenceError>{
        if self.status(source_id)?!=SourceStatus::MetadataCorroborated
            ||!bounded(publisher_url,2048)||!publisher_url.starts_with("https://")
            ||publisher_url.chars().any(char::is_whitespace)
            ||publisher_url.contains('\\')||publisher_url.contains('@')
            ||catalog_sha256.len()!=64
            ||!catalog_sha256.bytes().all(|b|b.is_ascii_hexdigit()){
            return Err(EvidenceError::Invalid);
        }
        self.connection.execute(
            "INSERT OR IGNORE INTO research_publisher_links
             (source_id,publisher_url,catalog_response_sha256) VALUES (?1,?2,?3)",
            params![source_id,publisher_url,catalog_sha256]
        )?;
        let existing:(String,String)=self.connection.query_row(
            "SELECT publisher_url,catalog_response_sha256 FROM research_publisher_links
             WHERE source_id=?1",[source_id],|r|Ok((r.get(0)?,r.get(1)?))
        )?;
        if existing.0!=publisher_url||existing.1!=catalog_sha256 {
            return Err(EvidenceError::Invalid);
        }
        Ok(())
    }
    pub fn recent_campaigns(
        &self,limit:usize
    )->Result<Vec<ResearchCampaignStatus>,EvidenceError>{
        if !(1..=32).contains(&limit){return Err(EvidenceError::Invalid);}
        let mut stmt=self.connection.prepare(
            "SELECT id FROM research_campaigns ORDER BY rowid DESC LIMIT ?1"
        )?;
        let rows=stmt.query_map([limit as i64],|r|r.get::<_,String>(0))?
            .collect::<Result<Vec<_>,_>>()?;
        rows.iter().map(|id|self.campaign_status(id)).collect()
    }
    /// Deterministically choose the next unreviewed DOI leads for a
    /// separately user-approved bounded pipeline step. No URL, DOI or
    /// source identity can be provided by the model.
    pub fn campaign_pending_sources(
        &self,id:&str,limit:usize
    )->Result<Vec<String>,EvidenceError>{
        if !Self::campaign_safe_id(id)||!(1..=10).contains(&limit){
            return Err(EvidenceError::Invalid);
        }
        let _=self.campaign_status(id)?;
        let mut stmt=self.connection.prepare(
            "SELECT l.source_id FROM research_campaign_leads l
             LEFT JOIN research_campaign_source_review r
               ON l.campaign_id=r.campaign_id AND l.source_id=r.source_id
             WHERE l.campaign_id=?1 AND r.source_id IS NULL
             ORDER BY l.first_page ASC,l.doi ASC LIMIT ?2"
        )?;
        let rows=stmt.query_map(params![id,limit as i64],|r|r.get(0))?;
        Ok(rows.collect::<Result<Vec<_>,_>>()?)
    }
    pub fn campaign_review_progress(
        &self,id:&str
    )->Result<(u32,u32,u32),EvidenceError>{
        let status=self.campaign_status(id)?;
        let (reviewed,full_text):(i64,i64)=self.connection.query_row(
            "SELECT COUNT(*),COALESCE(SUM(CASE WHEN stage='DOCUMENT_ACCESSED'
                  THEN 1 ELSE 0 END),0)
             FROM research_campaign_source_review WHERE campaign_id=?1",
            [id],|r|Ok((r.get(0)?,r.get(1)?))
        )?;
        Ok((status.distinct_dois,reviewed as u32,full_text as u32))
    }
    /// One immutable per-source pipeline receipt. Retry of a failed
    /// publisher is an explicit new operator action, not an auto-loop.
    pub fn record_campaign_source_review(
        &mut self,id:&str,source_id:&str,stage:&str,reason:&str
    )->Result<(),EvidenceError>{
        if !Self::campaign_safe_id(id)||!bounded(source_id,128)
            ||!matches!(stage,
                "METADATA_CORROBORATED"|"DOCUMENT_ACCESSED"
                |"METADATA_MISMATCH"|"DOCUMENT_UNAVAILABLE"|"ERROR")
            ||!matches!(reason,
                "OK"|"CATALOG_MISMATCH"|"PDF_UNSUPPORTED"
                |"PUBLISHER_UNAVAILABLE"|"RETRIEVAL_FAILED"
                |"UNSUPPORTED_CONTENT"|"METADATA_FAILED"){
            return Err(EvidenceError::Invalid);
        }
        let exists:Option<i64>=self.connection.query_row(
            "SELECT 1 FROM research_campaign_leads
             WHERE campaign_id=?1 AND source_id=?2 LIMIT 1",
            params![id,source_id],|r|r.get(0)
        ).optional()?;
        if exists.is_none(){return Err(EvidenceError::Missing);}
        self.connection.execute(
            "INSERT OR IGNORE INTO research_campaign_source_review
             (campaign_id,source_id,stage,reason_code)
             VALUES (?1,?2,?3,?4)",
            params![id,source_id,stage,reason]
        )?;
        let stored:(String,String)=self.connection.query_row(
            "SELECT stage,reason_code FROM research_campaign_source_review
             WHERE campaign_id=?1 AND source_id=?2",
            params![id,source_id],|r|Ok((r.get(0)?,r.get(1)?))
        )?;
        if stored!=(stage.to_owned(),reason.to_owned()){
            return Err(EvidenceError::Invalid);
        }
        Ok(())
    }
    fn campaign_safe_id(id:&str)->bool{
        id.starts_with("research-campaign-")&&id.len()<=128
            &&id.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'-'||c==b'_')
    }
    /// User-created, capped, chat-scoped search program. No network grant.
    pub fn create_campaign(
        &mut self,id:&str,query:&str,ceiling:u32
    )->Result<ResearchCampaignStatus,EvidenceError>{
        if !Self::campaign_safe_id(id)||!bounded(query,512)
            ||!(1..=120).contains(&ceiling){return Err(EvidenceError::Invalid);}
        self.connection.execute(
            "INSERT OR IGNORE INTO research_campaigns
             (id,query,candidate_ceiling,next_page,state)
             VALUES (?1,?2,?3,1,'READY')",
            params![id,query,ceiling]
        )?;
        let status=self.campaign_status(id)?;
        if status.query!=query||status.candidate_ceiling!=ceiling{
            return Err(EvidenceError::Invalid);
        }
        Ok(status)
    }
    pub fn campaign_status(&self,id:&str)
        ->Result<ResearchCampaignStatus,EvidenceError>{
        if !Self::campaign_safe_id(id){return Err(EvidenceError::Invalid);}
        self.connection.query_row(
            "SELECT id,query,state,next_page,candidate_ceiling,
             (SELECT COUNT(*) FROM research_campaign_leads WHERE campaign_id=?1),
             (SELECT COUNT(*) FROM research_campaign_pages WHERE campaign_id=?1),
             blocked_reason FROM research_campaigns WHERE id=?1",
            [id],|r|Ok(ResearchCampaignStatus{
                id:r.get(0)?,query:r.get(1)?,state:r.get(2)?,
                next_page:r.get::<_,i64>(3)? as u32,
                candidate_ceiling:r.get::<_,i64>(4)? as u32,
                distinct_dois:r.get::<_,i64>(5)? as u32,
                completed_pages:r.get::<_,i64>(6)? as u32,
                blocked_reason:r.get(7)?,
            })
        ).optional()?.ok_or(EvidenceError::Missing)
    }
    /// A crash in RUNNING does not advance pagination; an explicit host
    /// resume may safely re-run that same read-only page.
    pub fn claim_campaign_page(
        &mut self,id:&str
    )->Result<Option<(String,u32,u32)>,EvidenceError>{
        if !Self::campaign_safe_id(id){return Err(EvidenceError::Invalid);}
        let before=self.campaign_status(id)?;
        if before.state=="DONE"||before.state=="CANCELLED"{return Ok(None);}
        if before.state=="BLOCKED"{return Err(EvidenceError::Invalid);}
        if before.next_page>5{return Ok(None);}
        let changed=self.connection.execute(
            "UPDATE research_campaigns SET state='RUNNING'
             WHERE id=?1 AND state IN ('READY','PAUSED','RUNNING')",[id]
        )?;
        if changed!=1{return Err(EvidenceError::Invalid);}
        Ok(Some((before.query,before.next_page,
            before.candidate_ceiling.saturating_sub(before.distinct_dois))))
    }
    pub fn finish_campaign_page(
        &mut self,id:&str,page:u32,response_sha256:&str,
        discovered:&[Candidate],provider_has_next:bool
    )->Result<ResearchCampaignStatus,EvidenceError>{
        if !Self::campaign_safe_id(id)||!(1..=5).contains(&page)
            ||discovered.len()>25||response_sha256.len()!=64
            ||!response_sha256.bytes().all(|c|c.is_ascii_hexdigit()){
            return Err(EvidenceError::Invalid);
        }
        let state=self.campaign_status(id)?;
        if state.state!="RUNNING"||state.next_page!=page {
            return Err(EvidenceError::Invalid);
        }
        for lead in discovered{
            if !bounded(&lead.id,128)||!bounded(&lead.title,1024)
                ||!bounded(&lead.author,512)
                ||crate::web_research_planner::canonical_doi(&lead.identifier).is_none(){
                return Err(EvidenceError::Invalid);
            }
        }
        let tx=self.connection.transaction()?;
        tx.execute(
            "INSERT INTO research_campaign_pages
             (campaign_id,page,response_sha256) VALUES (?1,?2,?3)",
            params![id,page,response_sha256]
        )?;
        let mut count=state.distinct_dois;
        for lead in discovered{
            if count>=state.candidate_ceiling{break;}
            let doi=crate::web_research_planner::canonical_doi(&lead.identifier)
                .ok_or(EvidenceError::Invalid)?;
            tx.execute(
                "INSERT OR IGNORE INTO research_sources
                 (id,title,author,identifier,status)
                 VALUES (?1,?2,?3,?4,'DISCOVERED')",
                params![&lead.id,&lead.title,&lead.author,&lead.identifier]
            )?;
            count+=tx.execute(
                "INSERT OR IGNORE INTO research_campaign_leads
                 (campaign_id,doi,source_id,first_page)
                 VALUES (?1,?2,?3,?4)",
                params![id,doi,&lead.id,page]
            )? as u32;
        }
        let done=count>=state.candidate_ceiling||!provider_has_next||page>=5;
        tx.execute(
            "UPDATE research_campaigns SET next_page=?2,state=?3,blocked_reason=NULL
             WHERE id=?1 AND state='RUNNING'",
            params![id,page+1,if done{"DONE"}else{"PAUSED"}]
        )?;
        tx.commit()?;
        self.campaign_status(id)
    }
    pub fn block_campaign(&mut self,id:&str,reason:&str)
        ->Result<(),EvidenceError>{
        if !Self::campaign_safe_id(id)||!matches!(reason,
            "FETCH_FAILED"|"RATE_LIMIT"|"POLICY_DENIED"|"DOCUMENT_UNAVAILABLE"){
            return Err(EvidenceError::Invalid);
        }
        let changed=self.connection.execute(
            "UPDATE research_campaigns SET state='BLOCKED',blocked_reason=?2
             WHERE id=?1 AND state='RUNNING'",params![id,reason]
        )?;
        if changed!=1{return Err(EvidenceError::Invalid);}
        Ok(())
    }
    pub fn resume_campaign(&mut self,id:&str)->Result<(),EvidenceError>{
        if !Self::campaign_safe_id(id){return Err(EvidenceError::Invalid);}
        let changed=self.connection.execute(
            "UPDATE research_campaigns SET state='PAUSED',blocked_reason=NULL
             WHERE id=?1 AND state='BLOCKED'",[id]
        )?;
        if changed!=1{return Err(EvidenceError::Invalid);}
        Ok(())
    }
    pub fn cancel_campaign(&mut self,id:&str)->Result<(),EvidenceError>{
        if !Self::campaign_safe_id(id){return Err(EvidenceError::Invalid);}
        let changed=self.connection.execute(
            "UPDATE research_campaigns SET state='CANCELLED'
             WHERE id=?1 AND state NOT IN ('DONE','CANCELLED')",[id]
        )?;
        if changed!=1{return Err(EvidenceError::Invalid);}
        Ok(())
    }

    /// Only the independently fetched Crossref landing URL is a host-owned
    /// retrieval seed. The model cannot supply a replacement source URL.
    pub fn publisher_url_for_source(
        &self,id:&str
    )->Result<Option<String>,EvidenceError>{
        if !bounded(id,128){return Err(EvidenceError::Invalid);}
        if !matches!(self.status(id)?,SourceStatus::MetadataCorroborated
            |SourceStatus::DocumentAccessed|SourceStatus::PassageVerified){
            return Err(EvidenceError::NotCorroborated);
        }
        Ok(self.connection.query_row(
            "SELECT publisher_url FROM research_publisher_links WHERE source_id=?1",
            [id],|r|r.get(0)
        ).optional()?)
    }
    /// Record a bounded HTTPS document response and its observed redirect
    /// chain. PDF text is promoted only if the *actual final URL* is the
    /// independently corroborated catalog URL, extracted without truncation.
    /// HTML pages can be previewed but cannot imply complete paper access.
    pub fn record_document_retrieval(
        &mut self,id:&str,catalog_url:&str,final_url:&str,
        raw_sha256:&str,chain_sha256:&str,format:&str,text:&str,truncated:bool
    )->Result<DocumentRetrievalProof,EvidenceError>{
        if !bounded(id,128)||!matches!(format,"PDF"|"HTML")
            ||text.trim().is_empty()||text.len()>4_000_000
            ||text.chars().any(|c|c=='\0'||(c.is_control()&&c!='\n'&&c!='\r'&&c!='\t'))
            ||raw_sha256.len()!=64||chain_sha256.len()!=64
            ||!raw_sha256.bytes().all(|b|b.is_ascii_hexdigit())
            ||!chain_sha256.bytes().all(|b|b.is_ascii_hexdigit())
            ||!catalog_url.starts_with("https://")||!final_url.starts_with("https://")
            ||catalog_url.len()>2048||final_url.len()>2048
            ||catalog_url.chars().any(char::is_whitespace)
            ||final_url.chars().any(char::is_whitespace) {
            return Err(EvidenceError::Invalid);
        }
        if self.status(id)?!=SourceStatus::MetadataCorroborated {
            return Err(EvidenceError::NotCorroborated);
        }
        let pinned=self.publisher_url_for_source(id)?
            .ok_or(EvidenceError::NotCorroborated)?;
        if catalog_url!=pinned{return Err(EvidenceError::MetadataMismatch);}
        let promotion=format=="PDF"&&!truncated&&final_url==pinned;
        let extracted_sha=digest(text.as_bytes());
        let receipt=DocumentRetrievalProof{
            source_id:id.into(),catalog_url:catalog_url.into(),final_url:final_url.into(),
            raw_sha256:raw_sha256.into(),text_sha256:extracted_sha.clone(),
            chain_sha256:chain_sha256.into(),format:format.into(),
            truncated,document_accessed:promotion
        };
        let tx=self.connection.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO research_document_observations
             (source_id,chain_sha256,catalog_url,final_url,raw_sha256,
              extracted_sha256,format,truncated,document_accessed)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![id,chain_sha256,catalog_url,final_url,raw_sha256,
                &extracted_sha,format,i64::from(truncated),i64::from(promotion)]
        )?;
        let stored:(String,String,String,String,String,i64,i64)=tx.query_row(
            "SELECT catalog_url,final_url,raw_sha256,extracted_sha256,format,
             truncated,document_accessed FROM research_document_observations
             WHERE source_id=?1 AND chain_sha256=?2",
            params![id,chain_sha256],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
                r.get(4)?,r.get(5)?,r.get(6)?))
        )?;
        if stored!=(catalog_url.into(),final_url.into(),raw_sha256.into(),
            extracted_sha,format.into(),i64::from(truncated),i64::from(promotion)){
            return Err(EvidenceError::Invalid);
        }
        if promotion {
            let changed=tx.execute(
                "UPDATE research_sources SET status='DOCUMENT_ACCESSED',
                 document_text=?2, content_sha256=?3
                 WHERE id=?1 AND status='METADATA_CORROBORATED'",
                params![id,text,&receipt.text_sha256]
            )?;
            if changed!=1{return Err(EvidenceError::NotCorroborated);}
        }
        tx.commit()?;
        Ok(receipt)
    }
    /// An exact substring proof against *retained* Browser text. Caller
    /// cannot supply a replacement URL, page body, model paraphrase or hash.
    /// Catalog matching is a separate tier and requires the independently
    /// fetched publisher link to equal the Browser's actual final URL.
    pub fn prove_browser_passage(
        &mut self,generation_id:&str,call_id:&str,quotation:&str,
        publisher_source_id:Option<&str>
    )->Result<BrowserPassageProof,EvidenceError>{
        if !bounded(quotation,2048)||quotation.len()<4
            ||quotation.contains('\0') {
            return Err(EvidenceError::Invalid);
        }
        let source=self.browser_observation(generation_id,call_id)?;
        if !source.excerpt.contains(quotation){return Err(EvidenceError::PassageNotFound);}
        let linked=if let Some(id)=publisher_source_id {
            if !bounded(id,128)
                ||!matches!(self.status(id)?,SourceStatus::MetadataCorroborated
                    |SourceStatus::DocumentAccessed|SourceStatus::PassageVerified){
                return Err(EvidenceError::NotCorroborated);
            }
            let url:Option<String>=self.connection.query_row(
                "SELECT publisher_url FROM research_publisher_links WHERE source_id=?1",
                [id],|r|r.get(0)
            ).optional()?;
            if url.as_deref()!=Some(source.url.as_str()){
                return Err(EvidenceError::MetadataMismatch);
            }
            true
        } else {false};
        let source_id=publisher_source_id.unwrap_or("");
        self.connection.execute(
            "INSERT OR IGNORE INTO research_browser_passage_proofs
             (generation_id,call_id,source_id,quote,observed_sha256,
              publisher_link_matched) VALUES (?1,?2,?3,?4,?5,?6)",
            params![generation_id,call_id,source_id,quotation,
                source.observed_sha256,i64::from(linked)]
        )?;
        let stored:(String,i64)=self.connection.query_row(
            "SELECT observed_sha256,publisher_link_matched
             FROM research_browser_passage_proofs
             WHERE generation_id=?1 AND call_id=?2 AND source_id=?3 AND quote=?4",
            params![generation_id,call_id,source_id,quotation],
            |r|Ok((r.get(0)?,r.get(1)?))
        )?;
        if stored.0!=source.observed_sha256||stored.1!=i64::from(linked){
            return Err(EvidenceError::Invalid);
        }
        Ok(BrowserPassageProof{
            generation_id:generation_id.into(),call_id:call_id.into(),
            source_url:source.url,observed_sha256:source.observed_sha256,
            exact_quote:quotation.into(),source_id:publisher_source_id.map(str::to_owned),
            publisher_link_matched:linked,
        })
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
        if stored.is_none(){return Ok(false);}
        let original:Option<(String,String)>=self.connection.query_row(
            "SELECT document_text,content_sha256 FROM research_sources WHERE id=?1",
            [id],|row|Ok((row.get(0)?,row.get(1)?))
        ).optional()?;
        Ok(original.is_some_and(|(text,sha)|{
            sha==digest(text.as_bytes()) && text.contains(quote)
        }))
    }
    /// Final report gate: only records with a real corroborated identifier
    /// AND actual accessed text may be listed as reviewed sources.
    pub fn eligible_for_reviewed_bibliography(&self, id: &str) -> Result<bool, EvidenceError> {
        let record = self.record(id)?;
        if !matches!(record.status,SourceStatus::DocumentAccessed|SourceStatus::PassageVerified)
            ||record.record_url.is_none(){return Ok(false);}
        let body:Option<(String,String)>=self.connection.query_row(
            "SELECT document_text,content_sha256 FROM research_sources WHERE id=?1",
            [id],|row|Ok((row.get(0)?,row.get(1)?))
        ).optional()?;
        Ok(body.is_some_and(|(text,sha)|{
            !text.is_empty() && sha==digest(text.as_bytes())
                && record.content_sha256.as_deref()==Some(sha.as_str())
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn candidate() -> Candidate { Candidate {
        id:"source-1".into(),title:"A Verified Study".into(),
        author:"Jane Doe".into(),identifier:"doi:10.1000/example".into()
    } }
    pub(super) fn record() -> TrustedBibliography { TrustedBibliography {
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
    fn stored_document_tampering_revokes_passage_and_reviewed_bibliography(){
        let mut l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        l.discover(&candidate()).unwrap();
        l.corroborate("source-1",&record()).unwrap();
        l.store_accessed_text("source-1","An independently retrieved passage.").unwrap();
        l.verify_passage("source-1","full-text","retrieved passage").unwrap();
        assert!(l.has_verified_passage("source-1","retrieved passage").unwrap());
        assert!(l.eligible_for_reviewed_bibliography("source-1").unwrap());
        // Model-visible cached passage rows do not outrank the actual stored
        // document digest when a database is corrupted or tampered with.
        l.connection.execute("UPDATE research_sources SET document_text='Injected text'
            WHERE id='source-1'",[]).unwrap();
        assert!(!l.has_verified_passage("source-1","retrieved passage").unwrap());
        assert!(!l.eligible_for_reviewed_bibliography("source-1").unwrap());
        assert!(matches!(l.verify_passage("source-1","full-text","Injected text"),
            Err(EvidenceError::NoDocument)));
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
    use super::tests::{candidate,record};
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
            assert_eq!(saved.observed_text_bytes,48);
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
    fn excerpt_quote_is_not_academic_citation_and_false_quote_is_rejected(){
        let mut l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        l.observe_browser_text("g-claim","browser-call-claim","https://paper.example/article",
            "READ","The findings were mixed and uncertain.",false).unwrap();
        assert!(matches!(l.prove_browser_passage("g-claim","browser-call-claim",
            "Invented finding",None),Err(EvidenceError::PassageNotFound)));
        let proof=l.prove_browser_passage("g-claim","browser-call-claim",
            "findings were mixed",None).unwrap();
        assert!(!proof.publisher_link_matched);
        assert_eq!(proof.source_id,None);
        assert_eq!(proof.source_url,"https://paper.example/article");
    }
    #[test]
    fn research_campaign_can_checkpoint_120_distinct_dois_without_faking_verification(){
        let mut l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        let id="research-campaign-120";
        let initial=l.create_campaign(id,"cognition research",120).unwrap();
        assert_eq!(initial.state,"READY");
        for page in 1..=5u32{
            let (_,next,remaining)=l.claim_campaign_page(id).unwrap().unwrap();
            assert_eq!(next,page);
            assert_eq!(remaining,120-(page-1)*25);
            let works=(0..25u32).map(|n|Candidate{
                id:format!("https://openalex.org/W{}",page*100+n),
                title:format!("Study {page}-{n}"),author:"A Researcher".into(),
                identifier:format!("10.1234/{}-{}",page,n)
            }).collect::<Vec<_>>();
            let result=l.finish_campaign_page(id,page,&"b".repeat(64),
                &works,page<5).unwrap();
            assert_eq!(result.distinct_dois,(page*25).min(120));
            assert_eq!(result.state,if page==5{"DONE"}else{"PAUSED"});
        }
        assert_eq!(l.campaign_status(id).unwrap().completed_pages,5);
        assert_eq!(l.status("https://openalex.org/W100").unwrap(),SourceStatus::Discovered);
        assert!(l.claim_campaign_page(id).unwrap().is_none());
    }
    #[test]
    fn campaign_failed_page_recovery_does_not_invent_progress(){
        let mut l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        l.create_campaign("research-campaign-fail","my inquiry",120).unwrap();
        let page=l.claim_campaign_page("research-campaign-fail").unwrap().unwrap();
        assert_eq!(page.1,1);
        let resumed=l.claim_campaign_page("research-campaign-fail").unwrap().unwrap();
        assert_eq!(resumed.1,1);
        assert_eq!(l.campaign_status("research-campaign-fail").unwrap().completed_pages,0);
        l.block_campaign("research-campaign-fail","RATE_LIMIT").unwrap();
        assert!(l.claim_campaign_page("research-campaign-fail").is_err());
        l.resume_campaign("research-campaign-fail").unwrap();
        assert_eq!(l.claim_campaign_page("research-campaign-fail").unwrap().unwrap().1,1);
        l.cancel_campaign("research-campaign-fail").unwrap();
        assert!(l.claim_campaign_page("research-campaign-fail").unwrap().is_none());
    }
    #[test]
    fn full_text_access_requires_nontruncated_pdf_and_exact_catalog_url(){
        let mut l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        l.discover(&candidate()).unwrap();
        l.corroborate("source-1",&record()).unwrap();
        let digest64="a".repeat(64);
        l.bind_crossref_publisher_url("source-1",
            "https://publisher.example/document.pdf",&digest64).unwrap();
        assert!(l.record_document_retrieval("source-1",
            "https://wrong.example/document.pdf","https://publisher.example/document.pdf",
            &digest64,&"b".repeat(64),"PDF","page text",false).is_err());
        let html=l.record_document_retrieval("source-1",
            "https://publisher.example/document.pdf","https://publisher.example/document.pdf",
            &digest64,&"c".repeat(64),"HTML","HTML landing preview",false).unwrap();
        assert!(!html.document_accessed);
        let redirected=l.record_document_retrieval("source-1",
            "https://publisher.example/document.pdf","https://cdn.example/document.pdf",
            &digest64,&"d".repeat(64),"PDF","PDF redirected candidate",false).unwrap();
        assert!(!redirected.document_accessed);
        let clipped=l.record_document_retrieval("source-1",
            "https://publisher.example/document.pdf","https://publisher.example/document.pdf",
            &digest64,&"e".repeat(64),"PDF","partial PDF excerpt",true).unwrap();
        assert!(!clipped.document_accessed);
        assert_eq!(l.status("source-1").unwrap(),SourceStatus::MetadataCorroborated);
        let complete=l.record_document_retrieval("source-1",
            "https://publisher.example/document.pdf","https://publisher.example/document.pdf",
            &digest64,&"f".repeat(64),"PDF","A complete PDF passage.\nAnother line.",false).unwrap();
        assert!(complete.document_accessed);
        assert_eq!(l.status("source-1").unwrap(),SourceStatus::DocumentAccessed);
        assert!(l.eligible_for_reviewed_bibliography("source-1").unwrap());
        l.verify_passage("source-1","full-text","complete PDF passage").unwrap();
    }
    #[test]
    fn independently_bound_publisher_url_must_match_exact_browser_final_url(){
        let mut l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        l.discover(&Candidate{id:"paper-1".into(),title:"Study".into(),
            author:"Researcher".into(),identifier:"10.1234/study".into()}).unwrap();
        l.observe_browser_text("g-claim","browser-call-claim",
            "https://paper.example/article","READ",
            "A carefully qualified conclusion.",false).unwrap();
        assert!(l.prove_browser_passage("g-claim","browser-call-claim",
            "qualified conclusion",Some("paper-1")).is_err());
        l.corroborate("paper-1",&TrustedBibliography{
            record_url:"https://api.crossref.org/works/10.1234/study".into(),
            title:"Study".into(),author:"Researcher".into(),
            identifier:"10.1234/study".into()
        }).unwrap();
        assert!(l.prove_browser_passage("g-claim","browser-call-claim",
            "qualified conclusion",Some("paper-1")).is_err());
        let digest="f".repeat(64);
        l.bind_crossref_publisher_url("paper-1","https://paper.example/different",&digest).unwrap();
        assert!(matches!(l.prove_browser_passage("g-claim","browser-call-claim",
            "qualified conclusion",Some("paper-1")),Err(EvidenceError::MetadataMismatch)));
        assert!(l.bind_crossref_publisher_url("paper-1",
            "https://paper.example/article",&digest).is_err());
        l.observe_browser_text("g-second","browser-call-second",
            "https://paper.example/different","READ","A second real passage.",false).unwrap();
        let proof=l.prove_browser_passage("g-second","browser-call-second",
            "second real passage",Some("paper-1")).unwrap();
        assert!(proof.publisher_link_matched);
        assert_eq!(proof.source_id.as_deref(),Some("paper-1"));
        assert_eq!(l.status("paper-1").unwrap(),SourceStatus::MetadataCorroborated);
        assert!(!l.eligible_for_reviewed_bibliography("paper-1").unwrap());
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
