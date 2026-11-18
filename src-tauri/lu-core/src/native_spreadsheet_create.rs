//! Structured, host-owned spreadsheet creation for Lu Write.
//! Creates NEW CSV/XLSX only; no overwrite, no model-facing capability,
//! and no implicit authorization. The desktop must first verify the exact
//! persisted human request, target-specific grant, and durable effect journal.
//! Never interpret document data as a filesystem path or permission.
use std::{collections::{BTreeMap, HashMap}, io::{Cursor, Write}};
use cap_std::fs::Dir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};
use crate::native_atomic_write::{create_new_atomic_in_dir, AtomicWriteError, WriteReceipt};

const MAX_ROWS:u32=10_000;
const MAX_COLS:u16=256;
const MAX_CELLS:usize=25_000;
const MAX_TEXT:usize=32_768;
const MAX_XML_BYTES:usize=4*1024*1024;
#[derive(Debug,Clone,PartialEq)]
pub enum CellValue {
    Text(String),
    Number(f64),
    Boolean(bool),
    /// Explicit trusted formula only. No cached calculated result is fabricated.
    Formula(String),
}
#[derive(Debug,Clone,PartialEq)]
pub struct SpreadsheetCell { pub row:u32, pub column:u16, pub value:CellValue }
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum SpreadsheetWriteError {
    InvalidSheetName, InvalidCell, DuplicateCell, TooManyCells,
    InvalidText, InvalidNumber, InvalidFormula, TooLarge,
    Archive(String), Atomic(AtomicWriteError),
}
fn text_valid(value:&str)->bool {
    value.len()<=MAX_TEXT && value.chars().all(|c|
        c=='\n' || c=='\t' || c=='\r' || (!c.is_control() && c!='\u{FFFE}' && c!='\u{FFFF}'))
}
fn xml(s:&str)->String{
    let mut out=String::with_capacity(s.len());
    for ch in s.chars(){
        match ch {
            '&'=>out.push_str("&amp;"), '<'=>out.push_str("&lt;"),
            '>'=>out.push_str("&gt;"), '"'=>out.push_str("&quot;"),
            '\''=>out.push_str("&apos;"), _=>out.push(ch),
        }
    }
    out
}
fn check_formula(s:&str)->bool {
    // Prevent external workbook links, DDE, injected XML/control chars and
    // ambiguous model-submitted formulas. Full formula language is deferred.
    !s.is_empty() && s.len()<=512 && !s.starts_with('=')
        && s.chars().all(|c|c.is_ascii_alphanumeric() ||
            matches!(c,'+'|'-'|'*'|'/'|'(' | ')' | ':' | ',' | '.' | '$' | '_' | ' '))
        && !s.to_ascii_uppercase().contains("WEBSERVICE")
        && !s.to_ascii_uppercase().contains("HYPERLINK")
}
fn validate(cells:&[SpreadsheetCell])->Result<BTreeMap<(u32,u16),&CellValue>,SpreadsheetWriteError>{
    if cells.len()>MAX_CELLS{return Err(SpreadsheetWriteError::TooManyCells)}
    let mut map=BTreeMap::new();
    for c in cells{
        if !(1..=MAX_ROWS).contains(&c.row) || !(1..=MAX_COLS).contains(&c.column){
            return Err(SpreadsheetWriteError::InvalidCell)
        }
        match &c.value{
            CellValue::Text(t) if !text_valid(t)=>return Err(SpreadsheetWriteError::InvalidText),
            CellValue::Number(n) if !n.is_finite()=>return Err(SpreadsheetWriteError::InvalidNumber),
            CellValue::Formula(f) if !check_formula(f)=>return Err(SpreadsheetWriteError::InvalidFormula),
            _=>{}
        }
        if map.insert((c.row,c.column),&c.value).is_some(){
            return Err(SpreadsheetWriteError::DuplicateCell)
        }
    }
    Ok(map)
}
fn col_name(mut column:u16)->String{
    let mut s=String::new();
    while column>0{
        column-=1;
        s.insert(0,char::from(b'A'+(column%26) as u8));
        column/=26;
    }
    s
}
fn name_valid(name:&str)->bool{
    !name.is_empty() && name.chars().count()<=31 && text_valid(name)
        && !name.starts_with('\'') && !name.ends_with('\'')
        && !name.chars().any(|c|matches!(c,':'|'\\'|'/'|'?'|'*'|'['|']'))
}
fn csv_cell(value:&CellValue)->String {
    let raw=match value{
        CellValue::Text(t)=> {
            // A CSV has no trusted typed formula channel. Block spreadsheet
            // formula injection by explicitly forcing suspicious text literal.
            let first=t.trim_start_matches(|ch:char| matches!(ch,' '| '\t' | '\r' | '\n'));
            if matches!(first.chars().next(),Some('='|'+'|'-'|'@')) {format!("'{t}")} else {t.clone()}
        },
        CellValue::Number(n)=>n.to_string(),
        CellValue::Boolean(b)=>if *b{"TRUE".into()}else{"FALSE".into()},
        CellValue::Formula(f)=>format!("'={f}"), // csv is always data-only
    };
    format!("\"{}\"",raw.replace('"',"\"\""))
}
/// Generate bounded CSV, with every field quoted and formula-looking text
/// escaped so opening the CSV cannot silently execute spreadsheet formulas.
pub fn render_csv(cells:&[SpreadsheetCell])->Result<Vec<u8>,SpreadsheetWriteError>{
    let sorted=validate(cells)?;
    let max_row=sorted.keys().map(|(r,_)|*r).max().unwrap_or(0);
    let max_col=sorted.keys().map(|(_,c)|*c).max().unwrap_or(0);
    let mut output=Vec::new();
    // A sparse cell at (10000,256) would cause excessive empty CSV output.
    if u64::from(max_row)*u64::from(max_col)>MAX_CELLS as u64 {
        return Err(SpreadsheetWriteError::TooLarge)
    }
    for row in 1..=max_row{
        for col in 1..=max_col{
            if col>1{output.push(b',')}
            if let Some(v)=sorted.get(&(row,col)){
                output.extend_from_slice(csv_cell(v).as_bytes());
            }else{
                output.extend_from_slice(b"\"\"");
            }
            if output.len()>1024*1024{return Err(SpreadsheetWriteError::TooLarge)}
        }
        output.extend_from_slice(b"\r\n");
    }
    Ok(output)
}
fn package_part<W:Write+std::io::Seek>(
    writer:&mut ZipWriter<W>,path:&str,body:&str
)->Result<(),SpreadsheetWriteError>{
    if body.len()>MAX_XML_BYTES{return Err(SpreadsheetWriteError::TooLarge)}
    writer.start_file(path,SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored))
        .map_err(|e|SpreadsheetWriteError::Archive(e.to_string()))?;
    writer.write_all(body.as_bytes())
        .map_err(|e|SpreadsheetWriteError::Archive(e.to_string()))
}
/// Generate an independent, one-sheet Excel workbook with real typed cells,
/// shared strings and formulas (never evaluated by Lu). Reopening it with
/// Lu Reader must preserve numeric/text identity and sheet/cell anchors.
pub fn render_xlsx(sheet_name:&str,cells:&[SpreadsheetCell])->Result<Vec<u8>,SpreadsheetWriteError>{
    if !name_valid(sheet_name){return Err(SpreadsheetWriteError::InvalidSheetName)}
    let sorted=validate(cells)?;
    let mut shared=Vec::<String>::new();
    let mut indexes=HashMap::<String,usize>::new();
    let mut sheet=String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#);
    let mut current_row=0;
    for ((row,col),value) in sorted{
        if row!=current_row {
            if current_row!=0 {sheet.push_str("</row>")}
            current_row=row;
            sheet.push_str(&format!(r#"<row r="{row}">"#));
        }
        let address=format!("{}{row}",col_name(col));
        match value{
            CellValue::Text(t)=>{
                let index=if let Some(index)=indexes.get(t){*index}else{
                    let id=shared.len(); shared.push(t.clone()); indexes.insert(t.clone(),id);id
                };
                sheet.push_str(&format!(r#"<c r="{address}" t="s"><v>{index}</v></c>"#));
            },
            CellValue::Number(n)=>{
                sheet.push_str(&format!(r#"<c r="{address}" t="n"><v>{n}</v></c>"#));
            },
            CellValue::Boolean(b)=>{
                sheet.push_str(&format!(r#"<c r="{address}" t="b"><v>{}</v></c>"#,if *b{1}else{0}));
            },
            CellValue::Formula(f)=>{
                sheet.push_str(&format!(r#"<c r="{address}"><f>{}</f></c>"#,xml(f)));
            },
        }
        if sheet.len()>MAX_XML_BYTES{return Err(SpreadsheetWriteError::TooLarge)}
    }
    if current_row!=0{sheet.push_str("</row>")}
    sheet.push_str("</sheetData></worksheet>");
    let strings=shared.iter().map(|v|format!("<si><t xml:space=\"preserve\">{}</t></si>",xml(v))).collect::<String>();
    let shared_part=format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="{}" uniqueCount="{}">{strings}</sst>"#, shared.len(),shared.len());
    let workbook=format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="{}" sheetId="1" r:id="rId1"/></sheets><calcPr fullCalcOnLoad="1"/></workbook>"#,xml(sheet_name));
    let types=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/></Types>"#;
    let root_rels=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    let book_rels=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#;
    let mut zip=ZipWriter::new(Cursor::new(Vec::new()));
    for (name,content) in [
        ("[Content_Types].xml",types),
        ("_rels/.rels",root_rels),
        ("xl/workbook.xml",&workbook),
        ("xl/_rels/workbook.xml.rels",book_rels),
        ("xl/worksheets/sheet1.xml",&sheet),
        ("xl/sharedStrings.xml",&shared_part),
    ] {
        package_part(&mut zip,name,content)?;
    }
    let finished=zip.finish().map_err(|e|SpreadsheetWriteError::Archive(e.to_string()))?;
    let bytes=finished.into_inner();
    if bytes.len()>1024*1024{return Err(SpreadsheetWriteError::TooLarge)}
    Ok(bytes)
}
/// Publication only after host grant and durable effect journal admission.
/// Never silently overwrite an existing workbook.
pub fn create_spreadsheet_new_in_dir(
    root:&Dir,target_name:&str,stage_id:&str,sheet_name:&str,
    cells:&[SpreadsheetCell],
)->Result<WriteReceipt,SpreadsheetWriteError>{
    let content=if target_name.to_ascii_lowercase().ends_with(".xlsx") {
        render_xlsx(sheet_name,cells)?
    }else if target_name.to_ascii_lowercase().ends_with(".csv"){
        render_csv(cells)?
    }else{return Err(SpreadsheetWriteError::InvalidSheetName)};
    create_new_atomic_in_dir(root,target_name,stage_id,&content)
        .map_err(SpreadsheetWriteError::Atomic)
}
