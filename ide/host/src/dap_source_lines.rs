//! DEV-04.5D stage 1: bounded read-only source-coordinate evidence.
//!
//! A DAP source path is untrusted adapter output. These functions may only
//! review a previously OS-contained, independently correlated stack frame
//! against a native host's exact source file SHA. They NEVER confer debugger
//! launch rights, breakpoint/step/evaluate/variable access or DWARF/PDB
//! semantic line-table certification.
use crate::dap_session::DapError;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path},
};

const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 2048;
const MAX_SOURCE_LINE: u64 = 1_000_000;
const MAX_FRAME_ID: u64 = 1_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLineCandidate {
    pub frame_id: u64,
    pub line: u32,
    pub column: Option<u32>,
    // Never print or forward to WebView: this path is adapter-controlled.
    pub adapter_path: String,
    // This is the adapter's *claimed* runtime PC. It is not independently
    // load-bias/module-identity verified and cannot confer debugger authority.
    pub instruction_address: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLineReview {
    pub relative_source: String,
    pub file_sha256: String,
    pub line: u32,
    pub column: Option<u32>,
    pub frame_id: u64,
    // Source coordinates + exact bytes do NOT validate actual debug line
    // tables, inlined functions, instruction addresses or PDB type data.
    pub debug_symbol_line_table_verified: bool,
    pub debugger_execution_authorized: bool,
    pub installed_app_accepted: bool,
}

fn valid_absolute_source_path(path: &str) -> bool {
    if path.is_empty() || path.len() > MAX_PATH_BYTES
        || path.chars().any(char::is_control)
    {
        return false;
    }
    let bytes = path.as_bytes();
    let unix = path.starts_with('/') && !path.starts_with("//");
    let windows = bytes.len() >= 4 && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':' && bytes[2] == b'\\';
    if !(unix || windows) || path.starts_with("\\\\") {
        return false;
    }
    !path.split(['/', '\\']).any(|component| component == "." || component == "..")
}

/// Extract only a nonvirtual, local coordinate from an already-correlated
/// stackTrace frame. No untrusted sourceReference requests are permitted.
pub fn candidate_from_dap_frame(
    frame: &Value,
) -> Result<Option<SourceLineCandidate>, DapError> {
    let source = match frame.get("source") {
        Some(Value::Object(v)) => v,
        None => return Ok(None),
        Some(_) => return Err(DapError::InvalidMessage),
    };
    let reference = match source.get("sourceReference") {
        None => 0,
        Some(v) => v.as_u64().ok_or(DapError::InvalidMessage)?,
    };
    if reference != 0 {
        return Ok(None); // virtual source is not a local file
    }
    let path = match source.get("path") {
        Some(v) => v.as_str().ok_or(DapError::InvalidMessage)?,
        None => return Ok(None),
    };
    if !valid_absolute_source_path(path) {
        return Err(DapError::InvalidMessage);
    }
    let frame_id = frame.get("id").and_then(Value::as_u64)
        .filter(|n| *n > 0 && *n <= MAX_FRAME_ID)
        .ok_or(DapError::InvalidMessage)?;
    let line = frame.get("line").and_then(Value::as_u64)
        .filter(|n| *n > 0 && *n <= MAX_SOURCE_LINE)
        .ok_or(DapError::InvalidMessage)?;
    let column = match frame.get("column") {
        Some(v) => Some(
            u32::try_from(v.as_u64().ok_or(DapError::InvalidMessage)?)
                .map_err(|_| DapError::InvalidMessage)?
        ),
        None => None,
    };
    if column.is_some_and(|value| value == 0 || value > 4096) {
        return Err(DapError::InvalidMessage);
    }
    let instruction_address = match frame.get("instructionPointerReference") {
        None => None,
        Some(Value::String(v)) => {
            let hex = v.strip_prefix("0x").ok_or(DapError::InvalidMessage)?;
            if hex.is_empty() || hex.len() > 16
                || !hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(DapError::InvalidMessage);
            }
            Some(
                u64::from_str_radix(hex, 16)
                    .map_err(|_| DapError::InvalidMessage)?
            )
        }
        Some(_) => return Err(DapError::InvalidMessage),
    };
    if instruction_address == Some(0) {
        return Err(DapError::InvalidMessage);
    }
    Ok(Some(SourceLineCandidate {
        frame_id,
        line: line as u32,
        column,
        adapter_path: path.to_owned(),
        instruction_address,
    }))
}

