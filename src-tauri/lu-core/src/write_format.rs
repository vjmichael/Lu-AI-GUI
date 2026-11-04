//! WRITE-01D format-aware *text* admission for native create-new proposals.
//! The model selects a meaningful text filename/content; a trusted host checks
//! the format and exact bytes. This is NOT a language compiler nor a grant.
//!
//! JSON/JSONL receive syntactic parsing; source-code and other text formats
//! are preserved byte-for-byte for compiler/parser validation in later tools.
//! Binary/opaque extensions are excluded from this text-only path.
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFormat { Json, JsonLines, Markdown, PlainText, StructuredText, SourceText }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError { InvalidFilename, BinaryExtension, InvalidText, InvalidJson, InvalidJsonLines }

pub fn identify_text_format(filename: &str, content: &str) -> Result<TextFormat,FormatError> {
    if filename.is_empty() || filename.len() > 255 || filename=="." || filename==".."
        || filename.starts_with(".lu-stage-")
        || filename.chars().any(|ch| ch == '/' || ch == '\\' || ch == ':' || ch == '\0'
            || ch.is_control()) {
        return Err(FormatError::InvalidFilename);
    }
    if content.chars().any(|ch| ch == '\0') {
        return Err(FormatError::InvalidText);
    }
    let extension = Path::new(filename).extension()
        .and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase();
    // These extensions describe opaque/binary formats, not UTF-8 source.
    if matches!(extension.as_str(),
        "exe"|"dll"|"sys"|"so"|"dylib"|"bin"|"dat"|"gguf"|"png"|"jpg"|
        "jpeg"|"gif"|"webp"|"pdf"|"zip"|"7z"|"rar"|"mp4"|"mov"|"mp3"|
        "wav"|"ttf"|"otf"|"sqlite"|"db"|"ico"|"bmp"|"class"|"pyc"|
        "docx"|"xlsx"|"pptx"|"wasm"|"o"|"obj"|"a"|"lib"|"pdb"|
        "dcp"|"exr"|"tif"|"tiff"|"blend"|"psd") {
        return Err(FormatError::BinaryExtension);
    }
    match extension.as_str() {
        "json" => {
            serde_json::from_str::<serde_json::Value>(content)
                .map_err(|_|FormatError::InvalidJson)?;
            Ok(TextFormat::Json)
        }
        "jsonl"|"ndjson" => {
            for line in content.lines() {
                if line.trim().is_empty() { continue; }
                serde_json::from_str::<serde_json::Value>(line)
                    .map_err(|_|FormatError::InvalidJsonLines)?;
            }
            Ok(TextFormat::JsonLines)
        }
        "md"|"markdown"|"mdx"|"rst"|"adoc" => Ok(TextFormat::Markdown),
        "txt"|"text"|"log" => Ok(TextFormat::PlainText),
        "yaml"|"yml"|"toml"|"xml"|"csv"|"tsv"|"ini"|"cfg"|
        "conf"|"properties"|"sql"|"html"|"htm"|"css"|"svg"|"htaccess" => {
            Ok(TextFormat::StructuredText)
        },
        _ => Ok(TextFormat::SourceText),
    }
}

pub fn format_label(format: TextFormat) -> &'static str {
    match format {
        TextFormat::Json=>"JSON (parsed)",
        TextFormat::JsonLines=>"JSON Lines (parsed)",
        TextFormat::Markdown=>"Markdown text",
        TextFormat::PlainText=>"Plain UTF-8 text",
        TextFormat::StructuredText=>"Structured UTF-8 text (syntax not fully validated)",
        TextFormat::SourceText=>"Source or UTF-8 text (syntax not fully validated)",
    }
}
