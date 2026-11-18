//! TOOLS-10C: durable search/query frontier and candidate deduplication.
//! Discovery here is NEVER equivalent to bibliographic corroboration.
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

#[derive(Debug)]
pub enum PlanningError {
    Storage(rusqlite::Error),
    Invalid,
    NotFound,
}
impl From<rusqlite::Error> for PlanningError {
    fn from(value: rusqlite::Error) -> Self { Self::Storage(value) }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryTask {
    pub id: i64,
    pub provider: String,
    pub query: String,
    pub page_token: String,
}
pub struct ResearchPlanner { db: Connection }
fn safe(value: &str, cap: usize) -> bool {
    !value.trim().is_empty() && value.len() <= cap && !value.chars().any(char::is_control)
}
pub fn canonical_doi(text: &str) -> Option<String> {
    let lower=text.trim().to_ascii_lowercase();
    let doi=lower.strip_prefix("https://doi.org/")
        .or_else(||lower.strip_prefix("http://dx.doi.org/"))
        .or_else(||lower.strip_prefix("doi:")).unwrap_or(&lower);
    if doi.len()>256 || !doi.starts_with("10.") || doi.contains(char::is_whitespace) {
        return None;
    }
    let (registrant, suffix)=doi.split_once('/')?;
    if registrant.len()<5 || !registrant[3..].bytes().all(|x|x.is_ascii_digit())
       || suffix.is_empty() || suffix.bytes().any(|x| x.is_ascii_control()) {
        return None;
    }
    Some(doi.to_owned())
}
impl ResearchPlanner {
    pub fn open(path:&Path) -> Result<Self,PlanningError> {
        let db=Connection::open(path)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
          CREATE TABLE IF NOT EXISTS web_research_queries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            provider TEXT NOT NULL,
            query TEXT NOT NULL,
            page_token TEXT NOT NULL,
            state TEXT NOT NULL CHECK(state IN ('QUEUED','IN_PROGRESS','BLOCKED','DONE','SKIPPED_TOO_DIFFICULT')),
            blocked_reason TEXT,
            UNIQUE(provider,query,page_token)
          );
          CREATE TABLE IF NOT EXISTS web_research_leads (
            canonical_key TEXT PRIMARY KEY,
            reference_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            title TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'DISCOVERED'
              CHECK(status='DISCOVERED')
          );")?;
        Ok(Self{db})
    }
    pub fn enqueue(&mut self,provider:&str,query:&str,page_token:&str)
        -> Result<(),PlanningError> {
        if !safe(provider,64) || !provider.bytes().all(|b|b.is_ascii_lowercase()||b==b'-'||b==b'_')
            || !safe(query,1024) || page_token.len()>512
            || page_token.chars().any(char::is_control) { return Err(PlanningError::Invalid); }
        self.db.execute("INSERT OR IGNORE INTO web_research_queries
            (provider,query,page_token,state) VALUES (?1,?2,?3,'QUEUED')",
            params![provider,query,page_token])?;
        Ok(())
    }
    pub fn claim(&mut self) -> Result<Option<QueryTask>,PlanningError> {
        let tx=self.db.transaction()?;
        let item=tx.query_row("SELECT id,provider,query,page_token
            FROM web_research_queries WHERE state='QUEUED' ORDER BY id LIMIT 1",
            [], |row|Ok(QueryTask{id:row.get(0)?,provider:row.get(1)?,
                                  query:row.get(2)?,page_token:row.get(3)?})
        ).optional()?;
        if let Some(ref task)=item {
            tx.execute("UPDATE web_research_queries SET state='IN_PROGRESS'
               WHERE id=?1 AND state='QUEUED'",[task.id])?;
        }
        tx.commit()?;
        Ok(item)
    }
    pub fn finish(&mut self,id:i64,next_page_token:Option<&str>) -> Result<(),PlanningError> {
        let task=self.db.query_row("SELECT provider,query,page_token FROM web_research_queries
            WHERE id=?1 AND state='IN_PROGRESS'",[id],|row|
            Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?))
        ).optional()?.ok_or(PlanningError::NotFound)?;
        if let Some(page)=next_page_token {
            if page==task.2 || page.len()>512 || page.chars().any(char::is_control) {
                return Err(PlanningError::Invalid);
            }
        }
        let tx=self.db.transaction()?;
        tx.execute("UPDATE web_research_queries SET state='DONE' WHERE id=?1",[id])?;
        if let Some(page)=next_page_token {
            tx.execute("INSERT OR IGNORE INTO web_research_queries
                (provider,query,page_token,state) VALUES (?1,?2,?3,'QUEUED')",
                params![task.0,task.1,page])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn block(&mut self,id:i64,reason:&str) -> Result<(),PlanningError> {
        if !matches!(reason,"CAPTCHA_REQUIRED"|"LOGIN_REQUIRED"|"HTTP_RATE_LIMIT"|"POLICY_DENIED") {
            return Err(PlanningError::Invalid);
        }
        let affected=self.db.execute("UPDATE web_research_queries
            SET state='BLOCKED',blocked_reason=?2 WHERE id=?1 AND state='IN_PROGRESS'",
            params![id,reason])?;
        if affected==0 {return Err(PlanningError::NotFound);}
        Ok(())
    }
    /// Gracefully abandon a difficult web source after bounded safe attempts.
    /// The reason remains durable and visible, and other queries can advance.
    pub fn skip_too_difficult(&mut self,id:i64,reason:&str)->Result<(),PlanningError>{
        if !matches!(reason,"CAPTCHA_UNRESOLVED"|"PAYWALL"|"UNSUPPORTED_WEBSITE"
            |"RENDERING_FAILED"|"REPEATED_TIMEOUT"|"NAVIGATION_TOO_COMPLEX"
            |"ACCESS_DENIED") {return Err(PlanningError::Invalid);}
        let updated=self.db.execute("UPDATE web_research_queries
            SET state='SKIPPED_TOO_DIFFICULT',blocked_reason=?2 WHERE id=?1
            AND state IN ('IN_PROGRESS','BLOCKED')",params![id,reason])?;
        if updated==0 {return Err(PlanningError::NotFound);}
        Ok(())
    }
    pub fn skipped_reasons(&self)->Result<Vec<(i64,String)>,PlanningError>{
        let mut statement=self.db.prepare("SELECT id,blocked_reason FROM web_research_queries
            WHERE state='SKIPPED_TOO_DIFFICULT' ORDER BY id")?;
        let rows=statement.query_map([],|row|Ok((row.get(0)?,row.get(1)?)))?;
        Ok(rows.collect::<Result<_,_>>()?)
    }
    pub fn resume(&mut self,id:i64) -> Result<(),PlanningError> {
        let affected=self.db.execute("UPDATE web_research_queries
            SET state='QUEUED',blocked_reason=NULL WHERE id=?1
            AND state='BLOCKED' AND blocked_reason IN ('CAPTCHA_REQUIRED','LOGIN_REQUIRED','HTTP_RATE_LIMIT')",
            [id])?;
        if affected==0 {return Err(PlanningError::NotFound);}
        Ok(())
    }
    pub fn recover_read_only_queries(&mut self) -> Result<usize,PlanningError> {
        Ok(self.db.execute("UPDATE web_research_queries SET state='QUEUED'
            WHERE state='IN_PROGRESS'",[])?)
    }
    /// Candidate persistence and deduplication must not promote the lead to
    /// corroborated. Only the separate trusted EvidenceLedger does that.
    pub fn discover_doi(&mut self,doi:&str,reference_id:&str,provider:&str,title:&str)
        -> Result<bool,PlanningError> {
        let key=canonical_doi(doi).ok_or(PlanningError::Invalid)?;
        if !safe(reference_id,128)||!safe(provider,64)||!safe(title,1024) {
            return Err(PlanningError::Invalid);
        }
        let changed=self.db.execute(
            "INSERT OR IGNORE INTO web_research_leads
             (canonical_key,reference_id,provider,title) VALUES (?1,?2,?3,?4)",
            params![key,reference_id,provider,title]
        )?;
        Ok(changed==1)
    }
    pub fn lead_count(&self) -> Result<u64,PlanningError> {
        Ok(self.db.query_row("SELECT count(*) FROM web_research_leads",[],|row|row.get(0))?)
    }
    pub fn pending_count(&self) -> Result<u64,PlanningError> {
        Ok(self.db.query_row("SELECT count(*) FROM web_research_queries
             WHERE state IN ('QUEUED','IN_PROGRESS','BLOCKED')",[],|row|row.get(0))?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn many_results_are_discovered_not_verified_and_doi_variants_deduplicate() {
        let mut planner=ResearchPlanner::open(Path::new(":memory:")).unwrap();
        for i in 0..180 {
            assert!(planner.discover_doi(&format!("10.1234/study{i}"),&format!("s{i}"),
              "catalog",&format!("Study {i}")).unwrap());
        }
        assert_eq!(planner.lead_count().unwrap(),180);
        assert!(!planner.discover_doi("https://doi.org/10.1234/STUDY42",
            "duplicate","other-catalog","Same work").unwrap());
        assert_eq!(planner.lead_count().unwrap(),180);
    }
    #[test]
    fn pagination_and_blocked_query_survive_restart_with_no_fake_success() {
        let mut planner=ResearchPlanner::open(Path::new(":memory:")).unwrap();
        planner.enqueue("catalog","my thesis","").unwrap();
        planner.enqueue("archive","primary evidence","").unwrap();
        let a=planner.claim().unwrap().unwrap();
        planner.block(a.id,"CAPTCHA_REQUIRED").unwrap();
        let b=planner.claim().unwrap().unwrap();
        assert_eq!(b.provider,"archive");
        planner.finish(b.id,Some("page-2")).unwrap();
        planner.resume(a.id).unwrap();
        assert_eq!(planner.pending_count().unwrap(),2);
        assert_eq!(planner.claim().unwrap().unwrap().id,a.id);
        assert_eq!(planner.recover_read_only_queries().unwrap(),1);
    }
    #[test]
    fn hard_source_is_skipped_with_reason_and_other_searches_continue() {
        let mut planner=ResearchPlanner::open(Path::new(":memory:")).unwrap();
        planner.enqueue("challenging-site","source text","").unwrap();
        planner.enqueue("open-catalog","alternative evidence","").unwrap();
        let difficult=planner.claim().unwrap().unwrap();
        planner.block(difficult.id,"CAPTCHA_REQUIRED").unwrap();
        planner.skip_too_difficult(difficult.id,"CAPTCHA_UNRESOLVED").unwrap();
        assert_eq!(planner.skipped_reasons().unwrap(),
            vec![(difficult.id,"CAPTCHA_UNRESOLVED".into())]);
        assert!(planner.resume(difficult.id).is_err());
        assert_eq!(planner.claim().unwrap().unwrap().provider,"open-catalog");
    }
    #[test]
    fn malformed_doi_is_not_catalog_evidence() {
        for value in ["foo","10.x/abc","10.1234/","https://evil.example/10.1111/a"] {
            assert!(canonical_doi(value).is_none());
        }
    }
}
