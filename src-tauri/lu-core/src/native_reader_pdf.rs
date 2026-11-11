//! Conservative PDF text-layer adapter for classic PDFs.
//! This is not a general PDF renderer. It supports bounded, indirect page
//! content streams, uncompressed or zlib-flated, and basic Tj/TJ text runs.
//! Unsupported encodings, encryption, object streams, excessively deep/cyclic
//! page trees, image-only pages and complex layouts return an explicit error.
//! It never runs embedded JavaScript, launches a renderer or fetches URLs.
use crate::native_reader::{ReaderFailure,ReaderFormat,ReaderText,MAX_READER_OUTPUT_CHARS};
use flate2::read::ZlibDecoder;
use std::{collections::BTreeMap,io::Read};

const MAX_PDF_BYTES:usize=12*1024*1024;
const MAX_OBJECTS:usize=2048;
const MAX_PAGES:usize=128;
const MAX_PAGE_CONTENT:usize=2*1024*1024;
fn bad()->ReaderFailure{ReaderFailure::InvalidDocument}
fn unsupported()->ReaderFailure{ReaderFailure::UnsupportedAdapter(ReaderFormat::Pdf)}
fn is_ws(b:u8)->bool{matches!(b,b' '|b'\n'|b'\r'|b'\t'|0)}
fn boundary(b:u8)->bool{is_ws(b)||matches!(b,b'/'|b'<'|b'>'|b'['|b']'|b'('|b')')}
fn keyword_at(bytes:&[u8],pos:usize,word:&[u8])->bool{
    bytes.get(pos..pos+word.len())==Some(word)&&
    (pos==0||boundary(bytes[pos-1]))&&
    bytes.get(pos+word.len()).is_none_or(|b|boundary(*b))
}
fn find_keyword(bytes:&[u8],word:&[u8])->Option<usize>{
    bytes.windows(word.len()).position(|slice|slice==word)
}
fn skip_ws(b:&[u8],pos:&mut usize){
    while *pos<b.len()&&(is_ws(b[*pos])||b[*pos]==b'\x0c'){*pos+=1}
}
fn integer(b:&[u8],pos:&mut usize)->Option<usize>{
    skip_ws(b,pos);
    let start=*pos;
    while *pos<b.len()&&b[*pos].is_ascii_digit(){*pos+=1}
    if start==*pos{return None}
    std::str::from_utf8(&b[start..*pos]).ok()?.parse().ok()
}
fn value_after<'a>(b:&'a [u8],field:&[u8])->Option<&'a [u8]>{
    for i in 0..b.len().saturating_sub(field.len())+1{
        if keyword_at(b,i,field){return b.get(i+field.len()..)}
    }
    None
}
fn object_ref(b:&[u8],field:&[u8])->Option<usize>{
    let mut p=0;
    let rest=value_after(b,field)?;
    let n=integer(rest,&mut p)?;
    let gen=integer(rest,&mut p)?;
    skip_ws(rest,&mut p);
    if gen!=0||rest.get(p..p+1)!=Some(b"R"){return None}
    Some(n)
}
#[derive(Debug)]
struct PdfObject{
    dictionary:Vec<u8>,
    stream:Option<Vec<u8>>,
}
fn parse_objects(bytes:&[u8])->Result<BTreeMap<usize,PdfObject>,ReaderFailure>{
    if bytes.len()<20||bytes.len()>MAX_PDF_BYTES||!bytes.starts_with(b"%PDF-"){
        return Err(bad())
    }
    if !bytes.windows(5).any(|w|w==b"%%EOF"){return Err(bad())}
    let mut objects=BTreeMap::new();
    let mut at=0;
    while at<bytes.len(){
        if objects.len()>=MAX_OBJECTS{return Err(ReaderFailure::TooLarge)}
        // An indirect object starts at a physical line beginning. This
        // intentionally excludes many valid complex PDFs, but avoids
        // searching binary streams for arbitrary '/Type /Page' strings.
        if at>0&&bytes[at-1]!=b'\n'&&bytes[at-1]!=b'\r'{
            at+=1;continue
        }
        let mut p=at;
        let Some(n)=integer(bytes,&mut p) else{at+=1;continue};
        let Some(gen)=integer(bytes,&mut p) else{at+=1;continue};
        skip_ws(bytes,&mut p);
        if gen!=0||!keyword_at(bytes,p,b"obj"){
            at+=1;continue
        }
        p+=3;
        let end=find_keyword(&bytes[p..],b"endobj")
            .ok_or_else(bad)?+p;
        let body=&bytes[p..end];
        let stream_pos=find_keyword(body,b"stream");
        let (dict,stream)=if let Some(stream_at)=stream_pos{
            let header=&body[..stream_at];
            let length=value_after(header,b"/Length").and_then(|v|{
                let mut i=0;integer(v,&mut i)
            }).ok_or_else(unsupported)?;
            if length>MAX_PAGE_CONTENT{return Err(ReaderFailure::TooLarge)}
            let mut payload=stream_at+6;
            if body.get(payload..payload+2)==Some(b"\r\n"){payload+=2}
            else if body.get(payload)==Some(&b'\n'){payload+=1}
            else{return Err(bad())}
            let raw=body.get(payload..payload.checked_add(length).ok_or_else(bad)?)
                .ok_or_else(bad)?;
            let decoded=if header.windows(12).any(|w|w==b"/FlateDecode"){
                let mut v=Vec::new();
                ZlibDecoder::new(raw).take(MAX_PAGE_CONTENT as u64+1)
                    .read_to_end(&mut v).map_err(|_|bad())?;
                if v.len()>MAX_PAGE_CONTENT{return Err(ReaderFailure::TooLarge)}
                v
            }else if value_after(header,b"/Filter").is_some(){
                return Err(unsupported())
            }else{
                raw.to_vec()
            };
            (header.to_vec(),Some(decoded))
        }else{(body.to_vec(),None)};
        if objects.insert(n,PdfObject{dictionary:dict,stream}).is_some(){
            return Err(bad())
        }
        at=end+6;
    }
    if objects.is_empty(){return Err(bad())}
    Ok(objects)
}
fn parse_page_ids(dict:&[u8])->Result<Vec<usize>,ReaderFailure>{
    let kids=value_after(dict,b"/Kids").ok_or_else(unsupported)?;
    let start=kids.iter().position(|&c|c==b'[').ok_or_else(bad)?;
    let end=kids[start+1..].iter().position(|&c|c==b']')
        .ok_or_else(bad)?+start+1;
    let chunk=&kids[start+1..end];
    let mut pos=0;let mut ids=Vec::new();
    while pos<chunk.len(){
        skip_ws(chunk,&mut pos);
        if pos==chunk.len(){break}
        let id=integer(chunk,&mut pos).ok_or_else(bad)?;
        let generation=integer(chunk,&mut pos).ok_or_else(bad)?;
        skip_ws(chunk,&mut pos);
        if generation!=0||chunk.get(pos)!=Some(&b'R'){return Err(unsupported())}
        pos+=1;ids.push(id);
        if ids.len()>MAX_PAGES{return Err(ReaderFailure::TooLarge)}
    }
    if ids.is_empty(){return Err(unsupported())}
    Ok(ids)
}
// Resolve the page tree from the actual Catalog, not by collecting every
// /Pages object. Ordinary producer PDFs use nested trees. Traversal is
// bounded, checks parent links, rejects duplicate/cyclic nodes and verifies
// declared page counts before extracting any page text.
const MAX_PAGE_TREE_DEPTH:usize=16;
fn pdf_type_is(dict:&[u8],kind:&[u8])->bool{
    let Some(value)=value_after(dict,b"/Type") else{return false};
    let mut pos=0;skip_ws(value,&mut pos);
    value.get(pos..pos+kind.len())==Some(kind)
        &&value.get(pos+kind.len()).is_none_or(|b|boundary(*b))
}
fn collect_page_ids(objects:&BTreeMap<usize,PdfObject>)
    ->Result<Vec<usize>,ReaderFailure>{
    let mut catalogs=objects.iter()
        .filter(|(_,obj)|pdf_type_is(&obj.dictionary,b"/Catalog"));
    let (_,catalog)=catalogs.next().ok_or_else(unsupported)?;
    if catalogs.next().is_some(){return Err(unsupported())}
    let root=object_ref(&catalog.dictionary,b"/Pages").ok_or_else(unsupported)?;
    fn visit(
        id:usize,parent:Option<usize>,depth:usize,
        objects:&BTreeMap<usize,PdfObject>,
        visited:&mut std::collections::BTreeSet<usize>,
        pages:&mut Vec<usize>,
    )->Result<(),ReaderFailure>{
        if depth>MAX_PAGE_TREE_DEPTH||!visited.insert(id){
            return Err(unsupported())
        }
        let node=objects.get(&id).ok_or_else(bad)?;
        if parent.is_some()
            &&object_ref(&node.dictionary,b"/Parent")!=parent{
            return Err(unsupported())
        }
        if pdf_type_is(&node.dictionary,b"/Page"){
            if parent.is_none(){return Err(unsupported())}
            pages.push(id);
            if pages.len()>MAX_PAGES{return Err(ReaderFailure::TooLarge)}
            return Ok(())
        }
        if !pdf_type_is(&node.dictionary,b"/Pages"){
            return Err(unsupported())
        }
        let kids=parse_page_ids(&node.dictionary)?;
        let before=pages.len();
        for child in kids{
            visit(child,Some(id),depth+1,objects,visited,pages)?;
        }
        // A tree's /Count is the total *leaf* count, including nested kids.
        let count=value_after(&node.dictionary,b"/Count")
            .and_then(|rest|{let mut at=0;integer(rest,&mut at)})
            .ok_or_else(unsupported)?;
        if count!=pages.len()-before{return Err(unsupported())}
        Ok(())
    }
    let mut visited=std::collections::BTreeSet::new();
    let mut pages=Vec::new();
    visit(root,None,0,objects,&mut visited,&mut pages)?;
    if pages.is_empty(){return Err(unsupported())}
    Ok(pages)
}

