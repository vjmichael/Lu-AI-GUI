//! Wave 2: non-destructive OOXML revision of existing Word, Excel and
//! PowerPoint documents into a NEW version (never silently overwrite).
//! Only one existing text node or one simple numeric/boolean cell is changed.
//! Every other ZIP part is copied with identical UNCOMPRESSED content bytes;
//! compression headers/timestamps are NOT byte-for-byte preserved.
//! Signed, encrypted, macro-bearing and external-link packages are refused.
//! Caller must bind an exact original file SHA, approved new SHA, source file
//! identity, destination and persisted-human-turn journal before publication.
use std::{
    collections::BTreeSet,
    io::{Cursor,Read,Write},
};
use cap_std::fs::Dir;
use quick_xml::{
    events::{BytesStart,Event},
    reader::Reader,
    escape::{unescape,escape},
};
use sha2::{Digest,Sha256};
use zip::{ZipArchive,ZipWriter,write::SimpleFileOptions,CompressionMethod};
use crate::native_atomic_write::{
    create_new_atomic_in_dir,AtomicWriteError,WriteReceipt,
};
const MAX_PACKAGE:usize=1024*1024;
const MAX_PARTS:usize=256;
const MAX_PART:usize=512*1024;
const MAX_TEXT:usize=32_768;
#[derive(Debug,Clone,PartialEq)]
pub enum OfficeRevisionEdit {
    WordRun {text_index:usize,expected_old:String,new_text:String},
    SlideRun {slide_number:usize,text_index:usize,expected_old:String,new_text:String},
    CellNumber {sheet_number:usize,address:String,expected_old:f64,new_number:f64},
    CellBoolean {sheet_number:usize,address:String,expected_old:bool,new_boolean:bool},
    CellInlineText {sheet_number:usize,address:String,expected_old:String,new_text:String},
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum OfficeRevisionError {
    InvalidName,InvalidPackage,UnsafePackage,UnsupportedPackage,
    InvalidTarget,NotFound,AmbiguousTarget,SourceChanged,InvalidContent,
    TooLarge,UnsupportedCell,Archive(String),Publish(AtomicWriteError),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct OfficeRevisionPreview{
    pub original_sha256:String,
    pub revised_sha256:String,
    pub revised_bytes:Vec<u8>,
    pub target_part:String,
    pub unaffected_parts:Vec<(String,String)>,
}
fn hash(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
fn basename(s:&str)->bool{
    !s.is_empty()&&s.len()<=200&&s!="."&&s!=".."
        && !s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
        && !s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn safe_part(s:&str)->bool{
    !s.is_empty()&&s.len()<=512&&!s.starts_with('/')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'\\'|':'))
        &&s.trim_end_matches('/').split('/').all(|p|
            !p.is_empty()&&p!="."&&p!=".."&&!p.ends_with('.')
            &&!p.eq_ignore_ascii_case(".git"))
}
fn text_ok(s:&str)->bool{
    s.len()<=MAX_TEXT&&s.chars().all(|c|
        c=='\r'||c=='\n'||c=='\t'||(!c.is_control()&&c!='\u{FFFE}'&&c!='\u{FFFF}'))
}
fn elem_name(tag:&[u8])->&[u8]{
    tag.rsplit(|c|*c==b':').next().unwrap_or(tag)
}
fn value_attr(start:&BytesStart<'_>,attr:&[u8])->Result<Option<String>,OfficeRevisionError>{
    for item in start.attributes(){
        let a=item.map_err(|_|OfficeRevisionError::InvalidPackage)?;
        if elem_name(a.key.as_ref())==attr {
            let v=std::str::from_utf8(a.value.as_ref())
                .map_err(|_|OfficeRevisionError::InvalidPackage)?;
            return Ok(Some(unescape(v).map_err(|_|OfficeRevisionError::InvalidPackage)?.into_owned()))
        }
    }
    Ok(None)
}
fn xml_text(text:&str,tag:&[u8],index:usize,expected:&str,new:&str)
    ->Result<Vec<u8>,OfficeRevisionError>{
    if !text_ok(expected)||!text_ok(new)||index>=10_000{
        return Err(OfficeRevisionError::InvalidContent)
    }
    let mut reader=Reader::from_str(text);
    let mut count=0usize;
    let mut at=None;
    let mut events=0usize;
    loop{
        events+=1;
        if events>200_000{return Err(OfficeRevisionError::TooLarge)}
        let event=reader.read_event().map_err(|_|OfficeRevisionError::InvalidPackage)?;
        match event{
            Event::Start(e) if elem_name(e.name().as_ref())==tag=>{
                if count==index{
                    let from=reader.buffer_position() as usize;
                    let span=reader.read_to_end(e.name())
                        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
                    let to=span.end as usize;
                    // A Word/slide text node must not contain nested markup.
                    let inner=text.get(from..to).ok_or(OfficeRevisionError::InvalidPackage)?;
                    if inner.contains('<'){return Err(OfficeRevisionError::UnsupportedPackage)}
                    let previous=unescape(inner)
                        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
                    if previous!=expected{return Err(OfficeRevisionError::SourceChanged)}
                    if at.replace((from,to)).is_some(){
                        return Err(OfficeRevisionError::AmbiguousTarget)
                    }
                }
                count+=1;
            },
            Event::DocType(_)=>return Err(OfficeRevisionError::UnsafePackage),
            Event::Eof=>break,
            _=>{},
        }
    }
    let (from,to)=at.ok_or(OfficeRevisionError::NotFound)?;
    let mut updated=Vec::with_capacity(text.len()+new.len());
    updated.extend_from_slice(text[..from].as_bytes());
    updated.extend_from_slice(escape(new).as_bytes());
    updated.extend_from_slice(text[to..].as_bytes());
    Ok(updated)
}
fn xml_cell(text:&str,address:&str,expected:&str,new:&str,kind:&str)
    ->Result<Vec<u8>,OfficeRevisionError>{
    if address.is_empty()||address.len()>8||
        !address.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit())
        ||!address.bytes().any(|b|b.is_ascii_digit()) {
        return Err(OfficeRevisionError::InvalidTarget)
    }
    let mut reader=Reader::from_str(text);
    let mut found=None;
    let mut count=0usize;
    loop{
        count+=1;
        if count>200_000{return Err(OfficeRevisionError::TooLarge)}
        match reader.read_event().map_err(|_|OfficeRevisionError::InvalidPackage)? {
            Event::Start(e) if elem_name(e.name().as_ref())==b"c"=>{
                let id=value_attr(&e,b"r")?.unwrap_or_default();
                if id==address{
                    if found.is_some(){return Err(OfficeRevisionError::AmbiguousTarget)}
                    let typ=value_attr(&e,b"t")?.unwrap_or_else(||"n".into());
                    if typ!=kind{return Err(OfficeRevisionError::UnsupportedCell)}
                    let start=reader.buffer_position() as usize;
                    let span=reader.read_to_end(e.name())
                        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
                    let end=span.end as usize;
                    let inner=text.get(start..end).ok_or(OfficeRevisionError::InvalidPackage)?;
                    // Only a plain value may change. Never alter a formula cell,
                    // shared string table, extension, rich text, or hyperlink.
                    if inner.contains("<f")||inner.contains(":f")||inner.contains("<is")
                        ||inner.contains("<ext") {
                        return Err(OfficeRevisionError::UnsupportedCell)
                    }
                    let open=inner.find("<v>").ok_or(OfficeRevisionError::UnsupportedCell)?;
                    let close=inner.find("</v>").ok_or(OfficeRevisionError::UnsupportedCell)?;
                    if close<=open||inner[close+4..].contains("<v>")
                        ||inner[open+3..close].contains('<'){
                        return Err(OfficeRevisionError::UnsupportedCell)
                    }
                    let actual=unescape(&inner[open+3..close])
                        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
                    if actual!=expected{return Err(OfficeRevisionError::SourceChanged)}
                    found=Some((start+open+3,start+close));
                }
            },
            Event::DocType(_)=>return Err(OfficeRevisionError::UnsafePackage),
            Event::Eof=>break,
            _=>{},
        }
    }
    let (start,end)=found.ok_or(OfficeRevisionError::NotFound)?;
    let mut output=Vec::new();
    output.extend_from_slice(text[..start].as_bytes());
    output.extend_from_slice(new.as_bytes());
    output.extend_from_slice(text[end..].as_bytes());
    Ok(output)
}
/// Only single-run inlineStr worksheet text is editable. SharedStrings,
/// rich-text runs, hyperlinks, formulas and cached values are NOT rewritten.
fn xml_inline_cell(
    text:&str,address:&str,expected:&str,new:&str
)->Result<Vec<u8>,OfficeRevisionError>{
    if address.is_empty()||address.len()>8||
        !address.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit())||
        !address.bytes().any(|b|b.is_ascii_digit())||
        !text_ok(expected)||!text_ok(new){
        return Err(OfficeRevisionError::InvalidTarget)
    }
    let mut reader=Reader::from_str(text);
    let mut changed=None;
    let mut events=0usize;
    loop{
        events+=1;
        if events>200_000{return Err(OfficeRevisionError::TooLarge)}
        match reader.read_event().map_err(|_|OfficeRevisionError::InvalidPackage)?{
            Event::Start(ref e) if elem_name(e.name().as_ref())==b"c"=>{
                if value_attr(e,b"r")?.as_deref()==Some(address){
                    if changed.is_some(){return Err(OfficeRevisionError::AmbiguousTarget)}
                    if value_attr(e,b"t")?.as_deref()!=Some("inlineStr"){
                        return Err(OfficeRevisionError::UnsupportedCell)
                    }
                    let from=reader.buffer_position() as usize;
                    let span=reader.read_to_end(e.name())
                        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
                    let to=span.end as usize;
                    let inner=text.get(from..to).ok_or(OfficeRevisionError::InvalidPackage)?;
                    // Refuse rich/phonetic markup, more than one text run and
                    // any calculation/external extension metadata.
                    if inner.contains("<f")||inner.contains(":f")||
                        inner.contains("<r>")||inner.contains("<r ")||
                        inner.contains("<rPh")||inner.contains("<ext")||
                        !inner.contains("<is>")||!inner.contains("</is>"){
                        return Err(OfficeRevisionError::UnsupportedCell)
                    }
                    let rendered=xml_text(inner,b"t",0,expected,new)?;
                    // Only one t run is allowed, not part of a rich string.
                    let runs=inner.matches("<t").count()+inner.matches(":t").count();
                    if runs!=1{return Err(OfficeRevisionError::UnsupportedCell)}
                    changed=Some((from,to,rendered));
                }
            },
            Event::DocType(_)=>return Err(OfficeRevisionError::UnsafePackage),
            Event::Eof=>break,
            _=>{},
        }
    }
    let (from,to,rendered)=changed.ok_or(OfficeRevisionError::NotFound)?;
    let mut out=Vec::new();
    out.extend_from_slice(text[..from].as_bytes());
    out.extend_from_slice(&rendered);
    out.extend_from_slice(text[to..].as_bytes());
    Ok(out)
}
fn target_path(edit:&OfficeRevisionEdit)->Result<String,OfficeRevisionError>{
    match edit{
        OfficeRevisionEdit::WordRun{..}=>Ok("word/document.xml".into()),
        OfficeRevisionEdit::SlideRun{slide_number,..} if (1..=256).contains(slide_number)
            =>Ok(format!("ppt/slides/slide{slide_number}.xml")),
        OfficeRevisionEdit::CellNumber{sheet_number,..}
        |OfficeRevisionEdit::CellBoolean{sheet_number,..}
        |OfficeRevisionEdit::CellInlineText{sheet_number,..}
            if (1..=256).contains(sheet_number)
            =>Ok(format!("xl/worksheets/sheet{sheet_number}.xml")),
        _=>Err(OfficeRevisionError::InvalidTarget),
    }
}
fn edit_part(edit:&OfficeRevisionEdit,content:&[u8])
    ->Result<Vec<u8>,OfficeRevisionError>{
    let source=std::str::from_utf8(content).map_err(|_|OfficeRevisionError::InvalidPackage)?;
    match edit{
        OfficeRevisionEdit::WordRun{text_index,expected_old,new_text}=>{
            xml_text(source,b"t",*text_index,expected_old,new_text)
        },
        OfficeRevisionEdit::SlideRun{text_index,expected_old,new_text,..}=>{
            xml_text(source,b"t",*text_index,expected_old,new_text)
        },
        OfficeRevisionEdit::CellNumber{address,expected_old,new_number,..}=>{
            if !expected_old.is_finite()||!new_number.is_finite(){
                return Err(OfficeRevisionError::InvalidContent)
            }
            xml_cell(source,address,&expected_old.to_string(),&new_number.to_string(),"n")
        },
        OfficeRevisionEdit::CellBoolean{address,expected_old,new_boolean,..}=>{
            xml_cell(source,address,if *expected_old{"1"}else{"0"},
                if *new_boolean{"1"}else{"0"},"b")
        },
        OfficeRevisionEdit::CellInlineText{address,expected_old,new_text,..}=>{
            xml_inline_cell(source,address,expected_old,new_text)
        },
    }
}
/// Preview one new revision of an existing OOXML package. Refuse all package
/// signatures and embedded macros; do not pretend rewriting preserves signed
/// artifacts or full OOXML metadata. Requires an explicit versioned output.
pub fn render_office_revision(
    original:&[u8],original_sha256:&str,edit:&OfficeRevisionEdit,
)->Result<OfficeRevisionPreview,OfficeRevisionError>{
    if original.len()>MAX_PACKAGE{return Err(OfficeRevisionError::TooLarge)}
    if hash(original)!=original_sha256{return Err(OfficeRevisionError::SourceChanged)}
    let target=target_path(edit)?;
    let mut zip=ZipArchive::new(Cursor::new(original))
        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
    if zip.len()>MAX_PARTS{return Err(OfficeRevisionError::TooLarge)}
    if zip.has_overlapping_entries().map_err(|_|OfficeRevisionError::InvalidPackage)?
        ||zip.validate_header_consistency().is_err(){
        return Err(OfficeRevisionError::InvalidPackage)
    }
    if !zip.comment().is_empty(){return Err(OfficeRevisionError::UnsupportedPackage)}
    let mut out=ZipWriter::new(Cursor::new(Vec::new()));
    let mut seen=BTreeSet::new();
    let mut count=0usize;
    let mut total=0usize;
    let mut unchanged=Vec::new();
    let mut changed=false;
    for idx in 0..zip.len(){
        let mut part=zip.by_index(idx).map_err(|_|OfficeRevisionError::InvalidPackage)?;
        let name=part.name().map_err(|_|OfficeRevisionError::InvalidPackage)?.into_owned();
        if !safe_part(&name)||!seen.insert(name.to_lowercase())
            ||part.is_symlink()||part.encrypted() {
            return Err(OfficeRevisionError::UnsafePackage)
        }
        let name_lower=name.to_lowercase();
        if name_lower.contains("vbaproject")||name_lower.contains("_xmlsignatures")
            ||name_lower.contains("externallinks")||name_lower.ends_with(".bin")
            ||name_lower=="encryptioninfo" {
            return Err(OfficeRevisionError::UnsupportedPackage)
        }
        if !matches!(part.compression(),CompressionMethod::Stored|CompressionMethod::Deflated)
            ||part.size() as usize>MAX_PART
            ||(part.compressed_size()>0&&part.size()/part.compressed_size()>1000){
            return Err(OfficeRevisionError::UnsupportedPackage)
        }
        total=total.checked_add(part.size() as usize)
            .ok_or(OfficeRevisionError::TooLarge)?;
        if total>MAX_PACKAGE*4{return Err(OfficeRevisionError::TooLarge)}
        let mut bytes=Vec::new();
        (&mut part).take(MAX_PART as u64+1).read_to_end(&mut bytes)
            .map_err(|_|OfficeRevisionError::InvalidPackage)?;
        if bytes.len()>MAX_PART{return Err(OfficeRevisionError::TooLarge)}
        if name==target {
            if changed{return Err(OfficeRevisionError::AmbiguousTarget)}
            bytes=edit_part(edit,&bytes)?;
            changed=true;
        }else{
            unchanged.push((name.clone(),hash(&bytes)));
        }
        if part.is_dir(){
            out.add_directory(&name,SimpleFileOptions::default())
                .map_err(|e|OfficeRevisionError::Archive(e.to_string()))?;
        }else{
            out.start_file(&name,SimpleFileOptions::default()
                .compression_method(CompressionMethod::Stored))
                .map_err(|e|OfficeRevisionError::Archive(e.to_string()))?;
            out.write_all(&bytes)
                .map_err(|e|OfficeRevisionError::Archive(e.to_string()))?;
        }
        count+=1;
        if count>MAX_PARTS{return Err(OfficeRevisionError::TooLarge)}
    }
    if !changed{return Err(OfficeRevisionError::NotFound)}
    let revised=out.finish().map_err(|e|OfficeRevisionError::Archive(e.to_string()))?.into_inner();
    if revised.len()>MAX_PACKAGE{return Err(OfficeRevisionError::TooLarge)}
    Ok(OfficeRevisionPreview{
        original_sha256:original_sha256.into(),revised_sha256:hash(&revised),
        revised_bytes:revised,target_part:target,unaffected_parts:unchanged,
    })
}
/// Host-owned destination/new revision. Exact original and revised hashes are
/// verified, while the original remains untouched for human review/rollback.
pub fn publish_office_revision_new(
    source_dir:&Dir,source_name:&str,output_dir:&Dir,new_name:&str,
    stage_id:&str,original_sha256:&str,approved_revised_sha256:&str,
    edit:&OfficeRevisionEdit,
)->Result<WriteReceipt,OfficeRevisionError>{
    if !basename(source_name)||!basename(new_name)||source_name==new_name
        || ![".docx",".pptx",".xlsx"].iter().any(|ext|
            source_name.to_ascii_lowercase().ends_with(ext) &&
            new_name.to_ascii_lowercase().ends_with(ext)) {
        return Err(OfficeRevisionError::InvalidName)
    }
    let meta=source_dir.symlink_metadata(source_name)
        .map_err(|_|OfficeRevisionError::NotFound)?;
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>MAX_PACKAGE as u64{
        return Err(OfficeRevisionError::UnsafePackage)
    }
    let mut file=source_dir.open(source_name)
        .map_err(|_|OfficeRevisionError::NotFound)?;
    let mut original=Vec::new();
    (&mut file).take(MAX_PACKAGE as u64+1).read_to_end(&mut original)
        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
    let revised=render_office_revision(&original,original_sha256,edit)?;
    if revised.revised_sha256!=approved_revised_sha256{
        return Err(OfficeRevisionError::SourceChanged)
    }
    create_new_atomic_in_dir(output_dir,new_name,stage_id,&revised.revised_bytes)
        .map_err(OfficeRevisionError::Publish)
}

