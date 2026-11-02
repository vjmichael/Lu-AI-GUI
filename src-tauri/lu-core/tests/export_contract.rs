use lu_core::export_contract::{plan_export, ExportError, ExportFormat, ExportRequest};

fn request(format: ExportFormat) -> ExportRequest {
    ExportRequest {
        title: "Lu research report".into(),
        format,
        conversation_id: "thread-1234".into(),
        message_ids: vec!["user-1".into(), "assistant-2".into()],
        source_sha256: "a".repeat(64),
        generated_at_unix_secs: 1_782_600_000,
    }
}

#[test]
fn pdf_docx_and_csv_names_are_deterministic_and_provenance_is_retained() {
    for (format, extension) in [
        (ExportFormat::Pdf, "pdf"),
        (ExportFormat::Docx, "docx"),
        (ExportFormat::Csv, "csv"),
    ] {
        let item = request(format);
        let planned = plan_export(&item).unwrap();
        assert_eq!(
            planned.file_name,
            format!("lu-research-report-1782600000-aaaaaaaaaaaaaaaa.{extension}")
        );
        assert!(
            planned.create_new,
            "the host writer must never overwrite old exports"
        );
        assert_eq!(planned.provenance.conversation_id, item.conversation_id);
        assert_eq!(planned.provenance.message_ids, item.message_ids);
        assert_eq!(planned.provenance.source_sha256, item.source_sha256);
        assert_eq!(planned.provenance.format, format);
        let value = serde_json::to_value(&planned.provenance).unwrap();
        assert_eq!(value["generated_at_unix_secs"], 1_782_600_000u64);
    }
}

#[test]
fn hostile_titles_and_windows_reserved_devices_cannot_become_output_paths() {
    for hostile in [
        "../secret",
        r"C:\Users\name",
        "folder/name",
        "report\0.txt",
        "a..b",
    ] {
        let mut item = request(ExportFormat::Pdf);
        item.title = hostile.into();
        assert_eq!(plan_export(&item), Err(ExportError::UnsafeTitle));
    }
    for reserved in ["CON", "PRN", "AUX", "NUL", "COM1", "LPT9"] {
        let mut item = request(ExportFormat::Csv);
        item.title = reserved.into();
        let result = plan_export(&item).unwrap();
        assert!(result.file_name.starts_with("lu-"));
    }
    let mut item = request(ExportFormat::Docx);
    item.title = "日本語".into();
    assert_eq!(plan_export(&item), Err(ExportError::InvalidTitle));
}

#[test]
fn malformed_provenance_and_duplicate_message_ids_fail_closed() {
    let mut item = request(ExportFormat::Csv);
    item.conversation_id = "../other".into();
    assert_eq!(plan_export(&item), Err(ExportError::InvalidConversation));
    item.conversation_id = "thread-1234".into();
    item.message_ids.push("user-1".into());
    assert_eq!(plan_export(&item), Err(ExportError::InvalidMessages));
    item.message_ids = vec!["user-1".into()];
    item.source_sha256 = "NOT-A-DIGEST".into();
    assert_eq!(plan_export(&item), Err(ExportError::InvalidDigest));
    item.source_sha256 = "A".repeat(64);
    item.generated_at_unix_secs = 0;
    assert_eq!(plan_export(&item), Err(ExportError::InvalidTimestamp));
}

#[test]
fn files_with_distinct_source_digests_get_distinct_names() {
    let first = request(ExportFormat::Pdf);
    let mut second = first.clone();
    second.source_sha256 = "b".repeat(64);
    assert_ne!(
        plan_export(&first).unwrap().file_name,
        plan_export(&second).unwrap().file_name
    );
}
