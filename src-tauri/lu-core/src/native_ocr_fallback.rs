//! First-party Lu Core OCR fallback, not a second image-reading model tool.
//!
//! Original Jan-Mod Design §5.4: use genuine model vision when supported;
//! otherwise pass the *already authorized* image to Tesseract and return
//! OCR text + observed confidence + "ocr_fallback" provenance.
//!
//! The host, not a model argument, owns the image bytes, image reference,
//! executable, traineddata directory and the verified model vision flag.
//! No shell/MCP, arbitrary paths, downloaded data, writes or model-supplied
//! language/command. Runtime uses Tesseract's documented TSV interface.
//! This module DOES NOT advertise a standalone model schema. A future
//! read_image host adapter must select the route and authorize the image.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::{AtomicBool, Ordering}, Arc},
    time::{Duration, Instant},
};

use image::{ImageFormat, ImageReader};
use prost::Message;
use sha2::{Digest, Sha256};

use crate::{
    contracts::{
        ModuleKind, ModuleManifest, NativeTool, ProtoPayload, ToolDescriptor, ToolEffect,
        ToolFailure, ToolFuture, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION,
    },
    inference_image_refs::{validate_image_refs, ImageRef, MAX_IMAGE_BYTES},
    registry::NativeToolRegistry,
    wire::proto,
};

const MODULE: &str = "lu.media";
const TOOL: &str = "lu.media.ocr_fallback";
const INPUT: &str = "type.googleapis.com/lu.v1.OcrFallbackRequest";
const OUTPUT: &str = "type.googleapis.com/lu.v1.OcrFallbackResult";
const DEFAULT_TEXT_CHARS: u32 = 4000;
const MAX_TEXT_CHARS: u32 = 16_000;
const MAX_TSV_BYTES: usize = 256 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 4096;
const MAX_IMAGE_PIXELS: u64 = 16_000_000;
const OCR_TIMEOUT: Duration = Duration::from_secs(12);

fn error(code: &str, reason: &str) -> ToolFailure {
    ToolFailure::new(code, reason, false)
}

/// A routing *recommendation*, NOT a grant to read an attachment. Both
/// original-image and OCR paths still need the current human-turn authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadImageRoute {
    NativeVision,
    OcrFallback,
    Unavailable,
}
pub fn choose_read_image_route(
    trusted_image_admitted: bool,
    model_vision_verified: bool,
    ocr_backend_available: bool,
) -> ReadImageRoute {
    if !trusted_image_admitted {
        ReadImageRoute::Unavailable
    } else if model_vision_verified {
        ReadImageRoute::NativeVision
    } else if ocr_backend_available {
        ReadImageRoute::OcrFallback
    } else {
        ReadImageRoute::Unavailable
    }
}

/// Created by a trusted attachment resolver for ONE persisted user turn.
/// This is NOT supplied by model JSON, a WebView path or an ANSF citation.
pub struct ApprovedOcrImage {
    pub image_ref: String,
    pub mime_type: String,
    pub image_bytes: Vec<u8>,
    pub sha256: [u8; 32],
}

/// Host-configured, installed/provisioned first-party OCR engine. The model
/// may not choose these paths, language, options or locale.
pub struct TrustedTesseract {
    pub executable: PathBuf,
    pub traineddata_directory: PathBuf,
}

struct PinnedOcr {
    image: ApprovedOcrImage,
    executable: PathBuf,
    traineddata: PathBuf,
}