fn parse_literal(b:&[u8],pos:&mut usize)->Result<String,ReaderFailure>{
    if b.get(*pos)!=Some(&b'('){return Err(bad())}
    *pos+=1;
    let mut bytes=Vec::new();let mut nest=1usize;
    while *pos<b.len(){
        let ch=b[*pos];*pos+=1;
        if ch==b'\\'{
            let esc=*b.get(*pos).ok_or_else(bad)?;*pos+=1;
            match esc{
                b'n'=>bytes.push(b'\n'),b'r'=>bytes.push(b'\r'),
                b't'=>bytes.push(b'\t'),b'b'=>bytes.push(8),
                b'f'=>bytes.push(12),
                b'\n'=>{},b'\r'=>{if b.get(*pos)==Some(&b'\n'){*pos+=1}},
                b'0'..=b'7'=>{
                    let mut value=(esc-b'0') as u16;
                    for _ in 0..2{
                        if let Some(d @ b'0'..=b'7')=b.get(*pos).copied(){
                            value=value*8+(d-b'0') as u16;*pos+=1
                        }else{break}
                    }
                    bytes.push(value as u8)
                },
                other=>bytes.push(other),
            }
        }else if ch==b'('{
            nest+=1;if nest>16{return Err(ReaderFailure::TooLarge)}
            bytes.push(ch);
        }else if ch==b')'{
            nest-=1;if nest==0{break}
            bytes.push(ch);
        }else{bytes.push(ch)}
        if bytes.len()>MAX_PAGE_CONTENT{return Err(ReaderFailure::TooLarge)}
    }
    if nest!=0{return Err(bad())}
    decode_pdf_string(&bytes)
}
fn hex_nibble(b:u8)->Option<u8>{
    match b{
        b'0'..=b'9'=>Some(b-b'0'),
        b'a'..=b'f'=>Some(b-b'a'+10),
        b'A'..=b'F'=>Some(b-b'A'+10),
        _=>None,
    }
}
fn parse_hex_string(bytes:&[u8],pos:&mut usize)->Result<String,ReaderFailure>{
    if bytes.get(*pos)!=Some(&b'<') || bytes.get(*pos+1)==Some(&b'<'){
        return Err(unsupported())
    }
    *pos+=1;
    let mut decoded=Vec::new();let mut high=None;
    let mut closed=false;
    while *pos<bytes.len(){
        let b=bytes[*pos];*pos+=1;
        if b==b'>'{closed=true;break}
        if is_ws(b){continue}
        let nibble=hex_nibble(b).ok_or_else(unsupported)?;
        if let Some(upper)=high.take(){
            decoded.push((upper<<4)|nibble);
        }else{high=Some(nibble)}
        if decoded.len()>MAX_PAGE_CONTENT{return Err(ReaderFailure::TooLarge)}
    }
    if !closed{return Err(bad())}
    if let Some(upper)=high{decoded.push(upper<<4)}
    decode_pdf_string(&decoded)
}
fn decode_pdf_string(bytes:&[u8])->Result<String,ReaderFailure>{
    if bytes.starts_with(&[0xfe,0xff]){
        if (bytes.len()-2)%2!=0{return Err(unsupported())}
        let units=bytes[2..].chunks_exact(2)
            .map(|b|u16::from_be_bytes([b[0],b[1]])).collect::<Vec<_>>();
        return String::from_utf16(&units).map_err(|_|unsupported())
    }
    // Some fonts require PDF cmap / ToUnicode; do not invent text.
    if bytes.iter().any(|b|*b>=0x80||*b==0){return Err(unsupported())}
    String::from_utf8(bytes.to_vec()).map_err(|_|unsupported())
}
fn text_stream(bytes:&[u8])->Result<String,ReaderFailure>{
    let mut at=0;let mut inside=false;let mut out=String::new();
    let mut operands=Vec::<String>::new();
    let mut steps=0usize;
    while at<bytes.len(){
        steps+=1;if steps>500_000{return Err(ReaderFailure::TooLarge)}
        skip_ws(bytes,&mut at);
        if at>=bytes.len(){break}
        match bytes[at]{
            b'%'=>{while at<bytes.len()&&bytes[at]!=b'\n'{at+=1}},
            b'('=>{
                let s=parse_literal(bytes,&mut at)?;
                if inside{operands.push(s)}
            },
            b'['|b']'=>{at+=1},
            b'/'=>{
                at+=1;
                while at<bytes.len()&&!boundary(bytes[at]){at+=1}
            },
            b'<' =>{
                let text=parse_hex_string(bytes,&mut at)?;
                if inside{operands.push(text)}
            },
            b @ _ if !boundary(b)=>{
                let start=at;
                while at<bytes.len()&&!boundary(bytes[at]){at+=1}
                let op=&bytes[start..at];
                if op==b"BT"{inside=true;operands.clear()}
                else if op==b"ET"{inside=false;operands.clear();out.push('\n')}
                else if inside && (op==b"Tj"||op==b"TJ"){
                    for string in operands.drain(..){out.push_str(&string)}
                    out.push('\n');
                }else if inside && (op==b"T*"||op==b"Td"||op==b"TD"){
                    out.push('\n');operands.clear();
                }
                if op!=b"Tj"&&op!=b"TJ"&&operands.len()>1000{
                    return Err(ReaderFailure::TooLarge)
                }
            },
            _=>at+=1,
        }
        if out.len()>MAX_READER_OUTPUT_CHARS*4{return Err(ReaderFailure::TooLarge)}
    }
    Ok(out)
}
/// P0-TOOLS-04: independently reconstruct the page tree from original
/// approved PDF bytes. The external PDFInfo stdout is never consulted.
/// Complex/object-stream/encrypted PDFs return UNSUPPORTED, not a guess.
pub fn independent_classic_pdf_page_count(bytes:&[u8])->Option<u64>{
    let objects=parse_objects(bytes).ok()?;
    let leaves=collect_page_ids(&objects).ok()?;
    u64::try_from(leaves.len()).ok()
}
pub fn read_pdf_text(bytes:&[u8],max_chars:usize)->Result<ReaderText,ReaderFailure>{
    read_pdf_page(bytes,max_chars,None)
}
pub fn read_pdf_page(bytes:&[u8],max_chars:usize,selected:Option<usize>)
    ->Result<ReaderText,ReaderFailure>{
    if max_chars==0||max_chars>MAX_READER_OUTPUT_CHARS{
        return Err(ReaderFailure::TooLarge)
    }
    let objects=parse_objects(bytes)?;
    if bytes.windows(8).any(|w|w==b"/Encrypt")||
        bytes.windows(10).any(|w|w==b"/ObjStm"){
        return Err(unsupported())
    }
    let page_ids=collect_page_ids(&objects)?;
    let mut text=String::new();
    let mut found=false;
    for (index,id) in page_ids.iter().enumerate(){
        if selected.is_some_and(|page|page!=index+1){continue}
        found=true;
        let page=objects.get(id).ok_or_else(bad)?;
        if !pdf_type_is(&page.dictionary,b"/Page"){
            return Err(unsupported())
        }
        let stream_id=object_ref(&page.dictionary,b"/Contents")
            .ok_or_else(unsupported)?;
        let content=objects.get(&stream_id)
            .and_then(|o|o.stream.as_ref()).ok_or_else(unsupported)?;
        let page_text=text_stream(content)?;
        if page_text.trim().is_empty(){
            return Err(unsupported()) // scanned pages need OCR
        }
        text.push_str(&format!("[Page {}]\n",index+1));
        text.push_str(&page_text);
        if text.len()>MAX_READER_OUTPUT_CHARS*4{
            return Err(ReaderFailure::TooLarge)
        }
    }
    if !found{return Err(ReaderFailure::SelectorNotFound)}
    let truncated=text.chars().count()>max_chars;
    Ok(ReaderText{format:ReaderFormat::Pdf,
        text:if truncated{text.chars().take(max_chars).collect()}else{text},
        truncated,extracted_from_image:false})
}
#[cfg(test)]
mod tests{
    use super::*;
    fn fixture(content:&[u8],deflate:bool)->Vec<u8>{
        let mut source=Vec::new();
        source.extend_from_slice(b"%PDF-1.4\n");
        source.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        source.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
        source.extend_from_slice(b"3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>\nendobj\n");
        let payload=if deflate{
            use flate2::{write::ZlibEncoder,Compression};
            use std::io::Write;
            let mut z=ZlibEncoder::new(Vec::new(),Compression::default());
            z.write_all(content).unwrap();
            z.finish().unwrap()
        }else{content.to_vec()};
        let head=if deflate{
            format!("4 0 obj\n<< /Length {} /Filter /FlateDecode >>\nstream\n",payload.len())
        }else{format!("4 0 obj\n<< /Length {} >>\nstream\n",payload.len())};
        source.extend_from_slice(head.as_bytes());
        source.extend_from_slice(&payload);
        source.extend_from_slice(b"\nendstream\nendobj\nxref\ntrailer\n<< /Root 1 0 R >>\nstartxref\n0\n%%EOF\n");
        source
    }
    fn nested_fixture()->Vec<u8>{
        let mut source=b"%PDF-1.4\n".to_vec();
        for object in [
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            "2 0 obj\n<< /Type /Pages /Kids [5 0 R 6 0 R] /Count 2 >>\nendobj\n",
            "5 0 obj\n<< /Type /Pages /Parent 2 0 R /Kids [3 0 R] /Count 1 >>\nendobj\n",
            "3 0 obj\n<< /Type /Page /Parent 5 0 R /Contents 4 0 R >>\nendobj\n",
            "6 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 7 0 R >>\nendobj\n",
        ]{source.extend_from_slice(object.as_bytes());}
        for (id,content) in [(4,"BT (Nested first) Tj ET"),
                             (7,"BT (Top-level second) Tj ET")]{
            source.extend_from_slice(format!(
                "{id} 0 obj\n<< /Length {} >>\nstream\n{content}\nendstream\nendobj\n",
                content.len()).as_bytes());
        }
        source.extend_from_slice(b"xref\ntrailer\n<< /Root 1 0 R >>\nstartxref\n0\n%%EOF\n");
        source
    }
    #[test]
    fn nested_page_trees_preserve_catalog_order_and_page_selectors(){
        let input=nested_fixture();
        let read=read_pdf_text(&input,4096).unwrap();
        let first=read.text.find("[Page 1]\nNested first").unwrap();
        let second=read.text.find("[Page 2]\nTop-level second").unwrap();
        assert!(first<second);
        let selected=read_pdf_page(&input,4096,Some(2)).unwrap();
        assert!(selected.text.contains("[Page 2]"));
        assert!(selected.text.contains("Top-level second"));
        assert!(!selected.text.contains("Nested first"));
        assert_eq!(read_pdf_page(&input,4096,Some(3)),
            Err(ReaderFailure::SelectorNotFound));
    }
    #[test]
    fn nested_page_trees_reject_cycles_duplicates_invalid_counts_and_parents(){
        let input=String::from_utf8(nested_fixture()).unwrap();
        for (before,after) in [
            ("/Kids [3 0 R]","/Kids [2 0 R]"), // nested cycle
            ("/Count 2","/Count 3"), // root count mismatch
            ("/Parent 5 0 R","/Parent 999 0 R"), // forged parent
            ("/Kids [5 0 R 6 0 R]","/Kids [5 0 R 5 0 R]"), // same kid twice
        ]{
            assert!(input.contains(before));
            let hostile=input.replacen(before,after,1);
            assert!(read_pdf_text(hostile.as_bytes(),4096).is_err(),
                "accepted invalid page tree: {before}");
        }
    }

