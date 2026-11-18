//! TOOLS-10E synthetic 180-work integration acceptance.
//! These fixtures validate routing and trust gates, NOT real-world publications.
use lu_core::{
    web_catalog_crossref::parse_crossref_record,
    web_research_evidence::{Candidate, EvidenceLedger, EvidenceError, SourceStatus},
    web_research_planner::ResearchPlanner,
    web_research_analysis::{AnalysisStore, Interpretation, Stance},
};
use std::path::Path;

#[test]
fn one_hundred_eighty_distinct_sources_are_not_counted_until_checked() {
    let mut planner=ResearchPlanner::open(Path::new(":memory:")).unwrap();
    let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
    let mut analysis=AnalysisStore::open(Path::new(":memory:")).unwrap();
    for n in 0..180 {
        let id=format!("source-{n}");
        let doi=format!("10.1234/study-{n}");
        let title=format!("Primary Research Study {n}");
        assert!(planner.discover_doi(&doi,&id,"catalog",&title).unwrap());
        ledger.discover(&Candidate{
            id:id.clone(),title:title.clone(),author:"Jane Doe".into(),
            identifier:format!("doi:{doi}")
        }).unwrap();
        assert!(!ledger.eligible_for_reviewed_bibliography(&id).unwrap());
        let origin=serde_json::json!({"message":{"DOI":doi,"title":[title],
            "author":[{"given":"Jane","family":"Doe"}]}}).to_string();
        let catalog=parse_crossref_record(origin.as_bytes(),&doi).unwrap();
        ledger.corroborate(&id,&catalog).unwrap();
        assert!(!ledger.eligible_for_reviewed_bibliography(&id).unwrap());
        let passage=format!("This study reports evidence number {n}.");
        ledger.store_accessed_text(&id,&passage).unwrap();
        ledger.verify_passage(&id,"full-text",&passage).unwrap();
        assert_eq!(ledger.status(&id).unwrap(),SourceStatus::PassageVerified);
        let stance=match n % 3 {0=>Stance::Supports,1=>Stance::Challenges,_=>Stance::Complicates};
        analysis.add_interpretation(&ledger,Interpretation{
            source_id:&id,thesis_claim:"A test thesis requires evaluation",
            direct_quote:&passage,stance,reasoning:"Model interpretation requiring human evaluation"
        }).unwrap();
        assert!(ledger.eligible_for_reviewed_bibliography(&id).unwrap());
    }
    assert_eq!(planner.lead_count().unwrap(),180);
    assert_eq!(analysis.count_for(Stance::Supports).unwrap(),60);
    assert_eq!(analysis.count_for(Stance::Challenges).unwrap(),60);
    assert_eq!(analysis.count_for(Stance::Complicates).unwrap(),60);
    assert_eq!(analysis.export_rechecked(&ledger).unwrap().len(),180);
    assert!(!planner.discover_doi("https://doi.org/10.1234/STUDY-42",
       "duplicate","another-catalog","duplicate").unwrap());

    let false_id="invented-181";
    ledger.discover(&Candidate{id:false_id.into(),title:"Imaginary Paper".into(),
       author:"Jane Doe".into(),identifier:"doi:10.1234/study-0".into()}).unwrap();
    let real=serde_json::json!({"message":{"DOI":"10.1234/study-0",
       "title":["Primary Research Study 0"],"author":[{"given":"Jane","family":"Doe"}]}})
       .to_string();
    let record=parse_crossref_record(real.as_bytes(),"10.1234/study-0").unwrap();
    assert!(matches!(ledger.corroborate(false_id,&record),
        Err(EvidenceError::MetadataMismatch)));
    assert!(!ledger.eligible_for_reviewed_bibliography(false_id).unwrap());
}
#[test]
fn pending_captcha_survives_sqlite_restart_and_other_work_continues() {
    use std::{fs,process,time::{SystemTime,UNIX_EPOCH}};
    let nonce=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let file=std::env::temp_dir().join(format!("lu-web-resume-{}-{nonce}.sqlite",process::id()));
    {
        let mut ledger=EvidenceLedger::open(&file).unwrap();
        for (id,target) in [("a","https://catalog.example/a"),("b","https://catalog.example/b")] {
            ledger.discover(&Candidate{id:id.into(),title:id.into(),author:"Jane Doe".into(),
                identifier:format!("doi:10.1234/{id}")}).unwrap();
            ledger.queue_source(id,target).unwrap();
        }
        assert_eq!(ledger.claim_next_source().unwrap().unwrap().0,"a");
        ledger.request_human_assistance("a","CAPTCHA_REQUIRED").unwrap();
    }
    {
        let mut ledger=EvidenceLedger::open(&file).unwrap();
        assert_eq!(ledger.frontier_state("a").unwrap(),"NEEDS_HUMAN");
        assert_eq!(ledger.claim_next_source().unwrap().unwrap().0,"b");
        ledger.resume_after_human("a").unwrap();
        assert_eq!(ledger.claim_next_source().unwrap().unwrap().0,"a");
        assert!(!ledger.eligible_for_reviewed_bibliography("a").unwrap());
    }
    let _=fs::remove_file(&file);
    let _=fs::remove_file(file.with_extension("sqlite-shm"));
    let _=fs::remove_file(file.with_extension("sqlite-wal"));
}
