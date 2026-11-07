//! TOOLS-COMPLETE-B: optional, truly offline OCR worker for a host-selected
//! image snapshot. Model text and retrieved documents cannot choose binaries,
//! executable arguments, language packs, source paths or authority.
//!
//! This adapter deliberately does not render PDFs. Scanned and complex PDFs
//! require a separately reviewed pinned PDF rasterizer, not hidden FFMPEG or
//! a magic OCR label. OCR is a fallible transcription, not source verification.
use image::{GenericImageView,ImageFormat,ImageReader,Limits};
use sha2::{Digest,Sha256};
use std::{
    fs::{self,File},
    io::{Cursor,Read,Write},
    path::Path,
    process::{Command,Stdio},
    sync::atomic::{AtomicBool,Ordering},
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
    WorkerFailed,Unreadable,UnsupportedEncoding,Cancelled,
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
/// Exact pixel rectangle in the ORIGINAL pinned image, not a model-selected
/// filename or an inferred coordinate system.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct ImageOcrRegion{pub x:u32,pub y:u32,pub width:u32,pub height:u32}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct RegionOcrObservation{
    pub observation:OcrObservation,
    pub original_sha256:String,
    pub ocr_input_sha256:String,
    pub region:ImageOcrRegion,
}
fn sha(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn hex_digest(s:&str)->bool{
    s.len()==64&&s.bytes().all(|c|c.is_ascii_hexdigit())
}
fn image_format(bytes:&[u8])->Result<&'static str,OcrFailure>{
    if bytes.len()>MAX_IMAGE_BYTES{return Err(OcrFailure::TooLarge);}
    if bytes.len()<64{return Err(OcrFailure::InvalidImage);}
    let (label,fmt)=if bytes.starts_with(b"\x89PNG\r\n\x1a\n"){
        ("png",ImageFormat::Png)
    }else if bytes.starts_with(&[0xff,0xd8,0xff]){
        ("jpeg",ImageFormat::Jpeg)
    }else{return Err(OcrFailure::InvalidImage)};
    // Do not feed a decompression bomb or corrupt header to the separately
    // pinned OCR worker. Lu's native decoder validates the full source first.
    let mut reader=ImageReader::with_format(Cursor::new(bytes),fmt);
    let mut limits=Limits::default();
    limits.max_image_width=Some(8192);
    limits.max_image_height=Some(8192);
    limits.max_alloc=Some(128*1024*1024);
    reader.limits(limits);
    let image=reader.decode().map_err(|_|OcrFailure::InvalidImage)?;
    let (w,h)=image.dimensions();
    if w==0||h==0||u64::from(w)*u64::from(h)>32_000_000{
        return Err(OcrFailure::TooLarge);
    }
    Ok(label)
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
/// One host-owned, bounded stdin->stdout filter. Both Tesseract and the
/// optional PDF rasterizer share this process containment/cancellation path.
fn run_owned_filter(
    mut worker:Command,input:&[u8],max_stdout:usize,
    runtime:Duration,cancel:&AtomicBool,
)->Result<Vec<u8>,OcrFailure>{
    if cancel.load(Ordering::SeqCst){return Err(OcrFailure::Cancelled)}
    if max_stdout==0||max_stdout>MAX_IMAGE_BYTES{return Err(OcrFailure::TooLarge)}
    worker.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        worker.creation_flags(0x0000_0004); // CREATE_SUSPENDED
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        worker.process_group(0);
    }
    #[cfg(windows)]
    let job=lu_win_worker_job::Job::new()
        .map_err(|_|OcrFailure::WorkerFailed)?;
    let mut child=worker.spawn().map_err(|_|OcrFailure::WorkerFailed)?;
    #[cfg(windows)]
    if job.assign_and_resume_suspended(&child).is_err(){
        let _=child.kill();let _=child.wait();
        return Err(OcrFailure::WorkerFailed);
    }
    let mut stdin=child.stdin.take().ok_or(OcrFailure::WorkerFailed)?;
    let stdout=child.stdout.take().ok_or(OcrFailure::WorkerFailed)?;
    let owned=input.to_vec();
    let writer=thread::spawn(move||{
        let result=stdin.write_all(&owned);
        drop(stdin);result
    });
    let reader=thread::spawn(move||{
        let mut out=Vec::new();
        let result=stdout.take(max_stdout as u64+1).read_to_end(&mut out);
        (result,out)
    });
    let deadline=Instant::now()+runtime;
    loop{
        if cancel.load(Ordering::SeqCst){
            #[cfg(unix)]
            let _=lu_win_worker_job::unix_owned::kill_owned_group(&child);
            let _=child.kill();let _=child.wait();
            let _=writer.join();let _=reader.join();
            return Err(OcrFailure::Cancelled);
        }
        match child.try_wait(){
            Ok(Some(status))=>{
                // Kill any descendants still holding inherited pipes before
                // blocking on thread joins after the direct child exits.
                #[cfg(unix)]
                let _=lu_win_worker_job::unix_owned::kill_owned_group(&child);
                #[cfg(windows)]
                drop(job);
                let sent=writer.join().map_err(|_|OcrFailure::WorkerFailed)?;
                let (result,data)=reader.join().map_err(|_|OcrFailure::WorkerFailed)?;
                if sent.is_err()||result.is_err()||!status.success(){
                    return Err(OcrFailure::WorkerFailed)
                }
                if data.len()>max_stdout{return Err(OcrFailure::TooLarge)}
                return Ok(data);
            },
            Ok(None) if Instant::now()<deadline=>thread::sleep(Duration::from_millis(40)),
            _=>{
                #[cfg(unix)]
                let _=lu_win_worker_job::unix_owned::kill_owned_group(&child);
                let _=child.kill();let _=child.wait();
                let _=writer.join();let _=reader.join();
                return Err(OcrFailure::Timeout);
            }
        }
    }
}
/// Only the trusted desktop may call this after the operator chooses an
/// original image. A model cannot pass a filename or executable path.
pub fn recognize_pinned_image(
    bytes:&[u8],engine:&Path,expected_engine_sha256:&str,
    tessdata:&Path,expected_english_sha256:&str,
)->Result<OcrObservation,OcrFailure>{
    let cancel=AtomicBool::new(false);
    recognize_pinned_image_cancellable(
        bytes,engine,expected_engine_sha256,tessdata,expected_english_sha256,&cancel
    )
}
/// The trusted host owns cancellation. Neither prompt text nor source bytes
/// can select executable arguments or shorten/rewrite the granted scope.
pub fn recognize_pinned_image_cancellable(
    bytes:&[u8],engine:&Path,expected_engine_sha256:&str,
    tessdata:&Path,expected_english_sha256:&str,cancel:&AtomicBool,
)->Result<OcrObservation,OcrFailure>{
    if cancel.load(Ordering::SeqCst){return Err(OcrFailure::Cancelled)}
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
    if cancel.load(Ordering::SeqCst){return Err(OcrFailure::Cancelled)}
    let mut worker=Command::new(engine);
    // No shell, no ambient TESSDATA_PREFIX, user profile or network paths.
    worker.env_clear()
        .args(["stdin","stdout","-l","eng","--psm","3","--tessdata-dir"])
        .arg(tessdata)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let data=run_owned_filter(worker,bytes,MAX_TEXT_BYTES,MAX_RUNTIME,cancel)?;
    let text=String::from_utf8(data).map_err(|_|OcrFailure::UnsupportedEncoding)?;
    let normalized=text.replace("\r\n","\n");
    if normalized.trim().is_empty(){return Err(OcrFailure::Unreadable);}
    if normalized.chars().any(|c|c=='\0'){return Err(OcrFailure::Unreadable);}
    Ok(OcrObservation{
        source_sha256:sha(bytes),recognized_text_sha256:sha(normalized.as_bytes()),
        engine_sha256:digest,recognized_text:normalized,source_format:fmt,
        extracted_from_image:true,source_verified:false,human_review_required:true,
    })
}

