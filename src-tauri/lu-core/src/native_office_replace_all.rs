//! TOOLS-COMPLETE-C: deterministic, exact Word text-run find/replace.
//! Intentionally NOT regex, cross-run heuristic replacement, or destructive
//! rewriting of an existing OOXML package. A hit is one fully matched w:t.
//! Proven existing Office revision batch engine preserves non-target ZIP parts
//! and refuses macros, external links, signatures and ambiguous part names.
use crate::native_office_revision::{
    render_office_revision_batch,OfficeRevisionEdit,OfficeRevisionError,
    OfficeRevisionPreview,
};
use quick_xml::{events::Event,Reader,escape::unescape};
use sha2::{Digest,Sha256};
use std::io::{Cursor,Read};
use zip::ZipArchive;

const MAX_REPLACEMENTS:usize=32;
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct WordTextReplacePlan{
    pub match_count:usize,
    pub edited_run_indices:Vec<usize>,
    pub original_sha256:String,
    pub revised_sha256:String,
}
pub struct WordTextReplaceOutput{
    pub plan:WordTextReplacePlan,
    pub preview:OfficeRevisionPreview,
    pub edits:Vec<OfficeRevisionEdit>,
}
fn valid_text(s:&str)->bool{
    !s.trim().is_empty()&&s.len()<=8192
        &&!s.chars().any(|c|c=='\0'||c=='\u{fffe}'||c=='\u{ffff}')
}
fn elem(s:&[u8])->&[u8]{s.rsplit(|c|*c==b':').next().unwrap_or(s)}
/// Caller must obtain source via an OS picker, pin current file SHA and
/// present count/digest in a separate approval UI before any publication.
pub fn plan_word_run_replace_all(
    source:&[u8],original_sha256:&str,old_text:&str,new_text:&str
)->Result<WordTextReplaceOutput,OfficeRevisionError>{
    if !valid_text(old_text)||!valid_text(new_text)||old_text==new_text{
        return Err(OfficeRevisionError::InvalidContent);
    }
    if source.len()>12*1024*1024 {
        return Err(OfficeRevisionError::TooLarge);
    }
    if format!("{:x}",Sha256::digest(source))!=original_sha256 {
        return Err(OfficeRevisionError::SourceChanged);
    }
    let mut zip=ZipArchive::new(Cursor::new(source))
        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
    if zip.has_overlapping_entries().map_err(|_|OfficeRevisionError::InvalidPackage)?
        ||zip.validate_header_consistency().is_err(){
        return Err(OfficeRevisionError::InvalidPackage);
    }
    if zip.len()>2048{return Err(OfficeRevisionError::TooLarge);}
    let mut file=zip.by_name("word/document.xml")
        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
    if file.size()>8*1024*1024{return Err(OfficeRevisionError::TooLarge);}
    let mut xml=Vec::new();
    (&mut file).take(8*1024*1024+1).read_to_end(&mut xml)
        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
    if xml.len()>8*1024*1024{return Err(OfficeRevisionError::TooLarge);}
    let xml=std::str::from_utf8(&xml).map_err(|_|OfficeRevisionError::InvalidPackage)?;
    let mut reader=Reader::from_str(xml);
    let mut text_index=0usize;
    let mut hits=Vec::new();
    let mut steps=0usize;
    loop{
        steps+=1;
        if steps>500_000{return Err(OfficeRevisionError::TooLarge);}
        let event=reader.read_event().map_err(|_|OfficeRevisionError::InvalidPackage)?;
        match event{
            Event::Start(element) if elem(element.name().as_ref())==b"t"=>{
                let from=reader.buffer_position() as usize;
                let span=reader.read_to_end(element.name())
                    .map_err(|_|OfficeRevisionError::InvalidPackage)?;
                let to=span.end as usize;
                let inner=xml.get(from..to).ok_or(OfficeRevisionError::InvalidPackage)?;
                if !inner.contains('<'){
                    let previous=unescape(inner)
                        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
                    if previous==old_text{
                        hits.push(text_index);
                        if hits.len()>MAX_REPLACEMENTS{
                            return Err(OfficeRevisionError::TooLarge);
                        }
                    }
                }
                text_index+=1;
                if text_index>=10_000{return Err(OfficeRevisionError::TooLarge);}
            },
            Event::DocType(_)=>return Err(OfficeRevisionError::UnsafePackage),
            Event::Eof=>break,
            _=>{},
        }
    }
    if hits.is_empty(){return Err(OfficeRevisionError::NotFound);}
    let edits=hits.iter().copied().map(|text_index|OfficeRevisionEdit::WordRun{
        text_index,expected_old:old_text.into(),new_text:new_text.into(),
    }).collect::<Vec<_>>();
    let revised=render_office_revision_batch(source,original_sha256,&edits)?;
    let plan=WordTextReplacePlan{
        match_count:edits.len(),edited_run_indices:hits,
        original_sha256:original_sha256.into(),
        revised_sha256:revised.revised_sha256.clone(),
    };
    Ok(WordTextReplaceOutput{plan,preview:revised,edits})
}
