//! Wave 2: structured existing-CSV cell edit preflight, deliberately NOT a
//! filesystem tool. The returned replacement must be shown as a full diff and
//! committed through WRITE-01E's ReviewedEdit + durable EditExisting path.
use sha2::{Digest,Sha256};
use crate::edit_safety::{plan_text_edit,EditPreflightError,TextEditPlan,MAX_EDIT_DOCUMENT_BYTES};

const MAX_ROWS:usize=10_000;
const MAX_COLS:usize=256;
const MAX_CELLS:usize=25_000;
const MAX_TEXT:usize=32_768;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CsvEditError {
    InvalidName,InvalidEncoding,MalformedCsv,InvalidRange,TooLarge,
    BaselineChanged,InvalidReplacement,Unchanged,EditSafety(EditPreflightError),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct CsvCellChange{
    pub row:usize,pub column:usize,
    pub original_value:String,pub replacement_value:String,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct CsvPreview{
    pub change:CsvCellChange,
    pub original_sha256:String,
    pub replacement_sha256:String,
    pub replacement_bytes:Vec<u8>,
    pub full_diff_review_required:bool,
}
fn digest(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn valid_field(s:&str)->bool{
    s.len()<=MAX_TEXT && !s.contains('\0') &&
        s.chars().all(|c|c=='\r'||c=='\n'||c=='\t'||!c.is_control())
}
fn parse_csv(data:&[u8])->Result<Vec<Vec<String>>,CsvEditError>{
    if data.len()>MAX_EDIT_DOCUMENT_BYTES{return Err(CsvEditError::TooLarge)}
    let text=std::str::from_utf8(data).map_err(|_|CsvEditError::InvalidEncoding)?;
    if text.starts_with('\u{FEFF}') {return Err(CsvEditError::InvalidEncoding)}
    let mut rows=Vec::new();
    let mut row=Vec::new();
    let mut field=String::new();
    let mut quoted=false;
    let mut closed=false;
    let mut beginning=true;
    let mut chars=text.chars().peekable();
    let mut cell_count=0usize;
    while let Some(ch)=chars.next(){
        if quoted{
            if ch=='"'{
                if chars.peek()==Some(&'"'){chars.next();field.push('"')}
                else{quoted=false;closed=true}
            }else{field.push(ch)}
        }else{
            match ch{
                '"' if beginning=>{quoted=true;beginning=false},
                ','=>{
                    if !valid_field(&field){return Err(CsvEditError::TooLarge)}
                    cell_count+=1;
                    if cell_count>MAX_CELLS||row.len()>=MAX_COLS{
                        return Err(CsvEditError::TooLarge)
                    }
                    row.push(std::mem::take(&mut field));
                    closed=false;beginning=true;
                },
                '\r'|'\n'=>{
                    if ch=='\r' && chars.peek()==Some(&'\n'){chars.next();}
                    if !valid_field(&field){return Err(CsvEditError::TooLarge)}
                    cell_count+=1;
                    if cell_count>MAX_CELLS||row.len()>=MAX_COLS||
                        rows.len()>=MAX_ROWS{return Err(CsvEditError::TooLarge)}
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                    closed=false;beginning=true;
                },
                '"'|'\r' if closed=>return Err(CsvEditError::MalformedCsv),
                _ if closed=>return Err(CsvEditError::MalformedCsv),
                _=>{field.push(ch);beginning=false},
            }
        }
        if field.len()>MAX_TEXT{return Err(CsvEditError::TooLarge)}
    }
    if quoted{return Err(CsvEditError::MalformedCsv)}
    if !beginning||!row.is_empty()||!field.is_empty(){
        if !valid_field(&field){return Err(CsvEditError::TooLarge)}
        cell_count+=1;
        if cell_count>MAX_CELLS||row.len()>=MAX_COLS||
            rows.len()>=MAX_ROWS{return Err(CsvEditError::TooLarge)}
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}
fn serialize_csv(rows:&[Vec<String>])->Vec<u8>{
    let mut data=Vec::new();
    for row in rows{
        for (i,field) in row.iter().enumerate(){
            if i>0{data.push(b',')}
            data.push(b'"');
            for ch in field.chars(){
                if ch=='"'{data.push(b'"')}
                let mut buf=[0;4];
                data.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
            data.push(b'"');
        }
        data.extend_from_slice(b"\r\n");
    }
    data
}
/// Create a deterministic full-document replacement for exactly one CSV cell.
/// Existing fields remain semantically unchanged. Formula-like NEW text is
/// made literal for spreadsheet applications, never executed as a formula.
pub fn plan_csv_cell_update(
    name:&str,original:&[u8],expected_sha256:&str,
    row:usize,column:usize,new_value:&str,
)->Result<(CsvPreview,TextEditPlan),CsvEditError>{
    if !name.to_ascii_lowercase().ends_with(".csv")||
        name.is_empty()||name.len()>200||
        name.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':')){
        return Err(CsvEditError::InvalidName)
    }
    if digest(original)!=expected_sha256{return Err(CsvEditError::BaselineChanged)}
    if !valid_field(new_value){return Err(CsvEditError::InvalidReplacement)}
    let mut rows=parse_csv(original)?;
    if row==0||column==0||row>rows.len()||column>rows[row-1].len(){
        return Err(CsvEditError::InvalidRange)
    }
    let existing=rows[row-1][column-1].clone();
    let lead=new_value.trim_start_matches(|c|matches!(c,' '|'\t'|'\r'|'\n'));
    let sanitized=if matches!(lead.chars().next(),Some('='|'+'|'-'|'@')){
        format!("'{new_value}")
    }else{new_value.to_owned()};
    if !valid_field(&sanitized){return Err(CsvEditError::InvalidReplacement)}
    if existing==sanitized{return Err(CsvEditError::Unchanged)}
    rows[row-1][column-1]=sanitized.clone();
    let replacement=serialize_csv(&rows);
    if replacement.len()>MAX_EDIT_DOCUMENT_BYTES{return Err(CsvEditError::TooLarge)}
    let plan=plan_text_edit(name,original,expected_sha256,&replacement)
        .map_err(CsvEditError::EditSafety)?;
    Ok((CsvPreview{
        change:CsvCellChange{
            row,column,original_value:existing,replacement_value:sanitized,
        },
        original_sha256:expected_sha256.into(),
        replacement_sha256:digest(&replacement),
        replacement_bytes:replacement,
        full_diff_review_required:true,
    },plan))
}