/// OCR one strictly bounded crop from an already admitted source. This uses
/// the same trusted Tesseract worker; the source digest remains the ORIGINAL
/// image SHA, while the cropped input receives its own separate SHA.
pub fn recognize_pinned_image_region(
    original:&[u8],region:ImageOcrRegion,
    engine:&Path,expected_engine_sha256:&str,
    tessdata:&Path,expected_english_sha256:&str,cancel:&AtomicBool,
)->Result<RegionOcrObservation,OcrFailure>{
    let fmt=image_format(original)?;
    if cancel.load(Ordering::SeqCst){return Err(OcrFailure::Cancelled)}
    if region.width==0||region.height==0||region.width>8192
        ||region.height>8192
        ||u64::from(region.width)*u64::from(region.height)>32_000_000{
        return Err(OcrFailure::TooLarge)
    }
    let mut reader=ImageReader::with_format(Cursor::new(original),
        if fmt=="png"{ImageFormat::Png}else{ImageFormat::Jpeg});
    let mut limits=Limits::default();
    limits.max_image_width=Some(8192);
    limits.max_image_height=Some(8192);
    limits.max_alloc=Some(128*1024*1024);
    reader.limits(limits);
    let image=reader.decode().map_err(|_|OcrFailure::InvalidImage)?;
    let (width,height)=image.dimensions();
    if region.x>=width||region.y>=height||
       region.width>width-region.x||region.height>height-region.y{
        return Err(OcrFailure::InvalidImage);
    }
    let mut crop=Vec::new();
    image.crop_imm(region.x,region.y,region.width,region.height)
        .write_to(&mut Cursor::new(&mut crop),ImageFormat::Png)
        .map_err(|_|OcrFailure::InvalidImage)?;
    if crop.len()>MAX_IMAGE_BYTES{return Err(OcrFailure::TooLarge)}
    let mut observation=recognize_pinned_image_cancellable(
        &crop,engine,expected_engine_sha256,
        tessdata,expected_english_sha256,cancel
    )?;
    let ocr_input_sha256=observation.source_sha256.clone();
    observation.source_sha256=sha(original);
    Ok(RegionOcrObservation{
        observation,original_sha256:sha(original),ocr_input_sha256,region
    })
}

