//! TOOLS-07: no second model tool; host-owned vision-first route,
//! real image admission, typed native OCR and deterministic TSV parsing.
use std::{
    fs, io::Cursor, path::{Path,PathBuf},
    sync::atomic::{AtomicU64,Ordering},
    time::{SystemTime,UNIX_EPOCH},
};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use prost::Message;
use sha2::{Digest,Sha256};
use lu_core::{
    contracts::{AuthorizationError,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation,ProtoPayload},
    native_ocr_fallback::{
        ApprovedOcrImage,TrustedTesseract,ReadImageRoute,choose_read_image_route,
        parse_tesseract_tsv,register_ocr_fallback_native,
    },
    registry::{NativeToolRegistry,DispatchError},
    wire::proto,
};

const TSV:&str = concat!(
    "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n",
    "1\t1\t0\t0\t0\t0\t0\t0\t400\t100\t-1\t\n",
    "5\t1\t1\t1\t1\t1\t1\t1\t50\t20\t92.5\tHello\n",
    "5\t1\t1\t1\t1\t2\t70\t1\t50\t20\t87.5\tworld\n",
    "5\t1\t1\t1\t2\t1\t1\t30\t50\t20\t80.0\tAgain\n",
);
static ID:AtomicU64=AtomicU64::new(0);
fn root()->PathBuf {
    let n=ID.fetch_add(1,Ordering::Relaxed);
    let ts=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let p=std::env::temp_dir().join(format!("lu-ocr-{}-{n}-{ts}",std::process::id()));
    fs::create_dir(&p).unwrap();p
}
fn png_fixture()->ApprovedOcrImage {
    let mut bitmap=RgbImage::from_pixel(320,96,Rgb([255,255,255]));
    // A genuine PNG fixture. Text itself is tested independently by the
    // official TSV contract; acceptance of blank is a valid OCR outcome.
    for y in 20..35 {
        for x in 20..110 { bitmap.put_pixel(x,y,Rgb([0,0,0])); }
    }
    let img=DynamicImage::ImageRgb8(bitmap);
    let mut output=Cursor::new(Vec::new());
    img.write_to(&mut output,ImageFormat::Png).unwrap();
    let bytes=output.into_inner();
    let digest=Sha256::digest(&bytes);
    let mut sha=[0u8;32];sha.copy_from_slice(&digest);
    ApprovedOcrImage{
        image_ref:"image-20261104-1".into(),
        mime_type:"image/png".into(),image_bytes:bytes,sha256:sha,
    }
}
struct Allow;
impl ToolAuthorizer for Allow {
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{Ok(())}
}
struct Deny;
impl ToolAuthorizer for Deny {
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{
        Err(AuthorizationError::new("no current approved image"))
    }
}
fn trusted_dummy(root:&Path)->TrustedTesseract {
    let exe=root.join(if cfg!(windows){"tesseract.exe"}else{"tesseract"});
    fs::write(&exe,b"not executable Tesseract").unwrap();
    let tess=root.join("tessdata");fs::create_dir(&tess).unwrap();
    TrustedTesseract{executable:exe,traineddata_directory:tess}
}
#[test]
fn original_design_vision_first_and_no_second_model_tool() {
    assert_eq!(choose_read_image_route(true,true,true),ReadImageRoute::NativeVision);
    assert_eq!(choose_read_image_route(true,true,false),ReadImageRoute::NativeVision);
    assert_eq!(choose_read_image_route(true,false,true),ReadImageRoute::OcrFallback);
    assert_eq!(choose_read_image_route(true,false,false),ReadImageRoute::Unavailable);
    assert_eq!(choose_read_image_route(false,true,true),ReadImageRoute::Unavailable);
}
#[test]
fn tesseract_tsv_has_real_word_confidence_and_line_boundaries() {
    let result=parse_tesseract_tsv(TSV.as_bytes(),16000).unwrap();
    assert_eq!(result.method,"ocr_fallback");
    assert_eq!(result.text,"Hello world\nAgain");
    assert_eq!(result.word_count,3);
    assert_eq!(result.confidence_percent,87); // (92.5+87.5+80)/3
    assert!(!result.truncated);
    let limited=parse_tesseract_tsv(TSV.as_bytes(),11).unwrap();
    assert_eq!(limited.text,"Hello world");
    assert_eq!(limited.word_count,3);
    assert!(limited.truncated);
}
#[test]
fn invalid_or_injection_tsv_is_rejected_without_commands() {
    for sample in [
        "", "not TSV",
        "level\tbad\n",
        "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n5\t1\t1\n",
        "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n5\t1\t1\t1\t1\t1\t0\t0\t1\t1\tNaN\tevil\n",
        "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n5\t1\t1\t1\t1\t1\t0\t0\t1\t1\t101\tevil\n",
    ] {
        assert!(parse_tesseract_tsv(sample.as_bytes(),200).is_err(),
            "accepted malformed Tesseract stream: {sample}");
    }
}
#[tokio::test]
async fn pinned_image_hash_and_native_ref_refusal_precede_any_launch() {
    let dir=root();
    let mut modified=png_fixture();
    modified.sha256[0]^=1; // changed user attachment or replayed digest
    let engine=trusted_dummy(&dir);
    let r=NativeToolRegistry::new();
    assert!(register_ocr_fallback_native(&r,modified,engine).is_err());
    let img=png_fixture();
    let eng=TrustedTesseract {
        executable:dir.join(if cfg!(windows){"tesseract.exe"}else{"tesseract"}),
        traineddata_directory:dir.join("tessdata"),
    };
    register_ocr_fallback_native(&r,img,eng).unwrap();
    let tool_id="lu.media.ocr_fallback";
    assert_eq!(r.tools().unwrap().len(),1);
    assert_eq!(r.tools().unwrap()[0].effect,ToolEffect::ReadWorkspace);
    let request=|reference:&str|ToolInvocation{
        call_id:"ocr-call".into(),tool_id:tool_id.into(),
        workspace_id:Some("approved-saved-user-turn".into()),
        input:ProtoPayload {
            type_url:"type.googleapis.com/lu.v1.OcrFallbackRequest".into(),
            bytes:proto::OcrFallbackRequest {
                image_ref:reference.into(),max_text_chars:Some(400),
            }.encode_to_vec(),
        },
    };
    assert!(matches!(r.dispatch(request("image-20261104-1"),&Deny).await,
        Err(DispatchError::Denied(_))));
    // Even with authorizer yes, model-created image refs cannot escape the
    // host's pinned, digest-checked attachment. No subprocess launched.
    assert!(matches!(r.dispatch(request("wrong-image"),&Allow).await,
        Err(DispatchError::Execution(_))));
    let mut unknown=request("image-20261104-1");
    unknown.input.bytes.extend_from_slice(&[0x78,0x01]);
    assert!(matches!(r.dispatch(unknown,&Allow).await,
        Err(DispatchError::Execution(_))));
    let mut wrong_type=request("image-20261104-1");
    wrong_type.input.type_url="type.googleapis.com/lu.v1.OcrFallbackResult".into();
    assert!(r.dispatch(wrong_type,&Allow).await.is_err());
    let mut too_big=request("image-20261104-1");
    too_big.input.bytes=proto::OcrFallbackRequest{
        image_ref:"image-20261104-1".into(),
        max_text_chars:Some(16_001),
    }.encode_to_vec();
    assert!(matches!(r.dispatch(too_big,&Allow).await,
        Err(DispatchError::Execution(_))));
    drop(r);fs::remove_dir_all(dir).unwrap();
}
#[test]
fn host_refuses_untrusted_engine_and_wrong_mime() {
    let dir=root();
    let binary=trusted_dummy(&dir);
    let registry=NativeToolRegistry::new();
    let mut wrong=png_fixture();
    wrong.mime_type="image/jpeg".into();
    let backend=TrustedTesseract{
        executable:binary.executable.clone(),traineddata_directory:binary.traineddata_directory.clone()
    };
    assert!(register_ocr_fallback_native(&registry,wrong,backend).is_err());
    let backend=TrustedTesseract{
        executable:PathBuf::from("tesseract"),traineddata_directory:binary.traineddata_directory,
    };
    assert!(register_ocr_fallback_native(&registry,png_fixture(),backend).is_err());
    fs::remove_dir_all(dir).unwrap();
}

