//! Bounded, read-only RTF 1.x plain-text adapter.
//! Never instantiates OLE, links, scripts, fonts or external resources.
//! Character encoding support is deliberately limited to Windows-1252 and
//! signed UTF-16 RTF \u escapes. Rich layout is not represented.
use crate::native_reader::{ReaderFailure,ReaderFormat,ReaderText,MAX_READER_OUTPUT_CHARS};

const MAX_STEPS:usize=1_500_000;
const MAX_DEPTH:usize=96;
const MAX_INPUT:usize=8*1024*1024;
#[derive(Clone,Copy)]
struct State{skip:bool,uc:usize}
fn bad()->ReaderFailure{ReaderFailure::InvalidDocument}
fn cp1252(byte:u8)->Result<char,ReaderFailure>{
    if byte==0{return Err(ReaderFailure::UnsupportedEncoding)}
    let code=if (0x80..=0x9f).contains(&byte){
        const CODES:[u16;32]=[
            0x20ac,0x0000,0x201a,0x0192,0x201e,0x2026,0x2020,0x2021,
            0x02c6,0x2030,0x0160,0x2039,0x0152,0x0000,0x017d,0x0000,
            0x0000,0x2018,0x2019,0x201c,0x201d,0x2022,0x2013,0x2014,
            0x02dc,0x2122,0x0161,0x203a,0x0153,0x0000,0x017e,0x0178,
        ];
        let mapped=CODES[(byte-0x80) as usize];
        if mapped==0{return Err(ReaderFailure::UnsupportedEncoding)}
        u32::from(mapped)
    }else{u32::from(byte)};
    char::from_u32(code).ok_or(ReaderFailure::UnsupportedEncoding)
}
fn destination(word:&[u8])->bool{
    [
        b"fonttbl".as_slice(),b"colortbl",b"stylesheet",b"info",b"pict",
        b"object",b"header",b"footer",b"footnote",b"fldinst",
        b"listtable",b"listoverridetable",b"themedata",b"datastore",
        b"xmlnstbl",b"generator",b"filetbl",b"revtbl"
    ].iter().any(|item|*item==word)
}
fn append(out:&mut String,ch:char)->Result<(),ReaderFailure>{
    out.push(ch);
    if out.len()>MAX_READER_OUTPUT_CHARS*4{return Err(ReaderFailure::TooLarge)}
    Ok(())
}
pub fn read_rtf(bytes:&[u8],max_chars:usize)->Result<ReaderText,ReaderFailure>{
    if bytes.len()<7||bytes.len()>MAX_INPUT
        ||!bytes.starts_with(b"{\\rtf")
        ||max_chars==0||max_chars>MAX_READER_OUTPUT_CHARS{
        return Err(bad())
    }
    let mut out=String::new();
    let mut stack=Vec::<State>::new();
    let mut state=State{skip:false,uc:1};
    let mut fallback=0usize;
    let mut surrogate=None::<u16>;
    let mut pos=0usize;let mut steps=0usize;let mut root_closed=false;
    while pos<bytes.len(){
        steps+=1;if steps>MAX_STEPS{return Err(ReaderFailure::TooLarge)}
        let ch=bytes[pos];
        if root_closed{
            if !ch.is_ascii_whitespace(){return Err(bad())}
            pos+=1;continue;
        }
        match ch{
            b'{'=>{
                if stack.len()>=MAX_DEPTH{return Err(ReaderFailure::TooLarge)}
                stack.push(state);pos+=1;
            },
            b'}'=>{
                state=stack.pop().ok_or_else(bad)?;
                pos+=1;
                if stack.is_empty(){root_closed=true;}
            },
            b'\\'=>{
                pos+=1;if pos>=bytes.len(){return Err(bad())}
                let first=bytes[pos];
                if first==b'\''{
                    let hex=bytes.get(pos+1..pos+3).ok_or_else(bad)?;
                    let hex=std::str::from_utf8(hex).map_err(|_|bad())?;
                    let v=u8::from_str_radix(hex,16).map_err(|_|bad())?;
                    pos+=3;
                    if fallback>0{fallback-=1;}
                    else if !state.skip{append(&mut out,cp1252(v)?)?}
                    continue;
                }
                if !first.is_ascii_alphabetic(){
                    pos+=1;
                    match first{
                        b'*'=>state.skip=true,
                        b'\\'|b'{'|b'}'|b'~'|b'_'|b'-'=>{
                            if fallback>0{fallback-=1;}
                            else if !state.skip{
                                let c=match first{
                                    b'~'=>'\u{a0}',b'_'=>'\u{2011}',b'-'=>'\u{00ad}',
                                    x=>x as char
                                };
                                append(&mut out,c)?;
                            }
                        },
                        b'\n'|b'\r'=>{},
                        _=>{}
                    }
                    continue;
                }
                let word_start=pos;
                while pos<bytes.len()&&bytes[pos].is_ascii_alphabetic(){
                    pos+=1;if pos-word_start>40{return Err(bad())}
                }
                let word=&bytes[word_start..pos];
                let number_start=pos;
                if bytes.get(pos).is_some_and(|c|*c==b'-'){pos+=1;}
                while pos<bytes.len()&&bytes[pos].is_ascii_digit(){pos+=1;}
                let arg=if pos>number_start{
                    let raw=std::str::from_utf8(&bytes[number_start..pos]).map_err(|_|bad())?;
                    Some(raw.parse::<i32>().map_err(|_|bad())?)
                }else{None};
                if bytes.get(pos)==Some(&b' '){pos+=1;}
                if destination(word){state.skip=true;continue}
                if word==b"bin"{
                    let length=arg.ok_or_else(bad)?;
                    if length<0||length>1_048_576{return Err(ReaderFailure::TooLarge)}
                    pos=pos.checked_add(length as usize).filter(|n|*n<=bytes.len())
                        .ok_or_else(bad)?;
                    continue;
                }
                if word==b"uc"{
                    let n=arg.ok_or_else(bad)?;
                    if n<0||n>16{return Err(bad())}
                    state.uc=n as usize;
                    continue;
                }
                if word==b"ansicpg"{
                    if arg!=Some(1252){return Err(ReaderFailure::UnsupportedEncoding)}
                }
                if state.skip{continue;}
                if word==b"u"{
                    let n=arg.ok_or_else(bad)?;
                    if !(-32768..=65535).contains(&n){return Err(bad())}
                    let unit=(n as i16) as u16;
                    if (0xd800..=0xdbff).contains(&unit){
                        if surrogate.is_some(){return Err(ReaderFailure::UnsupportedEncoding)}
                        surrogate=Some(unit);
                    }else if (0xdc00..=0xdfff).contains(&unit){
                        let high=surrogate.take().ok_or(ReaderFailure::UnsupportedEncoding)?;
                        let code=0x10000+(u32::from(high)-0xd800)*1024+
                            (u32::from(unit)-0xdc00);
                        append(&mut out,char::from_u32(code)
                            .ok_or(ReaderFailure::UnsupportedEncoding)?)?;
                    }else{
                        if surrogate.is_some(){return Err(ReaderFailure::UnsupportedEncoding)}
                        append(&mut out,char::from_u32(u32::from(unit))
                            .ok_or(ReaderFailure::UnsupportedEncoding)?)?;
                    }
                    fallback=state.uc;
                }else if word==b"par"||word==b"line"{append(&mut out,'\n')?}
                else if word==b"tab"{append(&mut out,'\t')?}
                else{
                    let punctuation=[
                        (b"emdash".as_slice(),'—'),(b"endash",'–'),(b"bullet",'•'),
                        (b"lquote",'‘'),(b"rquote",'’'),
                        (b"ldblquote",'“'),(b"rdblquote",'”'),
                    ];
                    if let Some((_,c))=punctuation.iter().find(|(name,_)|*name==word){
                        append(&mut out,*c)?
                    }
                }
            },
            b'\r'|b'\n'=>pos+=1, // RTF formatting whitespace, not paragraph content
            _=>{
                pos+=1;
                if fallback>0{fallback-=1;}
                else if !state.skip{append(&mut out,cp1252(ch)?)?}
            }
        }
    }
    if !root_closed||!stack.is_empty()||surrogate.is_some(){return Err(bad())}
    if out.trim().is_empty(){return Err(ReaderFailure::Empty)}
    let truncated=out.chars().count()>max_chars;
    let text=if truncated{out.chars().take(max_chars).collect()}else{out};
    Ok(ReaderText{format:ReaderFormat::Rtf,text,truncated,extracted_from_image:false})
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn rtf_read_only_text_skips_font_table_and_objects(){
        let bytes=br"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0 Hidden}}Hello \b world\b0\par Euro \'80 {\*\unknown secret} Visible}";
        let output=read_rtf(bytes,500).unwrap();
        assert!(output.text.contains("Hello world\nEuro €"));
        assert!(!output.text.contains("Hidden"));
        assert!(!output.text.contains("secret"));
    }
    #[test]
    fn rtf_respects_unicode_fallback_and_rejects_unclosed_or_unsafe(){
        let output=read_rtf(br"{\rtf1\uc1\u233? cafe}",500).unwrap();
        assert!(output.text.contains("é cafe"));
        assert!(read_rtf(br"{\rtf1{\object forbidden}",500).is_err());
        assert!(read_rtf(br"{\rtf1\ansicpg65001 Hello}",500).is_err());
        assert_eq!(read_rtf(br"not rtf",500),Err(ReaderFailure::InvalidDocument));
    }
}
