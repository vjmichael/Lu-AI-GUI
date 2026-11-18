use lu_core::write_format::{identify_text_format, format_label, TextFormat, FormatError};

#[test]
fn model_selects_proper_text_extension_and_host_parses_json() {
    assert_eq!(identify_text_format("report.json", r#"{"valid":true,"count":2}"#),
        Ok(TextFormat::Json));
    assert_eq!(identify_text_format("report.JSON", "{ \"valid\": true }"),
        Ok(TextFormat::Json));
    assert_eq!(identify_text_format("events.jsonl", "{\"a\":1}\n{\"b\":2}\n"),
        Ok(TextFormat::JsonLines));
    assert_eq!(identify_text_format("notes.md", "# Heading\nSome text"),
        Ok(TextFormat::Markdown));
    assert_eq!(identify_text_format("main.rs", "fn main() {}"),
        Ok(TextFormat::SourceText));
    assert_eq!(identify_text_format("script.py", "print('hello')"),
        Ok(TextFormat::SourceText));
    assert_eq!(identify_text_format("settings.toml", "[general]\nready = true"),
        Ok(TextFormat::StructuredText));
    assert_eq!(identify_text_format(".gitignore", "target/\n*.tmp\n"),
        Ok(TextFormat::SourceText));
    assert!(format_label(TextFormat::Json).contains("parsed"));
}
#[test]
fn malformed_structured_or_binary_targets_are_refused_not_saved_under_wrong_extension() {
    assert_eq!(identify_text_format("report.json", "{broken"),
        Err(FormatError::InvalidJson));
    assert_eq!(identify_text_format("events.jsonl", "{\"ok\":1}\nNOPE"),
        Err(FormatError::InvalidJsonLines));
    for name in ["../etc/passwd", "a/b.py", "C:\\temp.json", "x\\escape.txt",
        ".lu-stage-hello.tmp", "a.exe", "a.gguf", "picture.png",
        "video.mp4", "financials.xlsx", "scene.blend"] {
        assert!(identify_text_format(name, "hello").is_err(),
            "filename was accepted as writable text: {name}");
    }
    assert_eq!(identify_text_format("data.txt", "ok\0bad"),
        Err(FormatError::InvalidText));
}
