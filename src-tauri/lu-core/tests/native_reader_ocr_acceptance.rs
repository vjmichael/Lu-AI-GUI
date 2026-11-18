//! Run with a two-page image-only PDF bearing the fixture text below.
use lu_core::{
    native_ocr::{recognize_pinned_pdf_pages, OcrFailure},
    native_reader::{read_selected_bytes, ReaderFailure, ReaderFormat},
};
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf, sync::atomic::AtomicBool};

fn fixture_path(key: &str) -> PathBuf {
    PathBuf::from(env::var_os(key).unwrap_or_else(|| panic!("{key} required")))
        .canonicalize()
        .unwrap()
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
#[ignore = "requires real reviewed Poppler, Tesseract, English data and scanned fixture"]
fn scanned_pdf_preserves_original_source_and_per_page_ocr_identity() {
    let pdf = fs::read(fixture_path("LU_READER_SCANNED_PDF")).unwrap();
    let poppler = fixture_path("LU_READER_PDFTOPPM");
    let tesseract = fixture_path("LU_READER_TESSERACT");
    let tessdata = fixture_path("LU_READER_TESSDATA");
    let poppler_sha = sha(&fs::read(&poppler).unwrap());
    let tesseract_sha = sha(&fs::read(&tesseract).unwrap());
    let language_sha = sha(&fs::read(tessdata.join("eng.traineddata")).unwrap());
    let pdf_sha = sha(&pdf);
    assert!(matches!(read_selected_bytes("scan.pdf", &pdf, 500, "page:1"),
        Err(ReaderFailure::UnsupportedAdapter(ReaderFormat::Pdf))));

    let pages = recognize_pinned_pdf_pages(
        &pdf, 1, 2, &poppler, &poppler_sha, &tesseract, &tesseract_sha,
        &tessdata, &language_sha, &AtomicBool::new(false),
    ).unwrap();
    assert_eq!(pages.len(), 2);
    assert_ne!(pages[0].raster_png_sha256, pages[1].raster_png_sha256);
    for (index, text) in ["ALPHA SOURCE PAGE", "BRAVO SOURCE PAGE"].iter().enumerate() {
        let page = &pages[index];
        assert_eq!(page.page, (index + 1) as u16);
        assert_eq!(page.original_pdf_sha256, pdf_sha);
        assert_eq!(page.raster_engine_sha256, poppler_sha);
        assert_eq!(page.observation.source_sha256, pdf_sha);
        assert_eq!(page.observation.engine_sha256, tesseract_sha);
        assert_eq!(page.observation.recognized_text.trim(), *text);
        assert_eq!(page.observation.recognized_text_sha256, sha(page.observation.recognized_text.as_bytes()));
        assert!(page.observation.human_review_required);
        assert!(!page.observation.source_verified);
    }
    assert_eq!(recognize_pinned_pdf_pages(
        &pdf, 1, 2, &poppler, &poppler_sha, &tesseract, &tesseract_sha,
        &tessdata, &language_sha, &AtomicBool::new(true),
    ), Err(OcrFailure::Cancelled));
}
