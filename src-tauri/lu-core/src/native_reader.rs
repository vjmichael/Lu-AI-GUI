//! Reader: text normalization and verified adapter dispatch.
//!
//! Host grants and file admission occur before calling this module. It must
//! never execute embedded macros, scripts, or document links. PDF, DOCX and
//! OCR intentionally require their own vetted adapters rather than pretending
//! a UTF-8 decoder can parse them.
use std::io::{self,Read};

pub const MAX_READER_INPUT_BYTES:u64=32*1024*1024;
pub const MAX_READER_OUTPUT_CHARS:usize=200_000;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ReaderFormat{
    PlainText,
    Markdown,
    Csv,
    Json,
    Xml,
    Html,
    SourceCode,
    Pdf,
    Docx,
    Xlsx,
    Pptx,
    Image,
    Unknown,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ReaderFailure{
    UnsupportedAdapter(ReaderFormat),
    UnsupportedEncoding,
    InvalidDocument,
    TooLarge,
    Empty,
    Io,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ReaderText {
    pub format:ReaderFormat,
    pub text:String,
    pub truncated:bool,
    pub extracted_from_image:bool,
}
pub fn classify(filename:&str,header:&[u8])->ReaderFormat{
    // Content signature overrides potentially misleading extensions for
    // container formats. Everything is still checked by its parser.
    if header.starts_with(b"%PDF-"){return ReaderFormat::Pdf;}
    if header.starts_with(b"PK\x03\x04"){
        return match filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str(){
            "docx"=>ReaderFormat::Docx,
            "xlsx"=>ReaderFormat::Xlsx,
            "pptx"=>ReaderFormat::Pptx,
            _=>ReaderFormat::Unknown,
        };
    }
    if header.starts_with(b"\x89PNG\r\n\x1a\n")
        ||header.starts_with(&[0xff,0xd8,0xff]){
        return ReaderFormat::Image;
    }
    let ext=filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str(){
        "txt"|"text"|"log"|"ini"|"cfg"|"conf"|"toml"|"yaml"|"yml"
        |"rst"|"tex"|"properties"|"env"|"gitignore"=>ReaderFormat::PlainText,
        "md"|"markdown"|"mdown"=>ReaderFormat::Markdown,
        "csv"|"tsv"=>ReaderFormat::Csv,
        "json"|"jsonl"|"ndjson"=>ReaderFormat::Json,
        "xml"|"svg"=>ReaderFormat::Xml,
        "html"|"htm"=>ReaderFormat::Html,
        "rs"|"py"|"c"|"h"|"cpp"|"hpp"|"cs"|"js"|"jsx"|"ts"
        |"tsx"|"java"|"go"|"sh"|"ps1"|"sql"|"bat"|"css"
        |"rb"|"php"|"swift"|"kt"|"lua"=>ReaderFormat::SourceCode,
        "pdf"=>ReaderFormat::Pdf,
        "docx"=>ReaderFormat::Docx,
        "xlsx"=>ReaderFormat::Xlsx,
        "pptx"=>ReaderFormat::Pptx,
        "png"|"jpg"|"jpeg"|"webp"|"tif"|"tiff"|"bmp"=>ReaderFormat::Image,
        _=>ReaderFormat::Unknown
    }
}
fn decode_text(bytes:&[u8])->Result<String,ReaderFailure>{
    if bytes.starts_with(&[0xff,0xfe]) || bytes.starts_with(&[0xfe,0xff]){
        let little=bytes.starts_with(&[0xff,0xfe]);
        let data=&bytes[2..];
        if data.len()%2!=0{return Err(ReaderFailure::UnsupportedEncoding);}
        let units=data.chunks_exact(2).map(|b|if little{
            u16::from_le_bytes([b[0],b[1]])
        }else{u16::from_be_bytes([b[0],b[1]])}).collect::<Vec<_>>();
        return String::from_utf16(&units).map_err(|_|ReaderFailure::UnsupportedEncoding);
    }
    let data=bytes.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(bytes);
    String::from_utf8(data.to_vec()).map_err(|_|ReaderFailure::UnsupportedEncoding)
}
/// Only actual text formats may enter this path. HTML/XML are retained as
/// literal source, not stripped or rendered; future structured adapters
/// will handle markup semantics separately.
pub fn read_text_bytes(
    filename:&str,bytes:&[u8],max_chars:usize,
)->Result<ReaderText,ReaderFailure>{
    if bytes.is_empty(){return Err(ReaderFailure::Empty);}
    if bytes.len() as u64>MAX_READER_INPUT_BYTES
       ||max_chars==0||max_chars>MAX_READER_OUTPUT_CHARS{
        return Err(ReaderFailure::TooLarge);
    }
    let format=classify(filename,&bytes[..bytes.len().min(16)]);
    if format==ReaderFormat::Pdf{
        return crate::native_reader_pdf::read_pdf_text(bytes,max_chars);
    }
    if format==ReaderFormat::Docx{
        return crate::native_reader_docx::read_docx_bytes(bytes,max_chars);
    }
    if format==ReaderFormat::Xlsx{
        return crate::native_reader_office::read_xlsx(bytes,max_chars);
    }
    if format==ReaderFormat::Pptx{
        return crate::native_reader_office::read_pptx(bytes,max_chars);
    }
    if matches!(format,ReaderFormat::Image
        |ReaderFormat::Unknown){
        return Err(ReaderFailure::UnsupportedAdapter(format));
    }
    let decoded=decode_text(bytes)?;
    if decoded.chars().any(|c|c=='\0'){
        return Err(ReaderFailure::UnsupportedEncoding);
    }
    let truncated=decoded.chars().count()>max_chars;
    let text=if truncated{decoded.chars().take(max_chars).collect()}else{decoded};
    Ok(ReaderText{format,text,truncated,extracted_from_image:false})
}
/// Bounded host-opened stream intake; no model-supplied OS pathname.
pub fn read_text_stream(
    filename:&str,mut input:impl Read,max_chars:usize,
)->Result<ReaderText,ReaderFailure>{
    let mut bytes=Vec::new();
    input.take(MAX_READER_INPUT_BYTES+1).read_to_end(&mut bytes)
        .map_err(|_|ReaderFailure::Io)?;
    read_text_bytes(filename,&bytes,max_chars)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn reads_utf8_markdown_and_common_source(){
        let a=read_text_bytes("readme.md",b"# Welcome\nHello",50).unwrap();
        assert_eq!(a.format,ReaderFormat::Markdown);
        assert_eq!(a.text,"# Welcome\nHello");
        assert_eq!(read_text_bytes("a.py",b"print(1)",10).unwrap().format,
            ReaderFormat::SourceCode);
    }
    #[test]
    fn reads_utf16_and_reports_output_truncation(){
        let x=read_text_bytes("a.txt",&[0xff,0xfe,0x41,0,0x42,0],1).unwrap();
        assert_eq!(x.text,"A");assert!(x.truncated);
    }
    #[test]
    fn rejects_binary_spoof_and_does_not_fake_pdf_docx_or_ocr(){
        assert_eq!(read_text_bytes("a.txt",b"%PDF-1.7",100),
            Err(ReaderFailure::InvalidDocument));
        assert_eq!(read_text_bytes("a.docx",b"PK\x03\x04bad",100),
            Err(ReaderFailure::InvalidDocument));
        assert_eq!(read_text_bytes("a.png",b"\x89PNG\r\n\x1a\n",100),
            Err(ReaderFailure::UnsupportedAdapter(ReaderFormat::Image)));
        assert_eq!(read_text_bytes("a.txt",&[0,1],100),
            Err(ReaderFailure::UnsupportedEncoding));
    }
    #[test]
    fn bounded_stream_never_reads_arbitrary_binary(){
        let huge=std::io::repeat(b'x').take(MAX_READER_INPUT_BYTES+1);
        assert_eq!(read_text_stream("big.txt",huge,100),
            Err(ReaderFailure::TooLarge));
    }
}
