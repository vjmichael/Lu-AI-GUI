//! TOOLS-COMPLETE-B: optional, truly offline OCR worker for a host-selected
//! image snapshot. Model text and retrieved documents cannot choose binaries,
//! executable arguments, language packs, source paths or authority.
//!
//! This adapter deliberately does not render PDFs. Scanned and complex PDFs
//! require a separately reviewed pinned PDF rasterizer, not hidden FFMPEG or
//! a magic OCR label. OCR is a fallible transcription, not source verification.
use sha2::{Digest,Sha256};
use std::{
    fs::{self,File},
    io::{Read,Write},
    path::Path,
    process::{Command,Stdio},
    thread,
    time::{Duration,Instant},
};

const MAX_IMAGE_BYTES:usize=12*1024*1024;
const MAX_TEXT_BYTES:usize=1_048_576;
const MAX_ENGINE_BYTES:u64=100*1024*1024;
const MAX_RUNTIME:Duration=Duration::from_secs(30);

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum OcrFailure{
    Unavailable,InvalidImage,TooLarge,InvalidEngine,Timeout,
    WorkerFailed,Unreadable,UnsupportedEncoding,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct OcrObservation {
    pub source_sha256:String,
    pub recognized_text_sha256:String,
    pub engine_sha256:String,
    pub recognized_text:String,
    pub source_format:&'static str,
    pub extracted_from_image:bool,
    pub source_verified:bool,
    pub human_review_required:bool,
}
fn sha(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn hex_digest(s:&str)->bool{
    s.len()==64&&s.bytes().all(|c|c.is_ascii_hexdigit())
}
fn image_format(bytes:&[u8])->Result<&'static str,OcrFailure>{
    if bytes.len()>MAX_IMAGE_BYTES{return Err(OcrFailure::TooLarge);}
    if bytes.len()<64{return Err(OcrFailure::InvalidImage);}
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n"){Ok("png")}
    else if bytes.starts_with(&[0xff,0xd8,0xff]){Ok("jpeg")}
    else{return Err(OcrFailure::InvalidImage);}
}
fn verified_engine(binary:&Path,expected_sha:&str)->Result<String,OcrFailure>{
    if !hex_digest(expected_sha)||!binary.is_absolute(){
        return Err(OcrFailure::InvalidEngine);
    }
    let meta=fs::symlink_metadata(binary).map_err(|_|OcrFailure::Unavailable)?;
    if !meta.file_type().is_file()||meta.file_type().is_symlink()
        ||meta.len()>MAX_ENGINE_BYTES{return Err(OcrFailure::InvalidEngine);}
    let mut src=File::open(binary).map_err(|_|OcrFailure::Unavailable)?;
    let mut digest=Sha256::new();
    let mut buf=[0u8;32*1024];
    let mut read=0u64;
    loop{
        let len=src.read(&mut buf).map_err(|_|OcrFailure::Unavailable)?;
        if len==0{break;}
        read+=len as u64;
        if read>MAX_ENGINE_BYTES{return Err(OcrFailure::InvalidEngine);}
        digest.update(&buf[..len]);
    }
    let got=format!("{:x}",digest.finalize());
    if !got.eq_ignore_ascii_case(expected_sha){
        return Err(OcrFailure::InvalidEngine);
    }
    Ok(got)
}
/// Only the trusted desktop may call this after the operator chooses an
/// original image. A model cannot pass a filename or executable path.
pub fn recognize_pinned_image(
    bytes:&[u8],engine:&Path,expected_engine_sha256:&str,
    tessdata:&Path,expected_english_sha256:&str,
)->Result<OcrObservation,OcrFailure>{
    let fmt=image_format(bytes)?;
    let digest=verified_engine(engine,expected_engine_sha256)?;
    let tess=fs::symlink_metadata(tessdata).map_err(|_|OcrFailure::Unavailable)?;
    if !tess.file_type().is_dir()||tess.file_type().is_symlink(){
        return Err(OcrFailure::Unavailable);
    }
    let eng=tessdata.join("eng.traineddata");
    let trained=fs::symlink_metadata(&eng).map_err(|_|OcrFailure::Unavailable)?;
    if !trained.file_type().is_file()||trained.file_type().is_symlink()
        ||trained.len()>50*1024*1024{
        return Err(OcrFailure::Unavailable);
    }
    // A reviewed installer's language data can be modified or replaced
    // after staging. Rehash the exact asset before every image read.
    let _english_sha=verified_engine(&eng,expected_english_sha256)?;
    let mut worker=Command::new(engine);
    // No shell, no ambient TESSDATA_PREFIX, user profile or network paths.
    worker.env_clear()
        .args(["stdin","stdout","-l","eng","--psm","3","--tessdata-dir"])
        .arg(tessdata)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child=worker.spawn().map_err(|_|OcrFailure::WorkerFailed)?;
    let mut stdin=child.stdin.take().ok_or(OcrFailure::WorkerFailed)?;
    let stdout=child.stdout.take().ok_or(OcrFailure::WorkerFailed)?;
    // Both pipes are serviced on dedicated bounded workers. A malicious
    // broken OCR process that never reads stdin must not hang the host's
    // user-visible request indefinitely. The worker is killed on deadline.
    let owned_image=bytes.to_vec();
    let writer=thread::spawn(move||{
        let result=stdin.write_all(&owned_image);
        drop(stdin);
        result
    });
    let reader=thread::spawn(move||{
        let mut out=Vec::new();
        let result=stdout.take((MAX_TEXT_BYTES+1) as u64).read_to_end(&mut out);
        (result,out)
    });
    let deadline=Instant::now()+MAX_RUNTIME;
    loop{
        match child.try_wait(){
            Ok(Some(status))=>{
                let sent=writer.join().map_err(|_|OcrFailure::WorkerFailed)?;
                let (r,data)=reader.join().map_err(|_|OcrFailure::WorkerFailed)?;
                if sent.is_err()||r.is_err()||!status.success(){
                    return Err(OcrFailure::WorkerFailed);
                }
                if data.len()>MAX_TEXT_BYTES{return Err(OcrFailure::TooLarge);}
                let text=String::from_utf8(data)
                    .map_err(|_|OcrFailure::UnsupportedEncoding)?;
                let normalized=text.replace("\r\n","\n");
                if normalized.trim().is_empty(){return Err(OcrFailure::Unreadable);}
                if normalized.chars().any(|c|c=='\0'){
                    return Err(OcrFailure::Unreadable);
                }
                return Ok(OcrObservation{
                    source_sha256:sha(bytes),
                    recognized_text_sha256:sha(normalized.as_bytes()),
                    engine_sha256:digest,recognized_text:normalized,
                    source_format:fmt,extracted_from_image:true,
                    source_verified:false,human_review_required:true,
                });
            },
            Ok(None) if Instant::now()<deadline=>{
                thread::sleep(Duration::from_millis(40));
            },
            _=>{
                let _=child.kill();
                let _=child.wait();
                // Killing the process closes both pipe endpoints so both
                // bounded workers can join without leaving live tasks.
                let _=writer.join();
                let _=reader.join();
                return Err(OcrFailure::Timeout);
            },
        }
    }
}