fn validate_image(image: &ApprovedOcrImage) -> Result<(), String> {
    let ref_info = ImageRef {
        image_ref: &image.image_ref,
        mime_type: &image.mime_type,
        sha256: &image.sha256,
        size_bytes: image.image_bytes.len() as u64,
    };
    validate_image_refs(&[ref_info])
        .map_err(|e| format!("approved image reference rejected: {e:?}"))?;
    if image.image_bytes.is_empty() || image.image_bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err("approved image has an invalid byte count".into());
    }
    let hash = Sha256::digest(&image.image_bytes);
    if hash.as_slice() != image.sha256 {
        return Err("approved image digest mismatch".into());
    }
    let format = image::guess_format(&image.image_bytes)
        .map_err(|_| "approved image format invalid")?;
    match (image.mime_type.as_str(), format) {
        ("image/png", ImageFormat::Png) | ("image/jpeg", ImageFormat::Jpeg) => {}
        _ => return Err("approved image MIME and bytes disagree".into()),
    };
    // Reject extreme dimensions *before* full decoding, not only after a
    // decompression bomb has allocated a huge image buffer.
    let reader = ImageReader::with_format(
        std::io::Cursor::new(image.image_bytes.as_slice()), format,
    );
    let (width, height) = reader.into_dimensions()
        .map_err(|_| "approved image header/dimensions cannot be decoded")?;
    if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
    {
        return Err("approved image dimensions exceed OCR admission limits".into());
    }
    image::load_from_memory_with_format(&image.image_bytes, format)
        .map_err(|_| "approved image payload could not be decoded")?;
    Ok(())
}

fn trusted_native_path(path: &Path, expect_dir: bool) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("OCR runtime path must be approved and absolute".into());
    }
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| "approved OCR installation missing")?;
    if meta.file_type().is_symlink() {
        return Err("OCR runtime path cannot be a symlink".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("OCR runtime Windows reparse-point path refused".into());
        }
    }
    if (expect_dir && !meta.is_dir()) || (!expect_dir && !meta.is_file()) {
        return Err("incorrect approved OCR installation file type".into());
    }
    let canonical = path.canonicalize().map_err(|_| "cannot pin OCR runtime path")?;
    if !expect_dir && cfg!(windows)
        && !canonical.to_string_lossy().to_ascii_lowercase().ends_with(".exe")
    {
        return Err("Windows OCR runtime must be a native executable".into());
    }
    Ok(canonical)
}

/// Parse word-level TSV. The TSV contents are untrusted OCR data: no commands,
/// permissions or image paths are ever inferred from text. A confidence
/// number is an ENGINE ESTIMATE for recognized words only.
pub fn parse_tesseract_tsv(
    output: &[u8], max_chars: usize,
) -> Result<proto::OcrFallbackResult, ToolFailure> {
    if output.is_empty() || output.len() > MAX_TSV_BYTES || max_chars == 0
        || max_chars > MAX_TEXT_CHARS as usize
    {
        return Err(error("OCR_OUTPUT_INVALID", "OCR output exceeds validated bounds"));
    }
    let raw = std::str::from_utf8(output)
        .map_err(|_| error("OCR_OUTPUT_INVALID", "OCR TSV is not UTF-8"))?;
    let mut lines = raw.lines();
    let header = lines.next().unwrap_or("").trim_end_matches('\r');
    if header != "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext" {
        return Err(error("OCR_OUTPUT_INVALID", "OCR TSV header does not match the expected schema"));
    }
    let mut text = String::new();
    let mut count = 0u32;
    let mut seen_conf = 0u32;
    let mut confidence_sum = 0f64;
    let mut last_line: Option<(u32,u32,u32,u32)> = None;
    let mut truncated = false;
    let mut visited = 0usize;
    for line in lines {
        visited += 1;
        if visited > 12_000 {
            return Err(error("OCR_OUTPUT_INVALID", "OCR TSV record budget exceeded"));
        }
        let cells: Vec<_> = line.trim_end_matches('\r').splitn(12,'\t').collect();
        if cells.len() != 12 {
            return Err(error("OCR_OUTPUT_INVALID", "OCR TSV row is incomplete"));
        }
        let level: u32 = cells[0].parse().map_err(|_|
            error("OCR_OUTPUT_INVALID","OCR TSV row has invalid level"))?;
        if level != 5 {
            continue; // Page/block/paragraph/line rows carry no word confidence
        }
        let coord = (
            cells[1].parse::<u32>(),
            cells[2].parse::<u32>(),
            cells[3].parse::<u32>(),
            cells[4].parse::<u32>(),
        );
        let (Ok(p),Ok(b),Ok(par),Ok(l)) = coord else {
            return Err(error("OCR_OUTPUT_INVALID", "OCR TSV word location invalid"));
        };
        let word = cells[11].trim();
        if word.is_empty() { continue; }
        // The engine emits a floating-point confidence 0..100, not a
        // calibrated probability or independently verified recognition.
        let confidence: f64 = cells[10].parse().map_err(|_|
            error("OCR_OUTPUT_INVALID", "OCR TSV word confidence invalid"))?;
        if !confidence.is_finite() || !(0.0..=100.0).contains(&confidence) {
            return Err(error("OCR_OUTPUT_INVALID", "OCR TSV confidence outside 0-100"));
        }
        count = count.saturating_add(1);
        seen_conf = seen_conf.saturating_add(1);
        confidence_sum += confidence;
        let group = (p,b,par,l);
        let separator = if text.is_empty() { "" }
            else if last_line != Some(group) { "\n" } else { " " };
        let candidate_chars = separator.chars().count() + word.chars().count();
        if text.chars().count() + candidate_chars > max_chars {
            truncated = true;
        } else if !truncated {
            text.push_str(separator);
            text.push_str(word);
        }
        last_line = Some(group);
    }
    Ok(proto::OcrFallbackResult {
        image_ref: String::new(), // host inserts verified exact pin
        text,
        confidence_percent: if seen_conf == 0 { 0 } else {
            (confidence_sum / seen_conf as f64).round().clamp(0.0,100.0) as u32
        },
        truncated,
        method: "ocr_fallback".into(),
        word_count: count,
    })
}

