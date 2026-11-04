//! TOOLS-10D: evidence-linked thesis analysis.
//! Report claims are model interpretations, NOT automatically verified facts.
//! Every claim must cite an independently stored, exact document passage.
use crate::web_research_evidence::{EvidenceError, EvidenceLedger};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Stance { Supports, Challenges, Complicates }
impl Stance {
    fn code(self)->&'static str {
        match self {Self::Supports=>"SUPPORTS",Self::Challenges=>"CHALLENGES",Self::Complicates=>"COMPLICATES"}
    }
}
#[derive(Debug)]
pub enum AnalysisError { Storage(rusqlite::Error), Evidence(EvidenceError), UnverifiedPassage, Invalid }
impl From<rusqlite::Error> for AnalysisError {
    fn from(error:rusqlite::Error)->Self{Self::Storage(error)}
}
impl From<EvidenceError> for AnalysisError {
    fn from(error:EvidenceError)->Self{Self::Evidence(error)}
}
pub struct AnalysisStore { db:Connection }
pub struct Interpretation<'a> {
    pub source_id:&'a str,
    pub thesis_claim:&'a str,
    pub direct_quote:&'a str,
    pub stance:Stance,
    pub reasoning:&'a str,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct EvidenceRow {
    pub source_id:String, pub thesis_claim:String, pub direct_quote:String,
    pub stance:String, pub reasoning:String, pub status:String,
}
fn clean(s:&str,max:usize)->bool {
    !s.trim().is_empty() && s.len()<=max && !s.chars().any(char::is_control)
}
impl AnalysisStore {
    pub fn open(path:&Path)->Result<Self,AnalysisError>{
        let db=Connection::open(path)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
          CREATE TABLE IF NOT EXISTS research_claim_evidence(
            source_id TEXT NOT NULL,
            thesis_claim TEXT NOT NULL,
            direct_quote TEXT NOT NULL,
            stance TEXT NOT NULL CHECK(stance IN ('SUPPORTS','CHALLENGES','COMPLICATES')),
            reasoning TEXT NOT NULL,
            assessment_status TEXT NOT NULL DEFAULT 'MODEL_INTERPRETATION'
              CHECK(assessment_status IN ('MODEL_INTERPRETATION','HUMAN_REVIEWED')),
            PRIMARY KEY(source_id,thesis_claim,direct_quote)
          );")?;
        Ok(Self{db})
    }
    pub fn add_interpretation(&mut self, ledger:&EvidenceLedger,
        interpretation:Interpretation<'_>)->Result<(),AnalysisError>{
        if !clean(interpretation.source_id,128)||!clean(interpretation.thesis_claim,4096)
            ||!clean(interpretation.direct_quote,4096)||!clean(interpretation.reasoning,4096){
            return Err(AnalysisError::Invalid);
        }
        if !ledger.has_verified_passage(interpretation.source_id,interpretation.direct_quote)? {
            return Err(AnalysisError::UnverifiedPassage);
        }
        self.db.execute("INSERT OR REPLACE INTO research_claim_evidence
            (source_id,thesis_claim,direct_quote,stance,reasoning,assessment_status)
            VALUES (?1,?2,?3,?4,?5,'MODEL_INTERPRETATION')",
            params![interpretation.source_id,interpretation.thesis_claim,
                    interpretation.direct_quote,interpretation.stance.code(),interpretation.reasoning]
        )?;
        Ok(())
    }
    pub fn rows(&self)->Result<Vec<EvidenceRow>,AnalysisError>{
        let mut stmt=self.db.prepare("SELECT source_id,thesis_claim,direct_quote,stance,
            reasoning,assessment_status FROM research_claim_evidence ORDER BY source_id,thesis_claim")?;
        let mapped=stmt.query_map([],|row|Ok(EvidenceRow {
            source_id:row.get(0)?,thesis_claim:row.get(1)?,direct_quote:row.get(2)?,
            stance:row.get(3)?,reasoning:row.get(4)?,status:row.get(5)?,
        }))?;
        Ok(mapped.collect::<Result<_,_>>()?)
    }
    /// Prevent stale evidence from silently appearing in a new report.
    /// Caller must assemble citations only from these re-checked rows.
    pub fn export_rechecked(&self,ledger:&EvidenceLedger)->Result<Vec<EvidenceRow>,AnalysisError>{
        let mut verified=Vec::new();
        for row in self.rows()? {
            if !ledger.has_verified_passage(&row.source_id,&row.direct_quote)? {
                return Err(AnalysisError::UnverifiedPassage);
            }
            if !ledger.eligible_for_reviewed_bibliography(&row.source_id)? {
                return Err(AnalysisError::UnverifiedPassage);
            }
            verified.push(row);
        }
        Ok(verified)
    }
    pub fn human_review(&mut self,source_id:&str,claim:&str,quote:&str)
        ->Result<(),AnalysisError>{
        // A caller's user identity and approval must be checked by Lu's trusted
        // host before invoking this method. Review is not model-writable.
        let changed=self.db.execute("UPDATE research_claim_evidence
            SET assessment_status='HUMAN_REVIEWED' WHERE
            source_id=?1 AND thesis_claim=?2 AND direct_quote=?3",
            params![source_id,claim,quote])?;
        if changed!=1 {return Err(AnalysisError::Invalid);}
        Ok(())
    }
    pub fn count_for(&self,stance:Stance)->Result<u64,AnalysisError>{
        Ok(self.db.query_row("SELECT count(*) FROM research_claim_evidence
            WHERE stance=?1",[stance.code()],|row|row.get(0))?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_research_evidence::{Candidate,TrustedBibliography};
    fn source(ledger:&mut EvidenceLedger){
        ledger.discover(&Candidate{id:"s1".into(),title:"Paper".into(),author:"Jane Doe".into(),
            identifier:"doi:10.1234/study".into()}).unwrap();
        ledger.corroborate("s1",&TrustedBibliography{
            record_url:"https://api.crossref.org/works/10.1234/study".into(),
            title:"Paper".into(),author:"Jane Doe".into(),identifier:"doi:10.1234/study".into()
        }).unwrap();
        ledger.store_accessed_text("s1","Original evidence is nuanced.").unwrap();
        ledger.verify_passage("s1","full-text","evidence is nuanced").unwrap();
    }
    #[test]
    fn unsourced_or_invented_claims_cannot_enter_report(){
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        source(&mut ledger);
        let mut analysis=AnalysisStore::open(Path::new(":memory:")).unwrap();
        assert!(matches!(analysis.add_interpretation(&ledger,Interpretation{
            source_id:"s1",thesis_claim:"The thesis holds",
            direct_quote:"Invented quote",stance:Stance::Supports,
            reasoning:"The source agrees"
        }),Err(AnalysisError::UnverifiedPassage)));
        analysis.add_interpretation(&ledger,Interpretation{
            source_id:"s1",thesis_claim:"The thesis needs qualification",
            direct_quote:"evidence is nuanced",stance:Stance::Complicates,
            reasoning:"Supports a narrower proposition"
        }).unwrap();
        let output=analysis.export_rechecked(&ledger).unwrap();
        assert_eq!(output.len(),1);
        assert_eq!(output[0].status,"MODEL_INTERPRETATION");
        assert_eq!(analysis.count_for(Stance::Complicates).unwrap(),1);
    }
}