    #[test]
    fn reads_page_anchored_pdf_text_and_flated_streams(){
        let plain=b"BT /F1 12 Tf 20 700 Td (Hello \\(PDF\\) world) Tj ET";
        for compressed in [false,true]{
            let read=read_pdf_text(&fixture(plain,compressed),1000).unwrap();
            assert_eq!(read.format,ReaderFormat::Pdf);
            assert!(read.text.contains("[Page 1]"));
            assert!(read.text.contains("Hello (PDF) world"));
        }
    }
    #[test]
    fn pdf_hex_text_operators_use_actual_ascii_or_utf16_bytes(){
        let ascii=b"BT <48656c6c6f20776f726c64> Tj ET";
        let text=read_pdf_text(&fixture(ascii,false),100).unwrap();
        assert!(text.text.contains("Hello world"));
        let utf16=b"BT <FEFF00480069> Tj ET";
        let text=read_pdf_text(&fixture(utf16,true),100).unwrap();
        assert!(text.text.contains("Hi"));
    }
    #[test]
    fn scanned_textless_page_and_binary_glyphs_fail_closed(){
        assert!(matches!(read_pdf_text(&fixture(b"q Q",false),1000),
            Err(ReaderFailure::UnsupportedAdapter(ReaderFormat::Pdf))));
        assert!(matches!(read_pdf_text(&fixture(b"BT (\\200) Tj ET",false),1000),
            Err(ReaderFailure::UnsupportedAdapter(ReaderFormat::Pdf))));
    }
    #[test]
    fn page_selector_refuses_page_outside_catalog(){
        let input=fixture(b"BT (Some page text) Tj ET",false);
        assert_eq!(read_pdf_page(&input,100,Some(2)),
            Err(ReaderFailure::SelectorNotFound));
        assert!(read_pdf_page(&input,100,Some(1)).unwrap().text.contains("[Page 1]"));
    }
    #[test]
    fn rejects_fake_pdf_and_bounded_output(){
        assert!(read_pdf_text(b"%PDF-1.4\n1 0 obj",1000).is_err());
        let read=read_pdf_text(&fixture(b"BT (Long text) Tj ET",false),8).unwrap();
        assert!(read.truncated);
    }
}