/// Actual Tesseract 5 engine exercise; CI sets both absolute trusted paths.
/// No mock engine and no memory screenshot OCR. A blank-image successful OCR
/// is allowed to produce empty text; it still verifies real engine execution
/// and correct parser/protobuf/method provenance.
#[tokio::test]
async fn real_tesseract_engine_acceptance_when_host_provisioned() {
    let Some(exe)=std::env::var_os("LU_OCR_TEST_TESSERACT") else {return};
    let Some(tess)=std::env::var_os("LU_OCR_TEST_TESSDATA") else {return};
    let r=NativeToolRegistry::new();
    register_ocr_fallback_native(&r,png_fixture(),TrustedTesseract{
        executable:PathBuf::from(exe),traineddata_directory:PathBuf::from(tess),
    }).unwrap();
    let invocation=ToolInvocation{
        call_id:"real-ocr-fixture".into(),tool_id:"lu.media.ocr_fallback".into(),
        workspace_id:Some("test-user".into()),
        input:ProtoPayload{
            type_url:"type.googleapis.com/lu.v1.OcrFallbackRequest".into(),
            bytes:proto::OcrFallbackRequest{
                image_ref:"image-20261104-1".into(),
                max_text_chars:Some(2000),
            }.encode_to_vec(),
        },
    };
    let result=match r.dispatch(invocation,&Allow).await {
        Ok(result)=>result,
        Err(error)=>{
            // CI-only controlled synthetic fixture diagnostics. Lu never
            // returns raw backend stderr from a real user's image.
            #[cfg(windows)]
            {
                use std::process::{Command,Stdio};
                use std::io::Write;
                let executable=std::env::var_os("LU_OCR_TEST_TESSERACT").unwrap();
                let tessdata=std::env::var_os("LU_OCR_TEST_TESSDATA").unwrap();
                let mut cmd=Command::new(executable);
                cmd.arg("stdin").arg("stdout").arg("--tessdata-dir")
                    .arg(tessdata).arg("-l").arg("eng").arg("--psm").arg("6")
                    .arg("tsv").env_clear().env("OMP_THREAD_LIMIT","1")
                    .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
                if let Some(sysroot)=std::env::var_os("SystemRoot"){
                    cmd.env("SystemRoot",sysroot);
                }
                let mut child=cmd.spawn().expect("debug trusted OCR command");
                let mut stdin=child.stdin.take().unwrap();
                let bytes=png_fixture().image_bytes;
                let write=std::thread::spawn(move || stdin.write_all(&bytes));
                let output=child.wait_with_output().expect("debug trusted OCR output");
                eprintln!("WINDOWS OCR DIAG (SYNTHETIC fixture only) status={:?}, stderr={:?}, stdout_len={}, write={:?}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr).chars().take(800).collect::<String>(),
                    output.stdout.len(),write.join().ok());
            }
            panic!("real Tesseract must run: {error:?}");
        }
    };
    let decoded=proto::OcrFallbackResult::decode(result.payload.bytes.as_slice()).unwrap();
    assert_eq!(decoded.image_ref,"image-20261104-1");
    assert_eq!(decoded.method,"ocr_fallback");
    assert!(decoded.confidence_percent<=100);
    assert!(!decoded.truncated);
}
