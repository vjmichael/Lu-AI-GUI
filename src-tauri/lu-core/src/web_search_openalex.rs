//! TOOLS-10C: OpenAlex discovery adapter. Search hits are leads, never citations.
//! The separate Crossref + EvidenceLedger gate must verify each candidate.
use crate::web_http_fetch::{SecureHttpFetcher,FetchError};
use crate::web_research_planner::canonical_doi;
use serde_json::Value;
use std::sync::atomic::AtomicBool;

#[derive(Debug,PartialEq,Eq)]
pub enum DiscoveryError { InvalidQuery, Transport, Unavailable, InvalidResponse }
impl From<FetchError> for DiscoveryError {
    fn from(_:FetchError)->Self {Self::Transport}
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct DiscoveryLead {
    pub provider:String, pub provider_id:String, pub doi:String,
    pub title:String, pub authors:String, pub catalog_url:String,
}
#[derive(Debug)]
pub struct DiscoveryPage {
    pub leads:Vec<DiscoveryLead>, pub page:u32, pub next_page:Option<u32>,
    pub observed_sha256:String, pub query:String,
}
fn percent_encode(text:&str)->String {
    let mut out=String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'|b'~') {
            out.push(b as char);
        } else {out.push_str(&format!("%{b:02X}"));}
    }
    out
}
pub fn parse_openalex_page(bytes:&[u8],query:&str,page:u32,checksum:&str)
    ->Result<DiscoveryPage,DiscoveryError>{
    if bytes.len()>1024*1024 || page==0 || page>100 || query.len()>512 {
        return Err(DiscoveryError::InvalidResponse);
    }
    let value:Value=serde_json::from_slice(bytes).map_err(|_|DiscoveryError::InvalidResponse)?;
    let results=value.get("results").and_then(Value::as_array).ok_or(DiscoveryError::InvalidResponse)?;
    if results.len()>100 {return Err(DiscoveryError::InvalidResponse);}
    let mut leads=Vec::new();
    for work in results {
        let Some(id)=work.get("id").and_then(Value::as_str) else {continue;};
        let Some(title)=work.get("display_name").and_then(Value::as_str) else {continue;};
        let Some(doi)=work.get("doi").and_then(Value::as_str).and_then(canonical_doi) else {continue;};
        if !id.starts_with("https://openalex.org/") || id.len()>256 ||
           title.trim().is_empty() || title.len()>1024 {continue;}
        let Some(authorships)=work.get("authorships").and_then(Value::as_array) else {continue;};
        if authorships.len()>128 || authorships.is_empty(){continue;}
        let mut names=Vec::new();
        for item in authorships {
            if let Some(name)=item.get("author").and_then(|a|a.get("display_name")).and_then(Value::as_str) {
                if !name.trim().is_empty() && name.len()<=128 {names.push(name);}
            }
        }
        if names.is_empty(){continue;}
        leads.push(DiscoveryLead {
            provider:"openalex".into(),provider_id:id.into(),
            doi,title:title.into(),authors:names.join("; "),
            catalog_url:id.into(),
        });
    }
    let count=value.get("meta").and_then(|m|m.get("count")).and_then(Value::as_u64);
    let next_page=if page<100 && count.is_some_and(|c| (page as u64)*25<c){
        Some(page+1)
    } else {None};
    Ok(DiscoveryPage{
        leads,page,next_page,observed_sha256:checksum.into(),query:query.into()
    })
}
pub fn search_openalex_cancellable(
    fetcher:&SecureHttpFetcher<'_>,query:&str,page:u32,cancel:&AtomicBool
)->Result<DiscoveryPage,DiscoveryError>{
    if query.trim().is_empty()||query.len()>512
        ||query.chars().any(char::is_control)||page==0||page>100{
        return Err(DiscoveryError::InvalidQuery);
    }
    let address=format!(
        "https://api.openalex.org/works?search={}&per-page=25&page={}",
        percent_encode(query),page
    );
    let result=fetcher.fetch_cancellable(&address,cancel)?;
    if result.http_status!=200{return Err(DiscoveryError::Unavailable);}
    parse_openalex_page(&result.bytes,query,page,&result.sha256)
}
pub fn search_openalex(fetcher:&SecureHttpFetcher<'_>,query:&str,page:u32)
    ->Result<DiscoveryPage,DiscoveryError>{
    if query.trim().is_empty()||query.len()>512||query.chars().any(char::is_control)
        ||page==0||page>100 {return Err(DiscoveryError::InvalidQuery);}
    let address=format!("https://api.openalex.org/works?search={}&per-page=25&page={}",
        percent_encode(query),page);
    let result=fetcher.fetch(&address)?;
    if result.http_status!=200 {return Err(DiscoveryError::Unavailable);}
    parse_openalex_page(&result.bytes,query,page,&result.sha256)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn twenty_five_search_pages_can_reach_real_research_depth_without_forged_verification(){
        let items:Vec<Value>=(0..25).map(|i|serde_json::json!({
            "id":format!("https://openalex.org/W{}",1000+i),
            "display_name":format!("Study {i}"),"doi":format!("https://doi.org/10.1234/study{i}"),
            "authorships":[{"author":{"display_name":"Jane Doe"}}]
        })).collect();
        let fixture=serde_json::json!({"meta":{"count":180},"results":items}).to_string();
        let page=parse_openalex_page(fixture.as_bytes(),"thesis question",1,"hash").unwrap();
        assert_eq!(page.leads.len(),25);
        assert_eq!(page.next_page,Some(2));
        assert_eq!(page.leads[0].doi,"10.1234/study0");
        assert_eq!(page.leads[0].provider,"openalex");
    }
    #[test]
    fn search_results_without_dois_are_leads_not_verified_sources(){
        let payload=serde_json::json!({"meta":{"count":1},"results":[{
           "id":"https://openalex.org/W1","display_name":"Original manuscript",
           "authorships":[{"author":{"display_name":"Archivist"}}]
        }]}).to_string();
        let output=parse_openalex_page(payload.as_bytes(),"originals",1,"hash").unwrap();
        assert!(output.leads.is_empty());
    }
}
