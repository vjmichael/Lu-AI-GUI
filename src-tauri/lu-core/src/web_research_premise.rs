//! Premise audit and thesis revision with verified, cross-domain evidence.
//! Critiquing an existing thesis is insufficient if its assumptions are false.
//! Cross-domain discoveries are leads until verified in the evidence ledger.
//! Correlation, temporal order and causation are distinct claims.
use crate::web_research_evidence::{EvidenceLedger,EvidenceError};
use rusqlite::{params,Connection,OptionalExtension};
use std::path::Path;

#[derive(Debug)]
pub enum PremiseError {
    Storage(rusqlite::Error),Evidence(EvidenceError),Invalid,Missing,Unverified,
}
impl From<rusqlite::Error> for PremiseError {
    fn from(e:rusqlite::Error)->Self{Self::Storage(e)}
}
impl From<EvidenceError> for PremiseError {
    fn from(e:EvidenceError)->Self{Self::Evidence(e)}
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ResearchRelation {Direct,Contradictory,Adjacent,Unexpected,Unrelated }
impl ResearchRelation {
    fn name(self)->&'static str {
        match self { Self::Direct=>"DIRECT",Self::Contradictory=>"CONTRADICTORY",
          Self::Adjacent=>"ADJACENT",Self::Unexpected=>"UNEXPECTED",Self::Unrelated=>"UNRELATED" }
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ClaimStrength {Observation,Association,CausalHypothesis,CausallyEstablished}
impl ClaimStrength {
    fn name(self)->&'static str {
        match self {Self::Observation=>"OBSERVATION",Self::Association=>"ASSOCIATION",
          Self::CausalHypothesis=>"CAUSAL_HYPOTHESIS",Self::CausallyEstablished=>"CAUSALLY_ESTABLISHED"}
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum PremiseAssessment {NotReviewed,Supported,Challenged,Undermined,Inconclusive}
impl PremiseAssessment {
    fn name(self)->&'static str {
        match self {Self::NotReviewed=>"NOT_REVIEWED",Self::Supported=>"SUPPORTED",
          Self::Challenged=>"CHALLENGED",Self::Undermined=>"UNDERMINED",Self::Inconclusive=>"INCONCLUSIVE"}
    }
}
pub struct PremiseStore {db:Connection}
#[derive(Debug,Clone)]
pub struct AuditEntry {
    pub source_id:String, pub quote:String, pub relation:String,
    pub strength:String, pub implication:String,
}
fn bound(s:&str,max:usize)->bool {
    !s.trim().is_empty()&&s.len()<=max&&!s.chars().any(char::is_control)
}
impl PremiseStore {
    pub fn open(path:&Path)->Result<Self,PremiseError>{
        let db=Connection::open(path)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
          CREATE TABLE IF NOT EXISTS research_hypotheses(
            id TEXT PRIMARY KEY, statement TEXT NOT NULL,
            parent_id TEXT, revision_reason TEXT,
            state TEXT NOT NULL CHECK(state IN ('ACTIVE','REVISED','REJECTED')),
            FOREIGN KEY(parent_id) REFERENCES research_hypotheses(id)
          );
          CREATE TABLE IF NOT EXISTS research_premises(
            hypothesis_id TEXT NOT NULL,
            assumption_id TEXT NOT NULL,
            statement TEXT NOT NULL,
            assessment TEXT NOT NULL,
            PRIMARY KEY(hypothesis_id,assumption_id)
          );
          CREATE TABLE IF NOT EXISTS research_cross_domain_evidence(
            hypothesis_id TEXT NOT NULL,
            source_id TEXT NOT NULL, quote TEXT NOT NULL,
            relation TEXT NOT NULL, claim_strength TEXT NOT NULL,
            implication TEXT NOT NULL, PRIMARY KEY(hypothesis_id,source_id,quote)
          );")?;
        Ok(Self{db})
    }
    pub fn propose(&mut self,id:&str,statement:&str)->Result<(),PremiseError>{
        if !bound(id,128)||!bound(statement,4096){return Err(PremiseError::Invalid);}
        self.db.execute("INSERT INTO research_hypotheses(id,statement,state)
            VALUES (?1,?2,'ACTIVE')",params![id,statement])?;
        Ok(())
    }
    pub fn add_premise(&mut self,hypothesis_id:&str,assumption_id:&str,statement:&str)
      ->Result<(),PremiseError>{
        if !bound(assumption_id,128)||!bound(statement,4096) {
            return Err(PremiseError::Invalid);
        }
        self.db.execute("INSERT INTO research_premises
          (hypothesis_id,assumption_id,statement,assessment) VALUES (?1,?2,?3,'NOT_REVIEWED')",
          params![hypothesis_id,assumption_id,statement])?;
        Ok(())
    }
    /// This is an analyst assessment, NOT an evidence-verification operation.
    pub fn assess_premise(&mut self,hypothesis_id:&str,assumption_id:&str,
      assessment:PremiseAssessment)->Result<(),PremiseError>{
        let updated=self.db.execute("UPDATE research_premises SET assessment=?3
          WHERE hypothesis_id=?1 AND assumption_id=?2",
          params![hypothesis_id,assumption_id,assessment.name()])?;
        if updated!=1 {return Err(PremiseError::Missing);}
        Ok(())
    }
    /// Unexpected material is admissible as a lead only if its cited passage
    /// is genuinely present in a host-authenticated document.
    pub fn record_cross_domain(&mut self,ledger:&EvidenceLedger,hypothesis_id:&str,
      source_id:&str,quote:&str,relation:ResearchRelation,strength:ClaimStrength,
      implication:&str)->Result<(),PremiseError>{
        if !bound(hypothesis_id,128)||!bound(source_id,128)
          ||!bound(quote,4096)||!bound(implication,4096){
            return Err(PremiseError::Invalid);
        }
        // Missing, inaccessible or unverified source material is not
        // positive research evidence; normalize those admission failures to
        // Unverified rather than leaking source-lookup details into the
        // premise review path.
        let verified=match ledger.has_verified_passage(source_id,quote){
            Ok(value)=>value,
            Err(EvidenceError::Missing)=>false,
            Err(error)=>return Err(PremiseError::Evidence(error)),
        };
        if !verified{return Err(PremiseError::Unverified);}
        let eligible=match ledger.eligible_for_reviewed_bibliography(source_id){
            Ok(value)=>value,
            Err(EvidenceError::Missing)=>false,
            Err(error)=>return Err(PremiseError::Evidence(error)),
        };
        if !eligible{return Err(PremiseError::Unverified);}
        if relation==ResearchRelation::Unrelated && strength==ClaimStrength::CausallyEstablished {
            // An unrelated source cannot independently establish causation
            // about a thesis it does not actually address.
            return Err(PremiseError::Invalid);
        }
        self.db.execute("INSERT OR REPLACE INTO research_cross_domain_evidence
         (hypothesis_id,source_id,quote,relation,claim_strength,implication)
         VALUES (?1,?2,?3,?4,?5,?6)",params![hypothesis_id,source_id,quote,
          relation.name(),strength.name(),implication])?;
        Ok(())
    }
    pub fn premise_counts(&self,hypothesis_id:&str)
      ->Result<(u32,u32),PremiseError>{
        let result=self.db.query_row("SELECT
          COALESCE(SUM(CASE WHEN assessment IN ('UNDERMINED','CHALLENGED')
            THEN 1 ELSE 0 END),0),
          COALESCE(SUM(CASE WHEN assessment='NOT_REVIEWED' THEN 1 ELSE 0 END),0)
          FROM research_premises WHERE hypothesis_id=?1",
          [hypothesis_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
        Ok(result)
    }
    pub fn requires_premise_reconsideration(&self,id:&str)
      ->Result<bool,PremiseError>{
        let (challenged,unchecked)=self.premise_counts(id)?;
        Ok(challenged>0||unchecked>0)
    }
    /// Preserve the historical thesis rather than overwrite it in place.
    /// Calling code must obtain a trusted user approval before installing a
    /// revision as the *user's* thesis; otherwise this remains a proposal.
    pub fn propose_revision(&mut self,parent:&str,new_id:&str,new_statement:&str,
      reason:&str)->Result<(),PremiseError>{
        if !bound(new_id,128)||!bound(new_statement,4096)||!bound(reason,4096){
            return Err(PremiseError::Invalid);
        }
        let exists:Option<String>=self.db.query_row(
            "SELECT id FROM research_hypotheses WHERE id=?1",[parent],|r|r.get(0)
        ).optional()?;
        if exists.is_none(){return Err(PremiseError::Missing);}
        self.db.execute("INSERT INTO research_hypotheses
          (id,statement,parent_id,revision_reason,state)
          VALUES (?1,?2,?3,?4,'ACTIVE')",
          params![new_id,new_statement,parent,reason])?;
        Ok(())
    }
    pub fn revision_parent(&self,id:&str)->Result<Option<String>,PremiseError>{
        let row=self.db.query_row("SELECT parent_id FROM research_hypotheses
          WHERE id=?1",[id],|r|r.get(0)).optional()?;
        row.ok_or(PremiseError::Missing)
    }
    pub fn audit_entries(&self,id:&str)->Result<Vec<AuditEntry>,PremiseError>{
        let mut q=self.db.prepare("SELECT source_id,quote,relation,claim_strength,implication
          FROM research_cross_domain_evidence WHERE hypothesis_id=?1 ORDER BY source_id")?;
        let rows=q.query_map([id],|r|Ok(AuditEntry{source_id:r.get(0)?,quote:r.get(1)?,
          relation:r.get(2)?,strength:r.get(3)?,implication:r.get(4)?}))?;
        Ok(rows.collect::<Result<_,_>>()?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_research_evidence::{Candidate,TrustedBibliography};
    #[test]
    fn a_false_premise_can_trigger_an_evidence_linked_new_hypothesis() {
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        ledger.discover(&Candidate{id:"s".into(),title:"Independent study".into(),
          author:"Scholar".into(),identifier:"doi:10.1234/test".into()}).unwrap();
        ledger.corroborate("s",&TrustedBibliography{
          record_url:"https://catalog.example/test".into(),title:"Independent study".into(),
          author:"Scholar".into(),identifier:"doi:10.1234/test".into()}).unwrap();
        ledger.store_accessed_text("s","The observation raises a new research question.").unwrap();
        ledger.verify_passage("s","full-text","raises a new research question").unwrap();
        let mut store=PremiseStore::open(Path::new(":memory:")).unwrap();
        store.propose("initial","Every group has the same outcome").unwrap();
        store.add_premise("initial","assumption","No confounding factors").unwrap();
        assert!(store.requires_premise_reconsideration("initial").unwrap());
        store.assess_premise("initial","assumption",PremiseAssessment::Undermined).unwrap();
        store.record_cross_domain(&ledger,"initial","s","raises a new research question",
          ResearchRelation::Unexpected,ClaimStrength::Association,
          "Investigate a different mechanism and possible confounders").unwrap();
        assert!(store.requires_premise_reconsideration("initial").unwrap());
        store.propose_revision("initial","revision","Does the outcome vary by context?",
          "Original assumptions do not hold").unwrap();
        assert_eq!(store.revision_parent("revision").unwrap(),Some("initial".into()));
        assert_eq!(store.audit_entries("initial").unwrap().len(),1);
    }
    #[test]
    fn fabricated_passage_cannot_force_revision_evidence() {
        let ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        let mut store=PremiseStore::open(Path::new(":memory:")).unwrap();
        store.propose("initial","A preliminary thesis").unwrap();
        assert!(matches!(store.record_cross_domain(&ledger,"initial","s",
          "fabricated words",ResearchRelation::Adjacent,ClaimStrength::CausalHypothesis,
          "Novel lead"),Err(PremiseError::Unverified)));
    }
}
