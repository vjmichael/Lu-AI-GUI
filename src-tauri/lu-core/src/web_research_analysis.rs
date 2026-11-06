//! TOOLS-10D: evidence-linked thesis analysis.
//! Report claims are model interpretations, NOT automatically verified facts.
//! Every claim must cite an independently stored, exact document passage.
use crate::web_research_evidence::{EvidenceError, EvidenceLedger};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Stance { Supports, Challenges, Complicates }
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum SourceClass { Primary, Secondary, Tertiary, Unclassified }
impl SourceClass {
    fn code(self)->&'static str {
        match self {Self::Primary=>"PRIMARY",Self::Secondary=>"SECONDARY",
            Self::Tertiary=>"TERTIARY",Self::Unclassified=>"UNCLASSIFIED"}
    }
}
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
/// Grouped claim matrix. Counts are distinct *catalog source IDs*, not
/// assumed independent research lineages. Interpretation is never a fact.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ClaimDebate {
    pub thesis_claim:String,
    pub supports:Vec<EvidenceRow>,
    pub challenges:Vec<EvidenceRow>,
    pub complicates:Vec<EvidenceRow>,
    pub distinct_catalog_sources:usize,
    pub unresolved_disagreement:bool,
    pub interpretation_verified:bool,
    pub reason_codes:Vec<&'static str>,
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
          );
          CREATE TABLE IF NOT EXISTS research_source_class(
            source_id TEXT PRIMARY KEY,
            class TEXT NOT NULL CHECK(class IN ('PRIMARY','SECONDARY','TERTIARY','UNCLASSIFIED')),
            review_status TEXT NOT NULL CHECK(review_status IN ('MODEL_CLASSIFICATION','HUMAN_REVIEWED'))
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
    /// Rechecks every stored source passage and content hash before deriving
    /// stance summaries. Counterevidence is exposed instead of outvoted by
    /// frequency or paraphrased into a fabricated majority conclusion.
    pub fn summarize_claims(
        &self,ledger:&EvidenceLedger
    )->Result<Vec<ClaimDebate>,AnalysisError>{
        use std::collections::{BTreeMap,BTreeSet};
        let mut groups=BTreeMap::<String,ClaimDebate>::new();
        for row in self.export_rechecked(ledger)? {
            let key=row.thesis_claim.clone();
            let item=groups.entry(key.clone()).or_insert_with(||ClaimDebate{
                thesis_claim:key,supports:Vec::new(),challenges:Vec::new(),
                complicates:Vec::new(),distinct_catalog_sources:0,
                unresolved_disagreement:false,interpretation_verified:false,
                reason_codes:Vec::new(),
            });
            match row.stance.as_str(){
                "SUPPORTS"=>item.supports.push(row),
                "CHALLENGES"=>item.challenges.push(row),
                "COMPLICATES"=>item.complicates.push(row),
                _=>return Err(AnalysisError::Invalid),
            }
        }
        for item in groups.values_mut(){
            let sources=item.supports.iter().chain(item.challenges.iter())
                .chain(item.complicates.iter())
                .map(|e|e.source_id.as_str()).collect::<BTreeSet<_>>();
            item.distinct_catalog_sources=sources.len();
            item.unresolved_disagreement=!item.challenges.is_empty()
                ||!item.complicates.is_empty();
            item.reason_codes.push("MODEL_INTERPRETATIONS_NOT_FACTS");
            if item.distinct_catalog_sources<2{
                item.reason_codes.push("FEWER_THAN_TWO_DISTINCT_CATALOG_SOURCES");
            }
            if item.challenges.is_empty(){
                item.reason_codes.push("NO_RECORDED_COUNTEREVIDENCE");
            }
            if item.unresolved_disagreement{
                item.reason_codes.push("OPEN_CONFLICT_OR_QUALIFICATION");
            }
            if item.supports.iter().chain(item.challenges.iter())
                .chain(item.complicates.iter())
                .any(|e|e.status!="HUMAN_REVIEWED"){
                item.reason_codes.push("HUMAN_INTERPRETATION_REVIEW_PENDING");
            }
        }
        Ok(groups.into_values().collect())
    }
    pub fn classify_source(&mut self,ledger:&EvidenceLedger,id:&str,
        category:SourceClass)->Result<(),AnalysisError>{
        if !clean(id,128) || !ledger.eligible_for_reviewed_bibliography(id)? {
            return Err(AnalysisError::UnverifiedPassage);
        }
        self.db.execute("INSERT OR REPLACE INTO research_source_class
            (source_id,class,review_status) VALUES (?1,?2,'MODEL_CLASSIFICATION')",
            params![id,category.code()])?;
        Ok(())
    }
    pub fn source_class(&self,id:&str)->Result<(String,String),AnalysisError>{
        Ok(self.db.query_row("SELECT class,review_status FROM research_source_class
           WHERE source_id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?)))
           .optional()?.unwrap_or(("UNCLASSIFIED".into(),"NOT_REVIEWED".into())))
    }
    /// An auditable JSON evidence matrix, not a prose answer invented by LLM.
    /// A verified publication can still have a model-interpreted meaning.
    pub fn export_verified_json(&self,ledger:&EvidenceLedger)
        ->Result<serde_json::Value,AnalysisError>{
        let rows=self.export_rechecked(ledger)?;
        let mut entries=Vec::new();
        for row in rows {
            let source=ledger.record(&row.source_id)?;
            let Some(url)=source.record_url else {
                return Err(AnalysisError::UnverifiedPassage);
            };
            let Some(sha)=source.content_sha256 else {
                return Err(AnalysisError::UnverifiedPassage);
            };
            let (source_class,class_review)=self.source_class(&row.source_id)?;
            entries.push(serde_json::json!({
                "source_class_model_assessment":source_class,
                "source_class_review_status":class_review,
                "source_id":row.source_id,
                "title":source.title,
                "catalog_record_url":url,
                "document_sha256":sha,
                "thesis_claim":row.thesis_claim,
                "verified_direct_quote":row.direct_quote,
                "stance_interpretation":row.stance,
                "reasoning_interpretation":row.reasoning,
                "interpretation_review_status":row.status,
                "locator":"full-text",
                "page_number_verified":false
            }));
        }
        let debates=self.summarize_claims(ledger)?
            .into_iter().map(|c|serde_json::json!({
                "thesis_claim":c.thesis_claim,
                "supports":c.supports.len(),
                "challenges":c.challenges.len(),
                "complicates":c.complicates.len(),
                "distinct_catalog_sources":c.distinct_catalog_sources,
                "unresolved_disagreement":c.unresolved_disagreement,
                "interpretation_verified":false,
                "reason_codes":c.reason_codes,
            })).collect::<Vec<_>>();
        Ok(serde_json::json!({
            "schema":"lu.research.evidence.v1",
            "source_count_in_matrix":entries.len(),
            "interpretations_are_not_independently_verified":true,
            "claims":debates,
            "entries":entries,
        }))
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
    fn conflicting_verified_quotes_remain_visible_without_majority_truth_claim(){
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        source(&mut ledger);
        let mut analysis=AnalysisStore::open(Path::new(":memory:")).unwrap();
        analysis.add_interpretation(&ledger,Interpretation{
            source_id:"s1",thesis_claim:"The finding generalizes",
            direct_quote:"evidence is nuanced",stance:Stance::Supports,
            reasoning:"The paper cautiously supports transfer."
        }).unwrap();
        analysis.add_interpretation(&ledger,Interpretation{
            source_id:"s1",thesis_claim:"The finding generalizes",
            direct_quote:"evidence is nuanced",stance:Stance::Challenges,
            reasoning:"The same excerpt may qualify the scope."
        }).unwrap();
        let rows=analysis.summarize_claims(&ledger).unwrap();
        assert_eq!(rows.len(),1);
        assert_eq!(rows[0].distinct_catalog_sources,1);
        assert!(rows[0].unresolved_disagreement);
        assert!(!rows[0].interpretation_verified);
        assert!(rows[0].reason_codes.contains(
            &"FEWER_THAN_TWO_DISTINCT_CATALOG_SOURCES"));
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
        analysis.classify_source(&ledger,"s1",SourceClass::Primary).unwrap();
        let report=analysis.export_verified_json(&ledger).unwrap();
        assert_eq!(report["entries"][0]["source_class_model_assessment"],"PRIMARY");
        assert_eq!(report["entries"][0]["source_class_review_status"],"MODEL_CLASSIFICATION");
        assert_eq!(report["entries"][0]["page_number_verified"],false);
        assert_eq!(report["entries"][0]["interpretation_review_status"],"MODEL_INTERPRETATION");
        assert_eq!(analysis.count_for(Stance::Complicates).unwrap(),1);
    }
}
