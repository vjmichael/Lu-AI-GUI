//! RESEARCH-09 deterministic report generation. Only the verified passage
//! ledger can introduce sources; model-authored URLs are never accepted.
use crate::{
    web_research_analysis::{AnalysisError,AnalysisStore},
    web_research_evidence::EvidenceLedger,
};
use sha2::{Digest,Sha256};
use std::collections::{BTreeMap,BTreeSet};
#[derive(Debug)]
pub enum ReportError {Invalid,Unverified(AnalysisError)}
impl From<AnalysisError> for ReportError{
    fn from(e:AnalysisError)->Self{Self::Unverified(e)}
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct VerifiedResearchReport {
    pub markdown:String,
    pub json:String,
    pub content_sha256:String,
    pub distinct_sources:usize,
    pub checked_passages:usize,
}
fn safe_title(s:&str)->bool{
    !s.trim().is_empty()&&s.len()<=180&&!s.chars().any(char::is_control)
}
fn md(s:&str)->String{
    let mut escaped=String::new();
    for c in s.chars(){
        if matches!(c,'\\'|'*'|'_'|'['|']'|'<'|'>'|'#'|'!'|'|'|'~')
            ||c=='\u{60}'{
            escaped.push('\\');
        }
        if c=='\n'||c=='\r'{escaped.push(' ')}else{escaped.push(c);}
    }
    escaped
}
pub fn render_verified_report(
    title:&str,ledger:&EvidenceLedger,analysis:&AnalysisStore
)->Result<VerifiedResearchReport,ReportError>{
    if !safe_title(title){return Err(ReportError::Invalid);}
    let matrix=analysis.export_verified_json(ledger)?;
    let entries=matrix.get("entries").and_then(|x|x.as_array())
        .ok_or(ReportError::Invalid)?;
    if entries.len()>4096{return Err(ReportError::Invalid);}
    let mut urls=BTreeMap::<String,(String,String,String)>::new();
    for item in entries{
        let id=item["source_id"].as_str().ok_or(ReportError::Invalid)?;
        let title=item["title"].as_str().ok_or(ReportError::Invalid)?;
        let url=item["catalog_record_url"].as_str().ok_or(ReportError::Invalid)?;
        let sha=item["document_sha256"].as_str().ok_or(ReportError::Invalid)?;
        if !url.starts_with("https://")||sha.len()!=64{return Err(ReportError::Invalid);}
        urls.insert(id.into(),(title.into(),url.into(),sha.into()));
    }
    let mut indexes=BTreeMap::<String,usize>::new();
    for (index,id) in urls.keys().enumerate(){indexes.insert(id.clone(),index+1);}
    let mut output=format!(
        "# {}\n\nResearch evidence report — native deterministic export\n\n\
         **Scope:** {} distinct catalog-identified documents; {} checked quotations.\n\n\
         **Limits:** Exact quotations and document hashes were rechecked. \
         Interpretations, source independence, factual truth and scientific \
         conclusions are NOT independently certified.\n\n",
        md(title),urls.len(),entries.len()
    );
    let groups=matrix["claims"].as_array().ok_or(ReportError::Invalid)?;
    output.push_str("## Claim and counterevidence matrix\n\n");
    if groups.is_empty(){
        output.push_str("No eligible document-linked interpretations exist. \
            This is not a researched conclusion.\n\n");
    }
    for item in groups{
        let claim=item["thesis_claim"].as_str().ok_or(ReportError::Invalid)?;
        let supports=item["supports"].as_u64().ok_or(ReportError::Invalid)?;
        let challenges=item["challenges"].as_u64().ok_or(ReportError::Invalid)?;
        let complicates=item["complicates"].as_u64().ok_or(ReportError::Invalid)?;
        let distinct=item["distinct_catalog_sources"].as_u64().ok_or(ReportError::Invalid)?;
        output.push_str(&format!(
            "### {}\n\nSupports: {} · Challenges: {} · Qualifies: {} · \
             Distinct catalog source IDs: {}\n\n",
            md(claim),supports,challenges,complicates,distinct
        ));
        if item["unresolved_disagreement"].as_bool()==Some(true){
            output.push_str("**Unresolved:** contrary or qualifying material \
                remains visible. No automatic majority verdict.\n\n");
        }
        if let Some(reasons)=item["reason_codes"].as_array(){
            let codes=reasons.iter().filter_map(|v|v.as_str())
                .map(md).collect::<Vec<_>>().join(", ");
            output.push_str(&format!("Assessment limitations: {}\n\n",codes));
        }
        for entry in entries{
            if entry["thesis_claim"].as_str()!=Some(claim){continue;}
            let id=entry["source_id"].as_str().ok_or(ReportError::Invalid)?;
            let n=indexes.get(id).ok_or(ReportError::Invalid)?;
            let stance=entry["stance_interpretation"].as_str().ok_or(ReportError::Invalid)?;
            let quote=entry["verified_direct_quote"].as_str().ok_or(ReportError::Invalid)?;
            let reasoning=entry["reasoning_interpretation"].as_str()
                .ok_or(ReportError::Invalid)?;
            let review=entry["interpretation_review_status"].as_str()
                .ok_or(ReportError::Invalid)?;
            output.push_str(&format!(
                "- **{} [R{}]** — exact document passage: “{}”. \
                 Interpretation: {}. Review: {}.\n",
                md(stance),n,md(quote),md(reasoning),md(review)
            ));
        }
        output.push('\n');
    }
    output.push_str("## Verified source provenance\n\n");
    if urls.is_empty(){output.push_str("No verified full-text sources available.\n\n");}
    for (id,(title,url,sha)) in &urls{
        let n=indexes.get(id).ok_or(ReportError::Invalid)?;
        output.push_str(&format!(
            "[R{}] {} — {}. Text SHA-256: {}. \
             Identity: Crossref-corroborated; document locally hash-checked.\n\n",
            n,md(title),md(url),sha
        ));
    }
    let digest=format!("{:x}",Sha256::digest(output.as_bytes()));
    let source_ids:BTreeSet<&String>=urls.keys().collect();
    let json=serde_json::to_string_pretty(&serde_json::json!({
        "schema":"lu.research.report.v1","title":title,
        "markdown_sha256":digest,
        "distinct_catalog_source_count":source_ids.len(),
        "checked_passage_count":entries.len(),
        "interpretations_independently_verified":false,
        "native_rechecked_evidence":matrix,
    })).map_err(|_|ReportError::Invalid)?;
    Ok(VerifiedResearchReport{
        markdown:output,json,content_sha256:digest,
        distinct_sources:source_ids.len(),checked_passages:entries.len(),
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{
        web_research_analysis::{Interpretation,Stance},
        web_research_evidence::{Candidate,TrustedBibliography},
    };
    use std::path::Path;
    #[test]
    fn report_emits_only_native_citations_and_keeps_conflicts_open(){
        let mut ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        ledger.discover(&Candidate{
            id:"src".into(),title:"A <Study>".into(),author:"Researcher".into(),
            identifier:"10.1234/study".into()
        }).unwrap();
        ledger.corroborate("src",&TrustedBibliography{
            record_url:"https://api.crossref.org/works/10.1234/study".into(),
            title:"A <Study>".into(),author:"Researcher".into(),
            identifier:"10.1234/study".into()
        }).unwrap();
        ledger.store_accessed_text("src","An actual verified passage.").unwrap();
        ledger.verify_passage("src","full-text","actual verified passage").unwrap();
        let mut analysis=AnalysisStore::open(Path::new(":memory:")).unwrap();
        analysis.add_interpretation(&ledger,Interpretation{
            source_id:"src",thesis_claim:"Interpretation is tentative",
            direct_quote:"actual verified passage",stance:Stance::Complicates,
            reasoning:"Claims need additional evidence."
        }).unwrap();
        let report=render_verified_report("Research report",&ledger,&analysis).unwrap();
        assert!(report.markdown.contains("[R1]"));
        assert!(report.markdown.contains("Unresolved"));
        assert!(report.markdown.contains("A \\<Study\\>"));
        assert_eq!(report.distinct_sources,1);
        assert_eq!(report.checked_passages,1);
    }
    #[test]
    fn an_empty_report_discloses_missing_sources_instead_of_inventing_any(){
        let ledger=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        let analysis=AnalysisStore::open(Path::new(":memory:")).unwrap();
        let output=render_verified_report("No findings",&ledger,&analysis).unwrap();
        assert_eq!(output.distinct_sources,0);
        assert!(output.markdown.contains("No eligible document-linked interpretations"));
        assert!(!output.markdown.contains("[R1]"));
    }
}