/// OCR one explicit page of a user-selected, pinned PDF through a separately
/// SHA-reviewed offline Poppler PDF rasterizer. No embedded PDF code executes
/// inside Lu. A missing licensed renderer or trained data fails closed.
/// The rasterized page is unverified OCR input, not a source-verification pass.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct PdfPageOcrObservation{
    pub page:u16,
    pub original_pdf_sha256:String,
    pub raster_png_sha256:String,
    pub raster_engine_sha256:String,
    pub observation:OcrObservation,
}
pub fn recognize_pinned_pdf_page(
    pdf:&[u8],page:u16,
    rasterizer:&Path,expected_rasterizer_sha:&str,
    engine:&Path,expected_engine_sha:&str,
    tessdata:&Path,expected_data_sha:&str,cancel:&AtomicBool,
)->Result<PdfPageOcrObservation,OcrFailure>{
    if pdf.len()<20||pdf.len()>8*1024*1024{return Err(OcrFailure::TooLarge)}
    if !pdf.starts_with(b"%PDF-")
        ||!pdf[pdf.len().saturating_sub(2048)..].windows(5)
            .any(|window|window==b"%%EOF"){
        return Err(OcrFailure::InvalidImage)
    }
    if page==0||page>32{return Err(OcrFailure::InvalidImage)}
    if cancel.load(Ordering::SeqCst){return Err(OcrFailure::Cancelled)}
    let raster_sha=verified_engine(rasterizer,expected_rasterizer_sha)?;
    let number=page.to_string();
    let mut worker=Command::new(rasterizer);
    // Explicit Poppler syntax: -singlefile with output '-' streams exactly
    // one PNG to stdout, while input '-' reads only approved stdin PDF bytes.
    worker.env_clear().args([
        "-f",number.as_str(),"-l",number.as_str(),
        "-singlefile","-scale-to","2048","-png","-","-"
    ]);
    let png=run_owned_filter(
        worker,pdf,MAX_IMAGE_BYTES,Duration::from_secs(35),cancel
    )?;
    if image_format(&png)?!="png"{return Err(OcrFailure::InvalidImage)}
    let raster_png_sha256=sha(&png);
    let mut observation=recognize_pinned_image_cancellable(
        &png,engine,expected_engine_sha,tessdata,expected_data_sha,cancel
    )?;
    let original_pdf_sha256=sha(pdf);
    observation.source_sha256=original_pdf_sha256.clone();
    observation.source_format="pdf";
    Ok(PdfPageOcrObservation{
        page,original_pdf_sha256,raster_png_sha256,
        raster_engine_sha256:raster_sha,observation,
    })
}

