//! RESEARCH-05: bounded HTML/PDF text extraction for host-authorized,
//! independently selected HTTPS research responses. Never executes HTML,
//! embedded PDF actions, scripts, OCR, outbound links or model commands.
//!
//! "Extracted" means text of fetched bytes, NOT a verified publication.
use crate::native_reader_pdf::read_pdf_text;
use crate::native_reader::MAX_READER_OUTPUT_CHARS;
use sha2::{Digest,Sha256};

pub const MAX_RESEARCH_DOCUMENT_BYTES:usize=12*1024*1024;
pub const MAX_RESEARCH_DOCUMENT_TEXT:usize=200_000;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum DocumentError { Invalid,TooLarge,NotReadable,Unsupported,Truncated }
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ResearchDocumentKind { Html, Pdf }
impl ResearchDocumentKind {
    pub fn code(self)->&'static str {
        match self {Self::Html=>"HTML",Self::Pdf=>"PDF"}
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ExtractedResearchDocument {
    pub kind:ResearchDocumentKind,
    pub raw_sha256:String,
    pub text_sha256:String,
    pub text:String,
    pub truncated:bool,
    /// HTML landing-page text may include navigation, previews or boilerplate.
    /// PDF parsed by Lu's conservative parser can be admitted as an accessed
    /// DOCUMENT only after independent URL identity checks and no truncation.
    pub document_candidate:bool,
}
fn sha(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn entity_decode(input:&str)->String {
    let mut result=String::new();
    let mut rest=input;
    while let Some(start)=rest.find('&') {
        result.push_str(&rest[..start]);
        let token=&rest[start..];
        let Some(stop)=token.find(';').filter(|v|*v<=12) else {
            result.push('&');rest=&token[1..];continue;
        };
        let name=&token[1..stop];
        let replacement=match name {
            "amp"=>Some('&'),"lt"=>Some('<'),"gt"=>Some('>'),
            "quot"=>Some('"'),"apos"=>Some('\''),"nbsp"=>Some(' '),
            _=>None,
        }.or_else(||{
            let numbers=name.strip_prefix("#x").or_else(||name.strip_prefix("#X"))
                .map(|s|u32::from_str_radix(s,16))
                .or_else(||name.strip_prefix('#').map(str::parse::<u32>))?;
            char::from_u32(numbers.ok()?).filter(|c|!c.is_control())
        });
        if let Some(c)=replacement{result.push(c)}
        else{result.push_str(&token[..=stop]);}
        rest=&token[stop+1..];
    }
    result.push_str(rest);
    result
}
fn html_text(bytes:&[u8])->Result<(String,bool),DocumentError>{
    let raw=std::str::from_utf8(bytes).map_err(|_|DocumentError::NotReadable)?;
    if !raw.to_ascii_lowercase().contains("<html") && !raw.to_ascii_lowercase().contains("<body")
       && !raw.to_ascii_lowercase().contains("<article") {
        return Err(DocumentError::Unsupported);
    }
    let mut text=String::new();
    let mut i=0usize;
    let mut skip:Option<String>=None;
    let mut steps=0usize;
    while i<raw.len(){
        steps+=1;if steps>3_000_000{return Err(DocumentError::TooLarge);}
        let tail=&raw[i..];
        if tail.starts_with('<'){
            let Some(close)=tail.find('>') else{return Err(DocumentError::NotReadable)};
            if close>8192{return Err(DocumentError::TooLarge);}
            let tag=tail[1..close].trim().to_ascii_lowercase();
            if tag.starts_with("!--"){
                if let Some(end)=tail.find("-->"){i+=end+3;continue}
                return Err(DocumentError::NotReadable);
            }
            let token=tag.trim_start_matches('/').split(|c:char|
                c.is_whitespace()||c=='/').next().unwrap_or("");
            let closing=tag.starts_with('/');
            if let Some(expected)=&skip {
                if closing && token==expected {skip=None;}
            } else if !closing && matches!(
                token,"head"|"script"|"style"|"svg"|"iframe"|"noscript"|"template"
            ) {
                skip=Some(token.to_owned());
            } else if matches!(
                token,"p"|"br"|"div"|"li"|"h1"|"h2"|"h3"|"h4"|"h5"|"h6"
                    |"section"|"article"|"blockquote"|"tr"|"td"
            ) {
                text.push('\n');
            }
            i+=close+1;
        } else {
            let end=tail.find('<').unwrap_or(tail.len());
            if skip.is_none(){
                let chunk=entity_decode(&tail[..end]);
                if chunk.chars().any(|c|c=='\0'){return Err(DocumentError::NotReadable);}
                text.push_str(&chunk);
                if text.chars().count()>MAX_RESEARCH_DOCUMENT_TEXT+1 {
                    return Ok((text.chars().take(MAX_RESEARCH_DOCUMENT_TEXT).collect(),true));
                }
            }
            i+=end;
        }
    }
    let cleaned=text.lines()
        .map(str::trim).filter(|l|!l.is_empty())
        .collect::<Vec<_>>().join("\n");
    if cleaned.trim().is_empty(){return Err(DocumentError::NotReadable);}
    Ok((cleaned,false))
}
pub fn extract_research_document(bytes:&[u8])
    ->Result<ExtractedResearchDocument,DocumentError>{
    if bytes.is_empty(){return Err(DocumentError::Invalid);}
    if bytes.len()>MAX_RESEARCH_DOCUMENT_BYTES{return Err(DocumentError::TooLarge);}
    let (kind,text,truncated)=if bytes.starts_with(b"%PDF-"){
        let page=read_pdf_text(bytes,MAX_READER_OUTPUT_CHARS)
            .map_err(|_|DocumentError::Unsupported)?;
        (ResearchDocumentKind::Pdf,page.text,page.truncated)
    }else{
        let (text,truncated)=html_text(bytes)?;
        (ResearchDocumentKind::Html,text,truncated)
    };
    if text.trim().is_empty(){return Err(DocumentError::NotReadable);}
    Ok(ExtractedResearchDocument{
        kind,raw_sha256:sha(bytes),text_sha256:sha(text.as_bytes()),
        document_candidate:kind==ResearchDocumentKind::Pdf && !truncated,
        text,truncated,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn html_extractor_ignores_code_and_preserves_only_observed_text(){
        let html=b"<html><head><title>Wrong document</title></head><body>\
            <article><h1>Findings &amp; limitations</h1>\
            <p>The result was &quot;uncertain&quot;.</p>\
            <script>alert('fake quote');</script><style>.hidden{color:red}</style>\
            <p>Publication note.</p></article></body></html>";
        let doc=extract_research_document(html).unwrap();
        assert_eq!(doc.kind,ResearchDocumentKind::Html);
        assert!(doc.text.contains("Findings & limitations"));
        assert!(doc.text.contains("The result was \"uncertain\"."));
        assert!(!doc.text.contains("fake quote"));
        assert!(!doc.text.contains("Wrong document"));
        assert!(!doc.document_candidate);
        assert_eq!(doc.raw_sha256.len(),64);
    }
    #[test]
    fn malformed_and_unreadable_document_fail_closed(){
        assert_eq!(extract_research_document(b"<html><body>open tag").err(),
            Some(DocumentError::NotReadable));
        assert!(extract_research_document(b"\x89PNGimage").is_err());
        assert!(extract_research_document(&vec![b'A';MAX_RESEARCH_DOCUMENT_BYTES+1]).is_err());
    }
    #[test]
    fn html_entity_numeric_and_named_decode_only_known_entities(){
        assert_eq!(entity_decode("A&amp;B&#33;&#x21;&unknown;"),
            "A&B!!&unknown;");
    }
}
