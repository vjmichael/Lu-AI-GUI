//! TOOLS-10A: authoritative DOI agency lookup, not model-supplied citations.
//! A bibliographic claim is corroborated only when the fetched registry record
//! identifies the same DOI and supplies its actual title and authors.
use crate::web_http_fetch::{FetchError, SecureHttpFetcher};
use crate::web_research_evidence::TrustedBibliography;
use crate::web_research_planner::canonical_doi;
use serde_json::Value;

#[derive(Debug,PartialEq,Eq)]
pub enum CatalogError {
    InvalidIdentifier,
    Transport,
    NotFound,
    RateLimited,
    ProviderDenied,
    InvalidMetadata,
    MismatchedIdentifier,
}
impl From<FetchError> for CatalogError {
    fn from(_:FetchError)->Self {Self::Transport}
}
fn path_encode(value:&str)->String {
    let mut out=String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'|b'.'|b'~'|b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
pub fn parse_crossref_record(body:&[u8],queried_doi:&str)
    -> Result<TrustedBibliography,CatalogError> {
    let doi=canonical_doi(queried_doi).ok_or(CatalogError::InvalidIdentifier)?;
    if body.len()>1024*1024 {return Err(CatalogError::InvalidMetadata);}
    let obj:Value=serde_json::from_slice(body).map_err(|_|CatalogError::InvalidMetadata)?;
    let message=obj.get("message").ok_or(CatalogError::InvalidMetadata)?;
    let returned=message.get("DOI").and_then(Value::as_str)
        .ok_or(CatalogError::InvalidMetadata)?;
    let returned=canonical_doi(returned).ok_or(CatalogError::InvalidMetadata)?;
    if returned!=doi {return Err(CatalogError::MismatchedIdentifier);}
    let title=message.get("title").and_then(Value::as_array).and_then(|a|a.first())
        .and_then(Value::as_str).ok_or(CatalogError::InvalidMetadata)?;
    if title.trim().is_empty() || title.len()>1024 {return Err(CatalogError::InvalidMetadata);}
    let authors=message.get("author").and_then(Value::as_array)
        .ok_or(CatalogError::InvalidMetadata)?;
    if authors.is_empty() || authors.len()>128 {return Err(CatalogError::InvalidMetadata);}
    let mut names=Vec::new();
    for author in authors {
        let given=author.get("given").and_then(Value::as_str).unwrap_or("");
        let family=author.get("family").and_then(Value::as_str).unwrap_or("");
        let name=format!("{given} {family}").trim().to_owned();
        if name.is_empty() || name.len()>128 {return Err(CatalogError::InvalidMetadata);}
        names.push(name);
    }
    Ok(TrustedBibliography {
        record_url:format!("https://api.crossref.org/works/{}",path_encode(&doi)),
        title:title.to_owned(),
        author:names.join("; "),
        identifier:format!("doi:{doi}"),
    })
}
pub fn resolve_crossref(fetcher:&SecureHttpFetcher<'_>,doi:&str)
    -> Result<TrustedBibliography,CatalogError> {
    let canonical=canonical_doi(doi).ok_or(CatalogError::InvalidIdentifier)?;
    let url=format!("https://api.crossref.org/works/{}",path_encode(&canonical));
    let page=fetcher.fetch(&url)?;
    match page.http_status {
        200=>parse_crossref_record(&page.bytes,&canonical),
        404=>Err(CatalogError::NotFound),
        429=>Err(CatalogError::RateLimited),
        401|403=>Err(CatalogError::ProviderDenied),
        _=>Err(CatalogError::Transport),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn record(doi:&str,title:&str)->Vec<u8> {
        serde_json::json!({"message":{"DOI":doi,"title":[title],
            "author":[{"given":"Jane","family":"Doe"}]}}).to_string().into_bytes()
    }
    #[test]
    fn genuine_doi_attached_to_other_article_is_refused() {
        assert_eq!(parse_crossref_record(&record("10.1234/other","Another Article"),
            "10.1234/claimed").unwrap_err(),CatalogError::MismatchedIdentifier);
    }
    #[test]
    fn authenticated_catalog_metadata_must_match_candidate() {
        let book=parse_crossref_record(&record("10.1234/study","Actual Study"),
            "10.1234/study").unwrap();
        assert_eq!(book.author,"Jane Doe");
        assert_eq!(book.title,"Actual Study");
        assert_eq!(book.identifier,"doi:10.1234/study");
    }
    #[test]
    fn metadata_without_real_authors_is_not_verified() {
        let empty=serde_json::json!({"message":{"DOI":"10.1234/study",
            "title":["Actual Study"],"author":[]}}).to_string();
        assert_eq!(parse_crossref_record(empty.as_bytes(),"10.1234/study").unwrap_err(),
                   CatalogError::InvalidMetadata);
    }
}
