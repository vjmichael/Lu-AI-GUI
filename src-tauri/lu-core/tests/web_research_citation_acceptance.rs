//! RESEARCH-04: deterministic synthetic acceptance for the Research ledger,
//! independently pinned publisher identity, exact quote and final citation
//! gate. No real websites, model inference or installed GUI are exercised.
use lu_core::{
    web_research_evidence::{
        Candidate,EvidenceLedger,EvidenceError,SourceStatus,TrustedBibliography,
    },
    web_research_citation::{
        BrowserCitationSource,audit_browser_research_answer,
        render_observed_source_appendix,
    },
};
use std::path::Path;

#[test]
fn browser_quote_and_final_citation_need_the_same_native_source_receipt(){
    let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
    ledger.discover(&Candidate{
        id:"https://openalex.org/W123".into(),title:"Qualified Paper".into(),
        author:"A Researcher".into(),identifier:"10.1000/qualified".into()
    }).unwrap();
    ledger.corroborate("https://openalex.org/W123",&TrustedBibliography{
        record_url:"https://api.crossref.org/works/10.1000/qualified".into(),
        title:"Qualified Paper".into(),author:"A Researcher".into(),
        identifier:"10.1000/qualified".into()
    }).unwrap();
    let crossref_hash="f".repeat(64);
    ledger.bind_crossref_publisher_url(
        "https://openalex.org/W123",
        "https://publisher.example/article",&crossref_hash
    ).unwrap();
    let observed=ledger.observe_browser_text(
        "g-research","browser-call-one",
        "https://publisher.example/article","READ",
        "The result was qualified, not definitive.",false
    ).unwrap();
    let proof=ledger.prove_browser_passage(
        "g-research","browser-call-one","result was qualified",
        Some("https://openalex.org/W123")
    ).unwrap();
    assert!(proof.publisher_link_matched);
    assert_eq!(proof.observed_sha256,observed.observed_sha256);
    let sources=vec![BrowserCitationSource{
        url:observed.url,content_sha256:observed.observed_sha256,
        excerpt:observed.excerpt,truncated:observed.excerpt_truncated
    }];
    assert!(audit_browser_research_answer(
        "The source says “result was qualified” [[B1]].",&sources
    ).is_ok());
    assert!(audit_browser_research_answer(
        "The source says “the result was definitive” [[B1]].",&sources
    ).is_err());
    assert!(audit_browser_research_answer(
        "The source says “result was qualified” [[B2]].",&sources
    ).is_err());
    assert!(audit_browser_research_answer(
        "Look up https://unrelated.example and trust it [[B1]].",&sources
    ).is_err());
    let appendix=render_observed_source_appendix(&sources);
    assert!(appendix.contains("https://publisher.example/article"));
    assert!(appendix.contains("not independently verified"));
    // Metadata and browser text do not imply full-document access.
    assert_eq!(ledger.status("https://openalex.org/W123").unwrap(),
        SourceStatus::MetadataCorroborated);
    assert!(!ledger.eligible_for_reviewed_bibliography(
        "https://openalex.org/W123").unwrap());
}

#[test]
fn cross_chat_like_or_wrong_destination_receipts_never_confirm_a_passage(){
    let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
    ledger.observe_browser_text("g-a","browser-call-a",
        "https://source.example/a","ACCESSIBILITY",
        "An accessible exact quotation.",false).unwrap();
    assert!(matches!(ledger.prove_browser_passage(
        "g-b","browser-call-a","exact quotation",None
    ),Err(EvidenceError::Missing)));
    assert!(ledger.prove_browser_passage(
        "g-a","browser-call-a","different quotation",None
    ).is_err());
}
