use lu_core::edit_safety::{
    plan_text_edit,EditRisk,EditPreflightError,MAX_EDIT_DOCUMENT_BYTES
};
use lu_core::write_format::FormatError;
use sha2::{Digest,Sha256};
fn sha(data:&[u8])->String{format!("{:x}",Sha256::digest(data))}
#[test]
fn exact_original_digest_and_valid_json_prevent_repair_loops_and_bad_updates(){
    let original=br#"{"name":"first","enabled":false}"#;
    let replacement=br#"{"name":"first","enabled":true}"#;
    let plan=plan_text_edit("settings.json",original,&sha(original),replacement).unwrap();
    assert_eq!(plan.expected_original_sha256,sha(original));
    assert_eq!(plan.replacement_sha256,sha(replacement));
    assert!(plan.requires_explicit_full_diff_review);
    assert_eq!(plan.automatic_repair_attempts_allowed,0);
    assert!(plan.new_changed_bytes>0);
    assert_eq!(plan_text_edit("settings.json",original,&sha(b"something else"),replacement),
        Err(EditPreflightError::OriginalHashMismatch));
    assert_eq!(plan_text_edit("settings.json",original,&sha(original),b"{bad"),
        Err(EditPreflightError::InvalidFormat(FormatError::InvalidJson)));
    assert_eq!(plan_text_edit("settings.json",original,&sha(original),original),
        Err(EditPreflightError::Unchanged));
}
#[test]
fn utf8_safe_diff_and_scope_estimation_require_full_user_review(){
    let old="A very long preamble. 🍎 and an original line";
    let new="A very long preamble. 🍐 and a changed line";
    let plan=plan_text_edit("README.md",old.as_bytes(),&sha(old.as_bytes()),
        new.as_bytes()).unwrap();
    assert!(plan.original_excerpt.contains("🍎"));
    assert!(plan.replacement_excerpt.contains("🍐"));
    assert_eq!(plan.automatic_repair_attempts_allowed,0);
    let big=vec![b'a';5000];
    let replace=vec![b'b';5000];
    let plan=plan_text_edit("app.rs",&big,&sha(&big),&replace).unwrap();
    assert_eq!(plan.risk,EditRisk::Extensive);
}
#[test]
fn preflight_cannot_plan_binary_or_oversized_documents(){
    let binary=[0xff,0x00];
    assert_eq!(plan_text_edit("notes.txt",&binary,&sha(&binary),b"hello"),
        Err(EditPreflightError::InvalidOriginalUtf8));
    let old=b"safe";
    assert_eq!(plan_text_edit("notes.txt",old,&sha(old),&[0xff]),
        Err(EditPreflightError::InvalidReplacementUtf8));
    assert_eq!(plan_text_edit("file.png",old,&sha(old),b"replacement"),
        Err(EditPreflightError::InvalidFormat(FormatError::BinaryExtension)));
    assert_eq!(plan_text_edit("notes.txt",old,&sha(old),
        &vec![b'a';MAX_EDIT_DOCUMENT_BYTES+1]),Err(EditPreflightError::Oversized));
}