fn run_engine(
    pinned: &PinnedOcr, max_chars: u32,
) -> Result<proto::OcrFallbackResult, ToolFailure> {
    // Fixed trusted host-selected executable and fixed English TSV arguments.
    // A native executable is a deliberate first-party dependency, NOT a
    // user/model shell capability. The run is bounded and reports failure.
    let mut command = Command::new(&pinned.executable);
    command
        .arg("stdin").arg("stdout")
        .arg("--tessdata-dir").arg(&pinned.traineddata)
        .arg("-l").arg("eng").arg("--psm").arg("6").arg("tsv")
        .env_clear()
        .env("OMP_THREAD_LIMIT", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    if let Some(windows_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", windows_root);
    }
    let mut process = command.spawn()
        .map_err(|_| error("OCR_UNAVAILABLE","approved OCR engine could not start"))?;
    let Some(mut input) = process.stdin.take() else {
        let _=process.kill();let _=process.wait();
        return Err(error("OCR_PROCESS", "OCR engine input unavailable"));
    };
    let Some(mut output) = process.stdout.take() else {
        let _=process.kill();let _=process.wait();
        return Err(error("OCR_PROCESS", "OCR engine output unavailable"));
    };
    let bytes = pinned.image.image_bytes.clone();
    let writer = std::thread::spawn(move || input.write_all(&bytes));
    let too_large = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&too_large);
    let reader = std::thread::spawn(move || {
        let mut data=Vec::new();
        let result = (&mut output).take(MAX_TSV_BYTES as u64+1)
            .read_to_end(&mut data);
        if data.len() > MAX_TSV_BYTES { signal.store(true,Ordering::Release); }
        result.map(|_| data)
    });
    let deadline=Instant::now()+OCR_TIMEOUT;
    let exit_ok=loop {
        if too_large.load(Ordering::Acquire) {
            let _=process.kill();let _=process.wait();break false;
        }
        match process.try_wait() {
            Ok(Some(exit)) => break exit.success(),
            Ok(None) if Instant::now()<deadline =>
                std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _=process.kill();let _=process.wait();
                let _=writer.join();let _=reader.join();
                return Err(error("OCR_TIMEOUT","OCR backend failed to finish on time"));
            }
        }
    };
    let input_ok=writer.join().map_err(|_|error("OCR_PROCESS","OCR input worker stopped"))?;
    let result=reader.join().map_err(|_|error("OCR_PROCESS","OCR output worker stopped"))?
        .map_err(|_|error("OCR_PROCESS","OCR output stream failed"))?;
    if too_large.load(Ordering::Acquire) || result.len() > MAX_TSV_BYTES {
        return Err(error("OCR_OUTPUT_LIMIT","OCR output exceeded the configured byte budget"));
    }
    if !exit_ok || input_ok.is_err() {
        return Err(error("OCR_FAILED","OCR engine rejected the approved image"));
    }
    let mut parsed=parse_tesseract_tsv(&result,max_chars as usize)?;
    parsed.image_ref=pinned.image.image_ref.clone();
    Ok(parsed)
}

