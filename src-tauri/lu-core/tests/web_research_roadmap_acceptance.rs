//! RESEARCH-05..10 synthetic native acceptance. No network or GUI is
//! simulated as a shipped Windows application; those gates are separate.
use lu_core::{
    web_research_document::{extract_research_document,ResearchDocumentKind},
    web_research_evidence::{Candidate,EvidenceLedger,SourceStatus,TrustedBibliography},
    web_research_analysis::{AnalysisStore,Interpretation,Stance},
    web_research_report::render_verified_report,
};
use std::path::Path;

fn simple_pdf()->Vec<u8>{
    let stream=b"BT /F1 12 Tf (The source supports only a limited claim.) Tj ET";
    let mut pdf=b"%PDF-1.4\n\
        1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
        2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
        3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>\nendobj\n".to_vec();
    pdf.extend_from_slice(format!(
        "4 0 obj\n<< /Length {} >>\nstream\n",stream.len()
    ).as_bytes());
    pdf.extend_from_slice(stream);
    pdf.extend_from_slice(b"\nendstream\nendobj\n\
        xref\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n");
    pdf
}
#[test]
fn only_independently_pinned_complete_pdf_reaches_verified_report(){
    let bytes=simple_pdf();
    let parsed=extract_research_document(&bytes).unwrap();
    assert_eq!(parsed.kind,ResearchDocumentKind::Pdf);
    assert!(parsed.document_candidate);
    assert!(!parsed.truncated);
    assert!(parsed.text.contains("supports only a limited claim"));
    let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
    let id="https://openalex.org/W9876";
    ledger.discover(&Candidate{
        id:id.into(),title:"Finite Evidence".into(),author:"Researcher".into(),
        identifier:"10.1000/evidence".into()
    }).unwrap();
    ledger.corroborate(id,&TrustedBibliography{
        record_url:"https://api.crossref.org/works/10.1000/evidence".into(),
        title:"Finite Evidence".into(),author:"Researcher".into(),
        identifier:"10.1000/evidence".into()
    }).unwrap();
    ledger.bind_crossref_publisher_url(id,"https://publisher.example/article.pdf",
        &"f".repeat(64)).unwrap();
    let html=extract_research_document(
        b"<html><body><article>Preview text only.</article></body></html>"
    ).unwrap();
    let preview=ledger.record_document_retrieval(
        id,"https://publisher.example/article.pdf",
        "https://publisher.example/article.pdf",
        &html.raw_sha256,&"e".repeat(64),html.kind.code(),
        &html.text,html.truncated
    ).unwrap();
    assert!(!preview.document_accessed);
    assert_eq!(ledger.status(id).unwrap(),SourceStatus::MetadataCorroborated);
    let redirected=ledger.record_document_retrieval(
        id,"https://publisher.example/article.pdf",
        "https://cdn.example/article.pdf",
        &parsed.raw_sha256,&"d".repeat(64),parsed.kind.code(),
        &parsed.text,parsed.truncated
    ).unwrap();
    assert!(!redirected.document_accessed);
    let direct=ledger.record_document_retrieval(
        id,"https://publisher.example/article.pdf",
        "https://publisher.example/article.pdf",
        &parsed.raw_sha256,&"c".repeat(64),parsed.kind.code(),
        &parsed.text,parsed.truncated
    ).unwrap();
    assert!(direct.document_accessed);
    ledger.verify_passage(id,"full-text","supports only a limited claim").unwrap();
    let mut analysis=AnalysisStore::open(Path::new(":memory:")).unwrap();
    analysis.add_interpretation(&ledger,Interpretation{
        source_id:id,thesis_claim:"The result has broad generality",
        direct_quote:"supports only a limited claim",
        stance:Stance::Complicates,
        reasoning:"The exact quoted wording narrows the scope."
    }).unwrap();
    let report=render_verified_report("Test report",&ledger,&analysis).unwrap();
    assert_eq!(report.distinct_sources,1);
    assert_eq!(report.checked_passages,1);
    assert!(report.markdown.contains("Unresolved"));
    assert!(report.markdown.contains("https://api.crossref.org/works/10.1000/evidence"));
    assert!(report.json.contains("interpretations_independently_verified"));
}
#[test]
fn source_quotations_dont_appear_without_document_attestation(){
    let doc=extract_research_document(
        b"<html><body><h1>Observed but unverified</h1></body></html>"
    ).unwrap();
    assert!(!doc.document_candidate);
    assert!(!doc.truncated);
    let ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
    let analysis=AnalysisStore::open(Path::new(":memory:")).unwrap();
    let report=render_verified_report("Insufficient sources",&ledger,&analysis).unwrap();
    assert_eq!(report.checked_passages,0);
    assert!(report.markdown.contains("No eligible document-linked interpretations"));
}