/// Deterministic ordered, bounded multi-change revision. All changes are
/// applied in memory, with exact source preconditions checked before ANY
/// publication. Each stage is a complete validated OOXML repack; a failure
/// discards the pending in-memory bytes and leaves the original untouched.
/// The final exact SHA-256 binds the entire resulting file to human approval.
pub fn render_office_revision_batch(
    original:&[u8],original_sha256:&str,edits:&[OfficeRevisionEdit],
)->Result<OfficeRevisionPreview,OfficeRevisionError>{
    if edits.is_empty()||edits.len()>32{
        return Err(OfficeRevisionError::InvalidTarget)
    }
    if hash(original)!=original_sha256{
        return Err(OfficeRevisionError::SourceChanged)
    }
    let mut targets=BTreeSet::new();
    let mut part_paths=BTreeSet::new();
    for edit in edits{
        let part=target_path(edit)?;
        let unique=match edit{
            OfficeRevisionEdit::WordRun{text_index,..}|
            OfficeRevisionEdit::SlideRun{text_index,..}=>format!("{part}:text:{text_index}"),
            OfficeRevisionEdit::CellNumber{address,..}|
            OfficeRevisionEdit::CellBoolean{address,..}|
            OfficeRevisionEdit::CellInlineText{address,..}=>format!("{part}:cell:{address}"),
        };
        if !targets.insert(unique){return Err(OfficeRevisionError::AmbiguousTarget)}
        part_paths.insert(part);
    }
    let mut bytes=original.to_vec();
    let mut prior_hash=original_sha256.to_owned();
    for edit in edits{
        let step=render_office_revision(&bytes,&prior_hash,edit)?;
        bytes=step.revised_bytes;
        prior_hash=step.revised_sha256;
    }
    // Unaffected parts are defined relative to original input, not last edit.
    // They retain identical uncompressed content across the complete batch.
    let mut archive=ZipArchive::new(Cursor::new(original))
        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
    let mut unaffected=Vec::new();
    for idx in 0..archive.len(){
        let mut part=archive.by_index(idx)
            .map_err(|_|OfficeRevisionError::InvalidPackage)?;
        let name=part.name().map_err(|_|OfficeRevisionError::InvalidPackage)?.into_owned();
        if part_paths.contains(&name){continue}
        let mut data=Vec::new();
        (&mut part).take(MAX_PART as u64+1).read_to_end(&mut data)
            .map_err(|_|OfficeRevisionError::InvalidPackage)?;
        if data.len()>MAX_PART{return Err(OfficeRevisionError::TooLarge)}
        unaffected.push((name,hash(&data)));
    }
    Ok(OfficeRevisionPreview{
        original_sha256:original_sha256.into(),revised_sha256:prior_hash,
        revised_bytes:bytes,
        target_part:format!("{} targeted parts; {} changes",part_paths.len(),edits.len()),
        unaffected_parts:unaffected,
    })
}
/// Save a batch only as a host-named NEW file. Exact source and final package
/// SHA are checked again under pinned source/destination Dir capabilities.
pub fn publish_office_revision_batch_new(
    source_dir:&Dir,source_name:&str,output_dir:&Dir,new_name:&str,
    stage_id:&str,original_sha256:&str,approved_revised_sha256:&str,
    edits:&[OfficeRevisionEdit],
)->Result<WriteReceipt,OfficeRevisionError>{
    if !basename(source_name)||!basename(new_name)||source_name==new_name
        ||![".docx",".pptx",".xlsx"].iter().any(|ext|
            source_name.to_ascii_lowercase().ends_with(ext) &&
            new_name.to_ascii_lowercase().ends_with(ext)){
        return Err(OfficeRevisionError::InvalidName)
    }
    let meta=source_dir.symlink_metadata(source_name)
        .map_err(|_|OfficeRevisionError::NotFound)?;
    if !meta.is_file()||meta.file_type().is_symlink()
        ||meta.len()>MAX_PACKAGE as u64{
        return Err(OfficeRevisionError::UnsafePackage)
    }
    #[cfg(windows)]{
        use cap_std::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err(OfficeRevisionError::UnsafePackage)
        }
    }
    let mut file=source_dir.open(source_name)
        .map_err(|_|OfficeRevisionError::NotFound)?;
    let mut original=Vec::new();
    (&mut file).take(MAX_PACKAGE as u64+1).read_to_end(&mut original)
        .map_err(|_|OfficeRevisionError::InvalidPackage)?;
    let revised=render_office_revision_batch(&original,original_sha256,edits)?;
    if revised.revised_sha256!=approved_revised_sha256{
        return Err(OfficeRevisionError::SourceChanged)
    }
    create_new_atomic_in_dir(output_dir,new_name,stage_id,&revised.revised_bytes)
        .map_err(OfficeRevisionError::Publish)
}