/// A short operator-approved PDF page range. Page receipts retain independent
/// derived raster identity and text hashes; a failed page returns NO partial
/// transcript disguised as a successful full range.
pub fn recognize_pinned_pdf_pages(
    pdf:&[u8],first:u16,last:u16,
    rasterizer:&Path,expected_rasterizer_sha:&str,
    engine:&Path,expected_engine_sha:&str,
    tessdata:&Path,expected_data_sha:&str,cancel:&AtomicBool,
)->Result<Vec<PdfPageOcrObservation>,OcrFailure>{
    if first==0||last<first||last>32||last-first>=8{
        return Err(OcrFailure::InvalidImage)
    }
    if cancel.load(Ordering::SeqCst){return Err(OcrFailure::Cancelled)}
    let mut results=Vec::with_capacity(usize::from(last-first+1));
    for page in first..=last{
        if cancel.load(Ordering::SeqCst){return Err(OcrFailure::Cancelled)}
        let observation=recognize_pinned_pdf_page(
            pdf,page,rasterizer,expected_rasterizer_sha,
            engine,expected_engine_sha,tessdata,expected_data_sha,cancel
        )?;
        results.push(observation);
    }
    Ok(results)
}

#[cfg(test)]
mod tests{
    use super::*;
    use image::DynamicImage;
    #[test]
    fn ocr_admission_rejects_corrupt_magic_and_decompression_bombs(){
        let mut image=Vec::new();
        DynamicImage::new_rgb8(40,20)
            .write_to(&mut Cursor::new(&mut image),ImageFormat::Png).unwrap();
        assert_eq!(image_format(&image),Ok("png"));
        let mut corrupt=image.clone();
        corrupt.truncate(80);
        assert_eq!(image_format(&corrupt),Err(OcrFailure::InvalidImage));
        assert_eq!(image_format(b"not a real png"),
            Err(OcrFailure::InvalidImage));
    }
    #[test]
    fn pdf_batch_bounds_fail_before_any_untrusted_worker(){
        let cancel=AtomicBool::new(false);
        let missing=Path::new("/nonexistent");
        for (start,end) in [(0,1),(2,1),(1,9),(25,33)]{
            assert_eq!(recognize_pinned_pdf_pages(
                b"not PDF",start,end,missing,"",missing,"",missing,"",&cancel
            ),Err(OcrFailure::InvalidImage));
        }
        cancel.store(true,Ordering::SeqCst);
        assert_eq!(recognize_pinned_pdf_pages(
            b"not PDF",2,5,missing,"",missing,"",missing,"",&cancel
        ),Err(OcrFailure::Cancelled));
    }
    #[test]
    fn pdf_ocr_refuses_unsafe_page_or_untrusted_pdf_before_spawn(){
        let cancelled=AtomicBool::new(false);
        let dummy=Path::new("/no-ocr-worker");
        let mut pdf=b"%PDF-1.4\n".to_vec();
        pdf.extend(vec![b' ';32]);
        pdf.extend(b"\n%%EOF\n");
        assert_eq!(recognize_pinned_pdf_page(
            &pdf,0,dummy,"",dummy,"",dummy,"",&cancelled
        ),Err(OcrFailure::InvalidImage));
        assert_eq!(recognize_pinned_pdf_page(
            b"not a pdf",1,dummy,"",dummy,"",dummy,"",&cancelled
        ),Err(OcrFailure::TooLarge));
        cancelled.store(true,Ordering::SeqCst);
        assert_eq!(recognize_pinned_pdf_page(
            &pdf,1,dummy,"",dummy,"",dummy,"",&cancelled
        ),Err(OcrFailure::Cancelled));
    }
    #[test]
    fn region_ocr_rejects_geometry_before_worker_invocation(){
        let mut png=Vec::new();
        image::DynamicImage::new_rgb8(60,40)
            .write_to(&mut Cursor::new(&mut png),ImageFormat::Png).unwrap();
        let cancel=AtomicBool::new(false);
        let invalid=recognize_pinned_image_region(
            &png,ImageOcrRegion{x:59,y:0,width:2,height:3},
            Path::new("/nonexistent"),"0",Path::new("/nonexistent"),"0",&cancel
        );
        assert_eq!(invalid,Err(OcrFailure::InvalidImage));
        cancel.store(true,Ordering::SeqCst);
        let cancelled=recognize_pinned_image_region(
            &png,ImageOcrRegion{x:1,y:2,width:3,height:3},
            Path::new("/nonexistent"),"0",Path::new("/nonexistent"),"0",&cancel
        );
        assert_eq!(cancelled,Err(OcrFailure::Cancelled));
    }
}