fn regular_file(meta: &fs::Metadata) -> bool {
    if !meta.is_file() || meta.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return false; // reparse point
        }
    }
    true
}
fn regular_dir(meta: &fs::Metadata) -> bool {
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}

/// Native-only file check. The host, NOT DAP/WebView, selects the project
/// root, exact relative source and independently pinned file digest.
/// A matching line within that immutable file is *not* DWARF/PDB proof.
pub fn review_native_source_line(
    root: &Path,
    relative_source: &str,
    pinned_file_sha256: &str,
    candidate: &SourceLineCandidate,
) -> Result<SourceLineReview, DapError> {
    if !root.is_absolute()
        || !regular_dir(&fs::symlink_metadata(root).map_err(|_| DapError::InvalidMessage)?)
        || relative_source.is_empty()
        || relative_source.len() > 512
        || relative_source.contains('\\')
        || !relative_source.bytes().all(|b| b.is_ascii_graphic())
        || pinned_file_sha256.len() != 64
        || !pinned_file_sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(DapError::InvalidMessage);
    }
    let relative = Path::new(relative_source);
    if !relative.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(DapError::InvalidMessage);
    }
    let mut parent = root.to_path_buf();
    if let Some(parts) = relative.parent() {
        for component in parts.components() {
            let Component::Normal(part) = component else {
                return Err(DapError::InvalidMessage);
            };
            parent.push(part);
            if !regular_dir(&fs::symlink_metadata(&parent)
                .map_err(|_| DapError::InvalidMessage)?) {
                return Err(DapError::InvalidMessage);
            }
        }
    }
    let exact = root.join(relative);
    // Deliberate exact native-source match: no basename/suffix lookup, path
    // canonicalization shortcut or alternate source supplied by an adapter.
    if exact.to_str() != Some(candidate.adapter_path.as_str()) {
        return Err(DapError::InvalidMessage);
    }
    let before = fs::symlink_metadata(&exact).map_err(|_| DapError::InvalidMessage)?;
    if !regular_file(&before) || before.len() == 0 || before.len() > MAX_SOURCE_BYTES {
        return Err(DapError::ExcessiveData);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.nlink() != 1 {
            return Err(DapError::InvalidMessage);
        }
    }
    let bytes = fs::read(&exact).map_err(|_| DapError::InvalidMessage)?;
    let after = fs::symlink_metadata(&exact).map_err(|_| DapError::InvalidMessage)?;
    if bytes.len() as u64 != before.len()
        || bytes.len() as u64 > MAX_SOURCE_BYTES
        || !regular_file(&after)
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        return Err(DapError::InvalidMessage);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != after.dev() || before.ino() != after.ino()
            || after.nlink() != 1 {
            return Err(DapError::InvalidMessage);
        }
    }
    let hash = format!("{:x}", Sha256::digest(&bytes));
    if hash != pinned_file_sha256 {
        return Err(DapError::InvalidMessage);
    }
    let source = std::str::from_utf8(&bytes).map_err(|_| DapError::InvalidMessage)?;
    let count = source.split('\n').count();
    if candidate.line == 0 || candidate.line as usize > count {
        return Err(DapError::InvalidMessage);
    }
    Ok(SourceLineReview {
        relative_source: relative_source.to_owned(),
        file_sha256: hash,
        line: candidate.line,
        column: candidate.column,
        frame_id: candidate.frame_id,
        debug_symbol_line_table_verified: false,
        debugger_execution_authorized: false,
        installed_app_accepted: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_absolute_local_nonvirtual_source_coordinates_are_candidates() {
        let valid = json!({"id": 7, "line": 3, "column": 2,
            "source": {"path": "/project/src/main.rs", "sourceReference": 0}});
        assert_eq!(candidate_from_dap_frame(&valid).unwrap().unwrap().line, 3);
        assert!(candidate_from_dap_frame(
            &json!({"id": 7, "line": 3, "source": {"sourceReference": 99}})
        ).unwrap().is_none());
        assert!(candidate_from_dap_frame(
            &json!({"id": 7, "line": 3})
        ).unwrap().is_none());
        for path in ["/project/../private.rs", "//server/source.rs", "src/main.rs",
                     "/tmp/./main.rs", "/tmp/secret\u{0}name.rs",
                     r"C:\project\..\other.rs"] {
            let forged = json!({"id": 7, "line": 3,
                "source": {"path": path, "sourceReference": 0}});
            assert_eq!(candidate_from_dap_frame(&forged),
                Err(DapError::InvalidMessage));
        }
    }

    #[test]
    fn read_only_native_source_digest_and_line_bound_are_both_required() {
        let root = std::env::temp_dir().join(format!(
            "dev045d-source-{:032x}", rand::random::<u128>()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let file = root.join("src/main.rs");
        let text = b"fn main() {\n  println!(\"hello\");\n}\n";
        fs::write(&file, text).unwrap();
        let hash = format!("{:x}", Sha256::digest(text));
        let candidate = SourceLineCandidate {
            frame_id: 8, line: 2, column: Some(3),
            adapter_path: file.to_str().unwrap().to_owned(),
            instruction_address: Some(0x401110),
        };
        let reviewed = review_native_source_line(&root, "src/main.rs", &hash, &candidate)
            .unwrap();
        assert_eq!(reviewed.relative_source, "src/main.rs");
        assert_eq!(reviewed.line, 2);
        assert!(!reviewed.debug_symbol_line_table_verified);
        assert!(!reviewed.debugger_execution_authorized);
        let mut forged = candidate.clone();
        forged.line = 100;
        assert!(review_native_source_line(&root, "src/main.rs", &hash, &forged).is_err());
        forged = candidate.clone();
        forged.adapter_path = root.join("src/other.rs").display().to_string();
        assert!(review_native_source_line(&root, "src/main.rs", &hash, &forged).is_err());
        assert!(review_native_source_line(&root, "../src/main.rs", &hash, &candidate).is_err());
        assert!(review_native_source_line(&root, "src/main.rs", &"0".repeat(64), &candidate).is_err());
        fs::write(&file, b"fn main() { changed }\n").unwrap();
        assert!(review_native_source_line(&root, "src/main.rs", &hash, &candidate).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_source_is_denied_even_if_both_files_have_identical_bytes() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!(
            "dev045d-link-{:032x}", rand::random::<u128>()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("real.rs"), b"fn main() {}\n").unwrap();
        symlink("../real.rs", root.join("src/main.rs")).unwrap();
        let candidate = SourceLineCandidate {
            frame_id: 1, line: 1, column: None,
            adapter_path: root.join("src/main.rs").display().to_string(),
            instruction_address: Some(0x401110),
        };
        let hash = format!("{:x}", Sha256::digest(b"fn main() {}\n"));
        assert!(review_native_source_line(&root, "src/main.rs", &hash, &candidate).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn stopped_instruction_pointer_claim_is_strictly_hex_or_absent() {
        let frame = |value: Value| {
            serde_json::json!({"id": 11, "line": 2,
                "source": {"path": "/project/src/main.rs", "sourceReference": 0},
                "instructionPointerReference": value})
        };
        assert_eq!(
            candidate_from_dap_frame(&frame(serde_json::json!("0x401110")))
                .unwrap().unwrap().instruction_address,
            Some(0x401110)
        );
        for bad in ["0", "401110", "0x0", "0x", "0xg", "0x12345678901234567",
                    "0x123\n45", "0x../src"] {
            assert_eq!(
                candidate_from_dap_frame(&frame(serde_json::json!(bad))),
                Err(DapError::InvalidMessage)
            );
        }
        assert_eq!(
            candidate_from_dap_frame(&frame(serde_json::json!(123))),
            Err(DapError::InvalidMessage)
        );
        assert!(candidate_from_dap_frame(&serde_json::json!({
            "id": 1, "line": 1, "source": {"path": "/project/src/main.rs"}
        })).unwrap().unwrap().instruction_address.is_none());
    }
}