struct FallbackTool {
    descriptor: ToolDescriptor,
    pinned: Arc<PinnedOcr>,
}
impl NativeTool for FallbackTool {
    fn descriptor(&self)->&ToolDescriptor { &self.descriptor }
    fn invoke(&self,invocation:ToolInvocation)->ToolFuture<'_> {
        let pinned=Arc::clone(&self.pinned);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let input=proto::OcrFallbackRequest::decode(invocation.input.bytes.as_slice())
                    .map_err(|_|error("BAD_PROTOBUF","invalid OCR request"))?;
                if input.encode_to_vec()!=invocation.input.bytes {
                    return Err(error("BAD_PROTOBUF","noncanonical OCR request"));
                }
                if input.image_ref!=pinned.image.image_ref {
                    return Err(error("IMAGE_REF_DENIED",
                        "OCR image reference is not the approved pinned attachment"));
                }
                let max_chars=input.max_text_chars.unwrap_or(DEFAULT_TEXT_CHARS);
                if !(1..=MAX_TEXT_CHARS).contains(&max_chars) {
                    return Err(error("OCR_INPUT_LIMIT","OCR output limit outside allowed range"));
                }
                let value=run_engine(&pinned,max_chars)?;
                Ok(ToolOutput {payload:ProtoPayload{
                    type_url:OUTPUT.into(),bytes:value.encode_to_vec(),
                }})
            }).await.map_err(|_|error("OCR_WORKER","OCR worker stopped"))?
        })
    }
}

/// Host-only native Core registry. Deliberately takes NO ModelToolCatalog:
/// model sees just the existing read_image intent, and the host selects the
/// verified vision-vs-OCR route. Neither OCR nor image bytes enter a prompt
/// unless a trusted user turn and ReadWorkspace authorization allow them.
pub fn register_ocr_fallback_native(
    registry:&NativeToolRegistry,
    image:ApprovedOcrImage,
    backend:TrustedTesseract,
) -> Result<(),String> {
    validate_image(&image)?;
    let executable=trusted_native_path(&backend.executable,false)?;
    let traineddata=trusted_native_path(&backend.traineddata_directory,true)?;
    if executable.starts_with(&traineddata) {
        return Err("OCR executable cannot live inside the traineddata directory".into());
    }
    let descriptor=ToolDescriptor {
        id:TOOL.into(),display_name:"Lu trusted vision fallback OCR".into(),
        effect:ToolEffect::ReadWorkspace,
        input_type_url:INPUT.into(),output_type_url:OUTPUT.into(),
    };
    registry.register(
        ModuleManifest {
            id:MODULE.into(),display_name:"Lu native image OCR fallback".into(),
            contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
            tools:vec![descriptor.clone()],panels:vec![],workflows:vec![],
        },
        vec![Arc::new(FallbackTool {
            descriptor,pinned:Arc::new(PinnedOcr{image,executable,traineddata}),
        })],
    ).map_err(|e|format!("OCR native registration refused: {e:?}"))
}
