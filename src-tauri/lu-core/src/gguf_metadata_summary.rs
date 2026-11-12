//! P03: bounded, read-only GGUF metadata summary for the local model picker.
//! GGUF strings are UNTRUSTED descriptive bytes. This is NOT a validator of
//! tensor data, tokenizer suitability, architecture support, licensing or GPU.
use std::{collections::BTreeMap,fs::File,io::{self,Read,Seek,SeekFrom},path::Path};
use sha2::{Digest,Sha256};
use crate::gguf_preflight::inspect_gguf_header;
const MAX_KV:u64=32768;
const MAX_METADATA_BYTES:u64=128*1024*1024;
const MAX_ARRAY:u64=1_000_000;

#[derive(Debug,Clone,PartialEq,Eq,Default)]
pub struct GgufMetadataSummary{
    pub architecture:Option<String>,
    pub tokenizer_model:Option<String>,
    pub reported_context_length:Option<u64>,
    pub file_type:Option<u32>,
}
#[derive(Debug)]
pub enum MetadataReadError{
    InvalidGguf,Io(io::Error),InvalidMetadata,
    LimitExceeded,UnsupportedType,
}
impl From<io::Error> for MetadataReadError{
    fn from(e:io::Error)->Self{Self::Io(e)}
}
fn read_u32(f:&mut File)->Result<u32,MetadataReadError>{
    let mut b=[0u8;4];f.read_exact(&mut b)?;Ok(u32::from_le_bytes(b))
}
fn read_u64(f:&mut File)->Result<u64,MetadataReadError>{
    let mut b=[0u8;8];f.read_exact(&mut b)?;Ok(u64::from_le_bytes(b))
}
fn skip(f:&mut File,len:u64,total:u64)->Result<(),MetadataReadError>{
    let position=f.stream_position()?;
    let end=position.checked_add(len).ok_or(MetadataReadError::LimitExceeded)?;
    if end>total||end>MAX_METADATA_BYTES{return Err(MetadataReadError::LimitExceeded)}
    f.seek(SeekFrom::Start(end))?;
    Ok(())
}
fn read_string(f:&mut File,total:u64,max_len:u64)
    ->Result<Option<String>,MetadataReadError>{
    let n=read_u64(f)?;
    if n>max_len{
        skip(f,n,total)?;
        return Ok(None)
    }
    let end=f.stream_position()?.checked_add(n)
        .ok_or(MetadataReadError::LimitExceeded)?;
    if end>total||end>MAX_METADATA_BYTES{
        return Err(MetadataReadError::LimitExceeded)
    }
    let mut b=vec![0u8;n as usize];
    f.read_exact(&mut b)?;
    Ok(String::from_utf8(b).ok())
}
fn fixed_width(kind:u32)->Option<u64>{
    match kind{
        0|1|7=>Some(1),
        2|3=>Some(2),
        4|5|6=>Some(4),
        10|11|12=>Some(8),
        _=>None,
    }
}
fn skip_value(f:&mut File,kind:u32,total:u64)->Result<(),MetadataReadError>{
    if let Some(width)=fixed_width(kind){return skip(f,width,total)}
    match kind{
        8=>{let bytes=read_u64(f)?;skip(f,bytes,total)},
        9=>{
            let element_type=read_u32(f)?;
            let count=read_u64(f)?;
            if count>MAX_ARRAY{return Err(MetadataReadError::LimitExceeded)}
            if let Some(width)=fixed_width(element_type){
                skip(f,count.checked_mul(width)
                    .ok_or(MetadataReadError::LimitExceeded)?,total)
            }else if element_type==8{
                for _ in 0..count{
                    let n=read_u64(f)?;skip(f,n,total)?;
                }
                Ok(())
            }else{Err(MetadataReadError::UnsupportedType)}
        },
        _=>Err(MetadataReadError::UnsupportedType),
    }
}
fn safe_label(text:Option<String>,max_len:usize)->Option<String>{
    text.filter(|s|!s.is_empty()&&s.len()<=max_len&&
        s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.')))
}
pub fn inspect_metadata(path:&Path)->Result<GgufMetadataSummary,MetadataReadError>{
    let h=inspect_gguf_header(path).map_err(|_|MetadataReadError::InvalidGguf)?;
    if h.metadata_count>MAX_KV{return Err(MetadataReadError::LimitExceeded)}
    let mut file=File::open(path)?;
    file.seek(SeekFrom::Start(24))?;
    let mut summary=GgufMetadataSummary::default();
    for _ in 0..h.metadata_count{
        if file.stream_position()?>MAX_METADATA_BYTES{
            return Err(MetadataReadError::LimitExceeded)
        }
        let key=read_string(&mut file,h.file_bytes,256)?
            .ok_or(MetadataReadError::InvalidMetadata)?;
        let kind=read_u32(&mut file)?;
        match (key.as_str(),kind){
            ("general.architecture",8)=>{
                summary.architecture=safe_label(
                    read_string(&mut file,h.file_bytes,96)?,96);
            },
            ("tokenizer.ggml.model",8)=>{
                summary.tokenizer_model=safe_label(
                    read_string(&mut file,h.file_bytes,96)?,96);
            },
            ("general.file_type",4)=>summary.file_type=Some(read_u32(&mut file)?),
            (name,4) if name.ends_with(".context_length")=>{
                let n=u64::from(read_u32(&mut file)?);
                if (1..=10_000_000).contains(&n){
                    summary.reported_context_length=Some(n)
                }
            },
            (name,10) if name.ends_with(".context_length")=>{
                let n=read_u64(&mut file)?;
                if (1..=10_000_000).contains(&n){
                    summary.reported_context_length=Some(n)
                }
            },
            _=>skip_value(&mut file,kind,h.file_bytes)?,
        }
    }
    Ok(summary)
}
/// Conservative exact identity of all declared GGUF tokenizer.* metadata,
/// including original token strings, merges, special-token IDs and chat
/// template. Unlike matching `tokenizer.ggml.model` labels, two distinct
/// vocabularies cannot accidentally pass this check. This does NOT attest
/// tensor compatibility or quality. A pinned backend must still load both
/// originals and independently prove speculative acceptance.
pub fn tokenizer_metadata_sha256(path:&Path)->Result<[u8;32],MetadataReadError>{
    let h=inspect_gguf_header(path).map_err(|_|MetadataReadError::InvalidGguf)?;
    if h.metadata_count>MAX_KV{return Err(MetadataReadError::LimitExceeded)}
    let mut file=File::open(path)?;
    file.seek(SeekFrom::Start(24))?;
    let mut ordered=BTreeMap::<String,[u8;32]>::new();
    let mut tokens_found=false;
    let mut model_found=false;
    for _ in 0..h.metadata_count {
        if file.stream_position()?>MAX_METADATA_BYTES {
            return Err(MetadataReadError::LimitExceeded)
        }
        let key=read_string(&mut file,h.file_bytes,256)?
            .ok_or(MetadataReadError::InvalidMetadata)?;
        let kind=read_u32(&mut file)?;
        let value_start=file.stream_position()?;
        skip_value(&mut file,kind,h.file_bytes)?;
        let value_end=file.stream_position()?;
        if !key.starts_with("tokenizer.") {continue}
        if key=="tokenizer.ggml.tokens"{tokens_found=true}
        if key=="tokenizer.ggml.model"{model_found=true}
        let mut sha=Sha256::new();
        sha.update(kind.to_le_bytes());
        file.seek(SeekFrom::Start(value_start))?;
        let mut left=value_end.saturating_sub(value_start);
        let mut buffer=[0u8;65536];
        while left>0{
            let n=(left.min(buffer.len() as u64)) as usize;
            file.read_exact(&mut buffer[..n])?;
            sha.update(&buffer[..n]);
            left-=n as u64;
        }
        file.seek(SeekFrom::Start(value_end))?;
        if ordered.insert(key,sha.finalize().into()).is_some(){
            return Err(MetadataReadError::InvalidMetadata)
        }
    }
    if !tokens_found||!model_found||ordered.len()<2 {
        return Err(MetadataReadError::InvalidMetadata)
    }
    let mut identity=Sha256::new();
    identity.update(b"lu-p5-tokenizer-metadata-v1");
    for (key,value) in ordered {
        identity.update((key.len() as u32).to_le_bytes());
        identity.update(key.as_bytes());
        identity.update(value);
    }
    Ok(identity.finalize().into())
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::{fs,io::Write,sync::atomic::{AtomicU64,Ordering},time::{SystemTime,UNIX_EPOCH}};
    static NEXT_FIXTURE:AtomicU64=AtomicU64::new(0);
    fn bytes()->Vec<u8>{
        let mut s=b"GGUF".to_vec();
        s.extend(3u32.to_le_bytes());
        s.extend(1u64.to_le_bytes()); // one tensor; this parser never reads weights
        s.extend(3u64.to_le_bytes()); // 3 metadata KVs
        s
    }
    fn add_string(s:&mut Vec<u8>,key:&str,value:&str){
        s.extend((key.len() as u64).to_le_bytes());
        s.extend(key.as_bytes());
        s.extend(8u32.to_le_bytes());
        s.extend((value.len() as u64).to_le_bytes());
        s.extend(value.as_bytes());
    }
    fn add_u32(s:&mut Vec<u8>,key:&str,value:u32){
        s.extend((key.len() as u64).to_le_bytes());
        s.extend(key.as_bytes());
        s.extend(4u32.to_le_bytes());
        s.extend(value.to_le_bytes());
    }
    fn temp(bytes:&[u8])->std::path::PathBuf{
        let time=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        // Distinct Windows test threads must never share fixture names.
        let seq=NEXT_FIXTURE.fetch_add(1,Ordering::Relaxed);
        let path=std::env::temp_dir().join(format!(
            "lu-p03-meta-{}-{time}-{seq}.gguf",std::process::id()
        ));
        let mut out=fs::OpenOptions::new().write(true).create_new(true)
            .open(&path).unwrap();
        out.write_all(bytes).unwrap();
        out.sync_all().unwrap();
        drop(out);
        assert_eq!(fs::metadata(&path).unwrap().len(),bytes.len() as u64);
        path
    }
    #[test] fn actual_gguf_kv_values_are_read_without_loading_tensors(){
        let mut data=bytes();
        add_string(&mut data,"general.architecture","qwen3");
        add_u32(&mut data,"qwen3.context_length",262144);
        add_u32(&mut data,"general.file_type",15);
        let p=temp(&data);
        let got=inspect_metadata(&p).unwrap();
        assert_eq!(got.architecture.as_deref(),Some("qwen3"));
        assert_eq!(got.reported_context_length,Some(262144));
        assert_eq!(got.file_type,Some(15));
        fs::remove_file(p).unwrap();
    }
    fn token_fixture(vocab:&str,template:&str)->Vec<u8>{
        let mut s=b"GGUF".to_vec();
        s.extend(3u32.to_le_bytes());
        s.extend(1u64.to_le_bytes());
        s.extend(4u64.to_le_bytes());
        add_string(&mut s,"general.architecture","qwen3");
        add_string(&mut s,"tokenizer.ggml.model","gpt2");
        add_string(&mut s,"tokenizer.chat_template",template);
        let key="tokenizer.ggml.tokens";
        s.extend((key.len() as u64).to_le_bytes());
        s.extend(key.as_bytes());
        s.extend(9u32.to_le_bytes()); // tokenizer token array
        s.extend(8u32.to_le_bytes()); // string element type
        s.extend(2u64.to_le_bytes());
        for item in ["hello",vocab]{
            s.extend((item.len() as u64).to_le_bytes());
            s.extend(item.as_bytes());
        }
        s
    }
    #[test]
    fn p5_draft_tokenizer_fingerprint_rejects_equal_labels_but_different_vocab(){
        let first=temp(&token_fixture("world","template-v1"));
        let copy=temp(&token_fixture("world","template-v1"));
        let other_token=temp(&token_fixture("new-token","template-v1"));
        let other_template=temp(&token_fixture("world","template-v2"));
        let a=tokenizer_metadata_sha256(&first).unwrap();
        assert_eq!(a,tokenizer_metadata_sha256(&copy).unwrap());
        assert_ne!(a,tokenizer_metadata_sha256(&other_token).unwrap());
        assert_ne!(a,tokenizer_metadata_sha256(&other_template).unwrap());
        for p in [first,copy,other_token,other_template]{
            fs::remove_file(p).unwrap();
        }
    }
    #[test]
    fn p5_draft_tokenizer_rejects_missing_token_arrays(){
        let mut s=b"GGUF".to_vec();
        s.extend(3u32.to_le_bytes());
        s.extend(1u64.to_le_bytes());
        s.extend(1u64.to_le_bytes());
        add_string(&mut s,"tokenizer.ggml.model","gpt2");
        let p=temp(&s);
        assert!(tokenizer_metadata_sha256(&p).is_err());
        fs::remove_file(p).unwrap();
    }
    #[test] fn truncated_and_oversized_metadata_fail_closed(){
        let mut data=bytes();
        add_string(&mut data,"general.architecture","qwen3");
        add_u32(&mut data,"qwen3.context_length",262144);
        add_u32(&mut data,"general.file_type",15);
        data.truncate(data.len()-1);
        let p=temp(&data);
        assert!(inspect_metadata(&p).is_err());
        fs::remove_file(p).unwrap();
        let mut data=bytes();
        data[16..24].copy_from_slice(&(MAX_KV+1).to_le_bytes());
        let p=temp(&data);
        assert!(matches!(inspect_metadata(&p),Err(MetadataReadError::LimitExceeded)));
        fs::remove_file(p).unwrap();
    }
}
