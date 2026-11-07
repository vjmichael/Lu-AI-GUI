//! P07 RESEARCH-STYLE-01: deterministic scholarly bibliographic formatting.
//! Formatting is NOT factual/quotation verification. Inputs must be native
//! catalog metadata linked to an already corroborated EvidenceLedger record.
//! Limited to books/journal articles; no invented years, pages, abbreviations
//! or unsupported ancient work/translation references. No network activity.
use crate::web_research_evidence::{EvidenceLedger,SourceStatus};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Style{Apa7,Mla9,Chicago18,Turabian9,Sbl2}
impl Style{
    pub fn label(self)->&'static str{
        match self{
            Self::Apa7=>"APA 7",Self::Mla9=>"MLA 9",
            Self::Chicago18=>"Chicago 18 Notes-Bibliography",
            Self::Turabian9=>"Turabian 9 Notes-Bibliography",
            Self::Sbl2=>"SBL Handbook 2 Notes-Bibliography",
        }
    }
    pub fn uses_notes(self)->bool{
        matches!(self,Self::Chicago18|Self::Turabian9|Self::Sbl2)
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Kind{Book,JournalArticle,AncientSource}
#[derive(Debug,Clone)]
pub struct Author{pub family:String,pub given:String}
#[derive(Debug,Clone)]
pub struct CatalogWork{
    pub source_id:String,pub title:String,pub record_url:String,
    pub kind:Kind,pub authors:Vec<Author>,pub year:Option<u16>,
    pub publisher:Option<String>,pub publication_place:Option<String>,
    pub journal:Option<String>,pub journal_abbreviation:Option<String>,
    pub volume:Option<String>,pub issue:Option<String>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct RenderedCitation{
    pub style:&'static str,pub source_id:String,pub bibliography:String,
    pub first_note:Option<String>,
    pub bibliography_identity_corroborated:bool,
    pub author_identity_corroborated:bool,
    /// Secondary year/publisher/journal detail is not yet independently
    /// corroborated in the existing ledger. Requires a future catalog adapter.
    pub publication_details_verified:bool,
    pub quotation_or_page_verified:bool,
    pub factual_claim_verified:bool,
    pub coverage:&'static str,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum StyleError{Uncorroborated,IdentityMismatch,MissingField,InvalidField,Unsupported}
fn bounded(s:&str,n:usize)->bool{
    !s.is_empty()&&s.trim()==s&&s.len()<=n
    &&!s.chars().any(|ch|ch.is_control()||matches!(ch,'<'|'>'|'['|']'))
}
fn field(x:Option<&String>)->Result<&str,StyleError>{
    x.map(String::as_str).ok_or(StyleError::MissingField)
}
fn full(a:&Author)->String{format!("{} {}",a.given,a.family)}
fn inverted(a:&Author)->String{format!("{}, {}",a.family,a.given)}
fn initials(s:&str)->String{
    s.split_whitespace().filter_map(|w|w.chars().next())
        .map(|c|format!("{}.",c.to_uppercase()))
        .collect::<Vec<_>>().join(" ")
}
fn authors(style:Style,xs:&[Author])->String{
    let first=&xs[0];
    match style {
        Style::Apa7=>{
            let names=xs.iter().map(|a|format!("{}, {}",a.family,initials(&a.given)))
                .collect::<Vec<_>>();
            if names.len()==1{names[0].clone()}
            else{format!("{} & {}",names[..names.len()-1].join(", "),names[names.len()-1])}
        },
        Style::Mla9=>{
            if xs.len()>2{format!("{} et al.",inverted(first))}
            else if xs.len()==2{format!("{} and {}",inverted(first),full(&xs[1]))}
            else{inverted(first)}
        },
        _=>{
            if xs.len()>6{format!("{} et al.",inverted(first))}
            else if xs.len()==1{inverted(first)}
            else{
                let mut names=vec![inverted(first)];
                names.extend(xs[1..].iter().map(full));
                let last=names.pop().unwrap_or_default();
                format!("{} and {last}",names.join(", "))
            }
        }
    }
}
fn note_author(xs:&[Author])->String{
    if xs.len()>2{format!("{} et al.",full(&xs[0]))}
    else if xs.len()==2{format!("{} and {}",full(&xs[0]),full(&xs[1]))}
    else{full(&xs[0])}
}
/// This function is host-side only; it has no model-visible command route.
/// Metadata is corroborated only for the ledger-bound title and URL. Author,
/// date, publisher, journal and abbreviation are independently reviewed
/// catalog fields, not claims made by scraped pages or model prose.
pub fn render_correlated(
    ledger:&EvidenceLedger,work:&CatalogWork,style:Style
)->Result<RenderedCitation,StyleError>{
    if !bounded(&work.source_id,128)||!bounded(&work.title,1024)
      ||!bounded(&work.record_url,2048)
      ||!work.record_url.starts_with("https://")
      ||work.record_url.contains('@')||work.record_url.contains('\\')
      ||work.authors.is_empty()||work.authors.len()>20
      ||work.authors.iter().any(|a|!bounded(&a.family,128)||!bounded(&a.given,128))
      ||[work.publisher.as_ref(),work.publication_place.as_ref(),
          work.journal.as_ref(),work.journal_abbreviation.as_ref(),
          work.volume.as_ref(),work.issue.as_ref()].iter()
          .flatten().any(|v|!bounded(v,512)){
        return Err(StyleError::InvalidField);
    }
    let source=ledger.record(&work.source_id)
        .map_err(|_|StyleError::Uncorroborated)?;
    if !matches!(source.status,SourceStatus::MetadataCorroborated
       |SourceStatus::DocumentAccessed|SourceStatus::PassageVerified){
        return Err(StyleError::Uncorroborated);
    }
    if source.title!=work.title||
       source.record_url.as_deref()!=Some(work.record_url.as_str()){
        return Err(StyleError::IdentityMismatch);
    }
    // The model must not replace the author list after catalog
    // corroboration. Compare against the original discovered identity which
    // corroborate() checked against the trusted bibliography author field.
    let catalog=ledger.discovered_candidate(&work.source_id)
        .map_err(|_|StyleError::Uncorroborated)?;
    let names=work.authors.iter().map(full).collect::<Vec<_>>();
    let attested_one=names.join("; ");
    let attested_other=names.join(", ");
    let original=catalog.author.split_whitespace().collect::<Vec<_>>().join(" ");
    if original!=attested_one&&original!=attested_other{
        return Err(StyleError::IdentityMismatch);
    }
    if work.kind==Kind::AncientSource{
        // Must additionally validate ancient work, edition, translation,
        // explicit passage locator and an SBL abbreviation. Never guess.
        return Err(StyleError::Unsupported);
    }
    let year=work.year.filter(|y|*y>=1000&&*y<=2200)
        .ok_or(StyleError::MissingField)?;
    let who=authors(style,&work.authors);
    let (bib,note)=match work.kind{
        Kind::Book=>{
            let publisher=field(work.publisher.as_ref())?;
            let imprint=if matches!(style,Style::Turabian9|Style::Sbl2){
                format!("{}: {publisher}",field(work.publication_place.as_ref())?)
            }else{publisher.into()};
            let entry=match style{
                Style::Apa7=>format!("{who} ({year}). {}. {imprint}. {}",
                    work.title,work.record_url),
                _=>format!("{who}. {}. {imprint}, {year}. {}",
                    work.title,work.record_url),
            };
            let note=style.uses_notes().then(||format!("{}, {} ({imprint}, {year}).",
                note_author(&work.authors),work.title));
            (entry,note)
        },
        Kind::JournalArticle=>{
            let title=if style==Style::Sbl2{
                field(work.journal_abbreviation.as_ref())?
            }else{field(work.journal.as_ref())?};
            let volume=field(work.volume.as_ref())?;
            let issue=work.issue.as_deref().map(|v|format!(", no. {v}"))
                .unwrap_or_default();
            let entry=match style{
                Style::Apa7=>format!("{who} ({year}). {}. {title}, {volume}{}. {}",
                    work.title,work.issue.as_deref().map(|v|format!("({v})"))
                    .unwrap_or_default(),work.record_url),
                Style::Mla9=>format!("{who}. \"{}.\" {title}, vol. {volume}{issue}, {year}. {}",
                    work.title,work.record_url),
                _=>format!("{who}. \"{}.\" {title} {volume}{issue} ({year}). {}",
                    work.title,work.record_url),
            };
            let note=style.uses_notes().then(||format!("{}, \"{},\" {title} {volume}{issue} ({year}).",
                note_author(&work.authors),work.title));
            (entry,note)
        },
        Kind::AncientSource=>unreachable!(),
    };
    Ok(RenderedCitation{
        style:style.label(),source_id:work.source_id.clone(),
        bibliography:if bib.ends_with('.') {bib}else{format!("{bib}.")},
        first_note:note,bibliography_identity_corroborated:true,
        author_identity_corroborated:true,
        publication_details_verified:false,
        quotation_or_page_verified:false,factual_claim_verified:false,
        coverage:"BASIC_BOOK_JOURNAL_ONLY_REVIEW_REQUIRED",
    })
}

/// A bounded CSL-JSON-compatible data record; not a CSL rendering engine.
    /// It exports *only* reviewed fields, preserving provenance and no-page
    /// honesty. Never serializes page numbers, guesses abbreviations or
    /// invents article metadata.
pub fn export_csl_json(
    ledger:&EvidenceLedger,work:&CatalogWork
)->Result<serde_json::Value,StyleError>{
    let _=render_correlated(ledger,work,Style::Chicago18)?;
    let year=work.year.ok_or(StyleError::MissingField)?;
    let author=work.authors.iter().map(|a|
        serde_json::json!({"family":a.family,"given":a.given})).collect::<Vec<_>>();
    let mut item=serde_json::json!({
        "id":work.source_id,
        "type":if work.kind==Kind::Book{"book"}else{"article-journal"},
        "title":work.title,
        "URL":work.record_url,
        "author":author,
        "issued":{"date-parts":[[year]]},
        "_lu_evidence":{
            "source_id":work.source_id,
            "bibliography_identity_corroborated":true,
            "author_identity_corroborated":true,
            "publication_details_verified":false,
            "quotation_or_page_verified":false,
            "factual_claim_verified":false
        }
    });
    if let Some(publisher)=&work.publisher{
        item["publisher"]=serde_json::json!(publisher);
    }
    if let Some(place)=&work.publication_place{
        item["publisher-place"]=serde_json::json!(place);
    }
    if work.kind==Kind::JournalArticle{
        if let Some(journal)=&work.journal{
            item["container-title"]=serde_json::json!(journal);
        }
        if let Some(volume)=&work.volume{
            item["volume"]=serde_json::json!(volume);
        }
        if let Some(issue)=&work.issue{
            item["issue"]=serde_json::json!(issue);
        }
    }
    Ok(item)
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::web_research_evidence::{Candidate,TrustedBibliography};
    use std::path::Path;
    fn verified()->EvidenceLedger{
        let mut l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        l.discover(&Candidate{id:"source-1".into(),title:"Study".into(),
          author:"Jane Doe".into(),identifier:"doi:10.1000/example".into()}).unwrap();
        l.corroborate("source-1",&TrustedBibliography{
          record_url:"https://publisher.example/study".into(),
          title:"Study".into(),author:"Jane Doe".into(),
          identifier:"doi:10.1000/example".into()}).unwrap();
        l
    }
    fn book()->CatalogWork{
        CatalogWork{
            source_id:"source-1".into(),title:"Study".into(),
            record_url:"https://publisher.example/study".into(),
            kind:Kind::Book,authors:vec![Author{family:"Doe".into(),given:"Jane".into()}],
            year:Some(2024),publisher:Some("Academic Press".into()),
            publication_place:Some("Chicago".into()),journal:None,
            journal_abbreviation:None,volume:None,issue:None,
        }
    }
    #[test]
    fn five_styles_emit_correlated_book_not_a_factual_or_page_verdict(){
        let l=verified();let work=book();
        for style in [Style::Apa7,Style::Mla9,Style::Chicago18,Style::Turabian9,Style::Sbl2]{
            let out=render_correlated(&l,&work,style).unwrap();
            assert!(out.bibliography.contains("https://publisher.example/study"));
            assert_eq!(out.first_note.is_some(),style.uses_notes());
            assert!(!out.quotation_or_page_verified&&!out.factual_claim_verified);
        }
        assert!(render_correlated(&l,&work,Style::Apa7).unwrap()
            .bibliography.contains("Doe, J. (2024)"));
        assert!(render_correlated(&l,&work,Style::Sbl2).unwrap()
            .bibliography.contains("Chicago: Academic Press"));
    }
    #[test]
    fn uncorroborated_mismatched_or_ancient_source_is_never_fabricated(){
        let missing=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        assert_eq!(render_correlated(&missing,&book(),Style::Apa7),
            Err(StyleError::Uncorroborated));
        let l=verified();
        let mut wrong=book();wrong.title="Unrelated".into();
        assert_eq!(render_correlated(&l,&wrong,Style::Apa7),
            Err(StyleError::IdentityMismatch));
        let mut ancient=book();ancient.kind=Kind::AncientSource;
        assert_eq!(render_correlated(&l,&ancient,Style::Sbl2),
            Err(StyleError::Unsupported));
        let mut undated=book();undated.year=None;
        assert_eq!(render_correlated(&l,&undated,Style::Mla9),
            Err(StyleError::MissingField));
    }
    #[test]
    fn csl_json_preserves_attested_identity_and_never_invents_locator(){
        let ledger=verified();let work=book();
        let json=export_csl_json(&ledger,&work).unwrap();
        assert_eq!(json["type"],"book");
        assert_eq!(json["author"][0]["family"],"Doe");
        assert_eq!(json["issued"]["date-parts"][0][0],2024);
        assert_eq!(json["_lu_evidence"]["publication_details_verified"],false);
        assert!(json.get("page").is_none());
        let mut fake=work.clone();
        fake.authors[0].family="Impostor".into();
        assert_eq!(render_correlated(&ledger,&fake,Style::Apa7),
            Err(StyleError::IdentityMismatch));
        assert_eq!(export_csl_json(&ledger,&fake),
            Err(StyleError::IdentityMismatch));
    }
    #[test]
    fn sbl_does_not_invent_journal_abbreviations(){
        let l=verified();let mut work=book();
        work.kind=Kind::JournalArticle;work.journal=Some("Journal of History".into());
        work.volume=Some("12".into());work.issue=Some("2".into());
        assert_eq!(render_correlated(&l,&work,Style::Sbl2),
            Err(StyleError::MissingField));
        work.journal_abbreviation=Some("JH".into());
        assert!(render_correlated(&l,&work,Style::Sbl2).unwrap()
            .bibliography.contains("JH 12, no. 2 (2024)"));
    }
}
