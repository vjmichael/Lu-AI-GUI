use lu_core::native_csv_edit::{plan_csv_cell_update,CsvEditError};
use lu_core::edit_safety::{bind_reviewed_edit,verify_reviewed_edit};
use sha2::{Sha256,Digest};
fn hash(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
#[test]
fn quoted_csv_can_replace_one_cell_and_roundtrip_into_reviewed_edit(){
    let source=b"\"name\",\"count\"\r\n\"apple, red\",\"3\"\r\n\"quoted \"\"name\"\"\",\"4\"\r\n";
    let sha=hash(source);
    let (preview,plan)=plan_csv_cell_update(
        "report.csv",source,&sha,2,2,"7"
    ).unwrap();
    assert_eq!(preview.change.original_value,"3");
    assert_eq!(preview.change.replacement_value,"7");
    assert!(preview.full_diff_review_required);
    assert!(String::from_utf8_lossy(&preview.replacement_bytes)
        .contains("\"apple, red\",\"7\""));
    assert!(String::from_utf8_lossy(&preview.replacement_bytes)
        .contains("\"quoted \"\"name\"\"\",\"4\""));
    let approved=bind_reviewed_edit(&plan,"Documents/report.csv",
        source,&preview.replacement_bytes).unwrap();
    verify_reviewed_edit(&plan,&approved,"Documents/report.csv",
        source,&preview.replacement_bytes,&approved.approval_sha256).unwrap();
    assert!(verify_reviewed_edit(&plan,&approved,"Documents/report.csv",
        b"tampered",&preview.replacement_bytes,&approved.approval_sha256).is_err());
}
#[test]
fn formula_injection_in_new_cell_becomes_literal_data(){
    let source=b"id,value\r\n1,good\r\n";
    let sha=hash(source);
    let (preview,_)=plan_csv_cell_update(
        "plan.csv",source,&sha,2,2,"=HYPERLINK(\"url\")"
    ).unwrap();
    assert_eq!(preview.change.replacement_value,"'=HYPERLINK(\"url\")");
    assert!(String::from_utf8_lossy(&preview.replacement_bytes)
        .contains("\"'=HYPERLINK(\"\"url\"\")\""));
}
#[test]
fn malformed_csv_hash_mismatch_invalid_range_and_source_are_rejected(){
    let original=b"\"a\",\"b\"\r\n";
    let sha=hash(original);
    assert_eq!(plan_csv_cell_update(
        "test.csv",original,&"0".repeat(64),1,1,"c"
    ).unwrap_err(),CsvEditError::BaselineChanged);
    assert_eq!(plan_csv_cell_update(
        "test.csv",original,&sha,2,1,"c"
    ).unwrap_err(),CsvEditError::InvalidRange);
    assert_eq!(plan_csv_cell_update(
        "../test.csv",original,&sha,1,1,"c"
    ).unwrap_err(),CsvEditError::InvalidName);
    let malformed=b"\"unclosed";
    assert_eq!(plan_csv_cell_update(
        "test.csv",malformed,&hash(malformed),1,1,"c"
    ).unwrap_err(),CsvEditError::MalformedCsv);
    assert_eq!(plan_csv_cell_update(
        "test.csv",original,&sha,1,1,"malicious\0value"
    ).unwrap_err(),CsvEditError::InvalidReplacement);
}
