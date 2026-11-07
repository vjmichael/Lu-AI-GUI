//! RESEARCH-04: deterministic, fail-closed citation audit for Browser research.
//! Browser observations are untrusted page data, not independent scholarly
//! verification. The model may cite only host-numbered evidence labels.
//! Real URLs and text digests are appended by the native host AFTER audit.
use std::collections::BTreeSet;

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct BrowserCitationSource {
    pub url:String,
    pub content_sha256:String,
    pub excerpt:String,
    pub truncated:bool,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CitationAuditError {
    NoSources,
    InvalidSource,
    InvalidAnswer,
    MissingCitation,
    UnknownCitation,
    UnapprovedLink,
    UnsupportedQuotation,
}
fn source_valid(s:&BrowserCitationSource)->bool{
    s.url.starts_with("https://")&&s.url.len()<=2048
        &&!s.url.chars().any(|c|c.is_whitespace()||c.is_control())
        &&!s.url.contains('@')&&!s.url.contains('\\')
        &&s.content_sha256.len()==64
        &&s.content_sha256.bytes().all(|b|b.is_ascii_hexdigit())
        &&!s.excerpt.trim().is_empty()&&s.excerpt.len()<=40_000
}
fn markers(paragraph:&str,limit:usize)->Result<BTreeSet<usize>,CitationAuditError>{
    let mut used=BTreeSet::new();
    let mut remain=paragraph;
    while let Some(start)=remain.find("[["){
        remain=&remain[start+2..];
        let Some(end)=remain.find("]]") else {
            return Err(CitationAuditError::UnknownCitation);
        };
        let token=&remain[..end];
        let number=token.strip_prefix('B')
            .filter(|s|!s.is_empty()&&s.len()<=3
                &&s.bytes().all(|b|b.is_ascii_digit()))
            .and_then(|s|s.parse::<usize>().ok())
            .filter(|n|*n>=1&&*n<=limit)
            .ok_or(CitationAuditError::UnknownCitation)?;
        used.insert(number);
        remain=&remain[end+2..];
    }
    // Reject conventional numbered pseudo-citations like [99], even if the
    // model also supplied an approved [[B1]] marker elsewhere.
    let bytes=paragraph.as_bytes();
    for (i,b) in bytes.iter().enumerate(){
        if *b!=b'['||(i>0&&bytes[i-1]==b'['){continue;}
        let mut at=i+1;
        while at<bytes.len()&&bytes[at].is_ascii_digit(){at+=1;}
        if at>i+1&&at<bytes.len()&&bytes[at]==b']'{
            return Err(CitationAuditError::UnknownCitation);
        }
    }
    Ok(used)
}
fn check_quoted_spans(
    text:&str,approved:&BTreeSet<usize>,sources:&[BrowserCitationSource]
)->Result<(),CitationAuditError>{
    // Both straight and typographic double quotes are treated as claimed
    // direct quotations. Short quoted terms (<=3 characters) are ignored.
    for (open,close) in [('"','"'),('“','”')] {
        let mut pending:Option<usize>=None;
        for (offset,ch) in text.char_indices(){
            if pending.is_none()&&ch==open {
                pending=Some(offset+ch.len_utf8());
            }else if let Some(from)=pending {
                if ch==close {
                    let passage=&text[from..offset];
                    if passage.chars().count()>3
                        &&!approved.iter().any(|n|
                            sources[*n-1].excerpt.contains(passage)){
                        return Err(CitationAuditError::UnsupportedQuotation);
                    }
                    pending=None;
                }
            }
        }
        if pending.is_some(){return Err(CitationAuditError::UnsupportedQuotation);}
    }
    Ok(())
}
/// Every prose paragraph needs at least one native source marker. No model-
/// authored HTTP URL, Markdown link or fabricated numeric citation may pass.
/// Quoted text must appear EXACTLY in a cited, retained source excerpt.
/// This verifies reference/quotation integrity, NOT truth of interpretation.
pub fn audit_browser_research_answer(
    answer:&str,sources:&[BrowserCitationSource]
)->Result<(),CitationAuditError>{
    if sources.is_empty()||sources.len()>120{return Err(CitationAuditError::NoSources);}
    if sources.iter().any(|s|!source_valid(s)){
        return Err(CitationAuditError::InvalidSource);
    }
    if answer.trim().is_empty()||answer.len()>200*1024 {
        return Err(CitationAuditError::InvalidAnswer);
    }
    if answer.chars().any(|c|c=='\0'
        ||(c.is_control()&&c!='\n'&&c!='\r'&&c!='\t')){
        return Err(CitationAuditError::InvalidAnswer);
    }
    // Check case-insensitively. The native host alone emits URLs after audit.
    let lower=answer.to_ascii_lowercase();
    if lower.contains("https://")||lower.contains("http://")
        ||lower.contains("](")||lower.contains("<a ")
        ||lower.contains("www."){
        return Err(CitationAuditError::UnapprovedLink);
    }
    let mut any=false;
    for part in answer.split("\n\n"){
        let paragraph=part.trim();
        if paragraph.is_empty(){continue;}
        let used=markers(paragraph,sources.len())?;
        // Plain headings have no factual citation obligations. Everything
        // else (including list paragraphs) must be source-labeled.
        if paragraph.starts_with('#')
            &&paragraph.lines().all(|line|line.starts_with('#')){continue;}
        if used.is_empty(){return Err(CitationAuditError::MissingCitation);}
        check_quoted_spans(paragraph,&used,sources)?;
        any=true;
    }
    if !any{return Err(CitationAuditError::MissingCitation);}
    Ok(())
}
/// Host-authored provenance (URLs and SHA receipts are never copied from
/// model prose). This is observed Browser evidence, not publisher attestation.
pub fn render_observed_source_appendix(sources:&[BrowserCitationSource])->String{
    let mut out=String::from(
        "\n\nSources (direct Browser observations; bibliographic identity and factual interpretations not independently verified):\n"
    );
    for (i,source) in sources.iter().enumerate(){
        out.push_str(&format!(
            "[[B{}]] {} — observed text SHA-256: {}{}\n",
            i+1,source.url,source.content_sha256,
            if source.truncated{" (excerpt or page truncated)"}else{""}
        ));
    }
    out
}

#[cfg(test)]
mod tests{
    use super::*;
    fn sources()->Vec<BrowserCitationSource>{
        vec![
            BrowserCitationSource{url:"https://archive.example/a".into(),
                content_sha256:"a".repeat(64),
                excerpt:"Evidence A says the estimate was uncertain.".into(),truncated:false},
            BrowserCitationSource{url:"https://journal.example/b".into(),
                content_sha256:"b".repeat(64),
                excerpt:"Study B found a qualified relationship.".into(),truncated:true}
        ]
    }
    #[test]
    fn grounded_markers_and_exact_quotations_pass(){
        let s=sources();
        assert!(audit_browser_research_answer(
            "A source states “the estimate was uncertain” [[B1]].\n\n\
             Another reports a “qualified relationship” [[B2]].",&s).is_ok());
        let appendix=render_observed_source_appendix(&s);
        assert!(appendix.contains("https://archive.example/a"));
        assert!(appendix.contains(" (excerpt or page truncated)"));
        assert!(appendix.contains("not independently verified"));
    }
    #[test]
    fn invented_links_or_citations_are_never_passed_to_completed_chat(){
        let s=sources();
        for answer in [
            "This is a conclusion.",
            "Invented source [[B3]].",
            "Bad numeric reference [99] and real [[B1]].",
            "Source: https://evil.example/article [[B1]].",
            "Source: HTTPS://EVIL.example/article [[B1]].",
            "Click [here](javascript:alert(1)) [[B1]].",
            "Claims [[B1]]\n\nAnother unsupported paragraph.",
            "Fabricated “important new discovery” [[B1]].",
            "Cross-source quotation “qualified relationship” [[B1]].",
            "Incomplete [[B",
        ] {
            assert!(audit_browser_research_answer(answer,&s).is_err(),"{answer}");
        }
    }
    #[test]
    fn labels_are_scoped_to_the_supplied_host_evidence_set(){
        let one=&sources()[..1];
        assert!(audit_browser_research_answer("One claim [[B1]].",one).is_ok());
        assert!(matches!(audit_browser_research_answer("Other claim [[B2]].",one),
            Err(CitationAuditError::UnknownCitation)));
        assert!(audit_browser_research_answer("A quote “qualified relationship” [[B2]].",
            &sources()).is_ok());
        assert!(audit_browser_research_answer("A quote “qualified relationship” [[B1]].",
            &sources()).is_err());
    }
}
