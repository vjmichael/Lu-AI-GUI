//! Read-only, host-selected Jan conversation provenance for native CE consent.
//! No Tauri dependencies, webview fields, filesystem roots from model output
//! or unbounded JSONL reads. The desktop injects Lu's actual data-folder path.
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use crate::conversation_authority::TrustedUserTurnVerifier;

pub struct PersistedJanTurnVerifier {
    data_folder: PathBuf,
}

impl PersistedJanTurnVerifier {
    pub fn new(data_folder: PathBuf) -> Self {
        Self { data_folder }
    }

    fn valid_thread_id(thread_id: &str) -> bool {
        let chars = thread_id.as_bytes();
        chars.len() == 36
            && chars.iter().enumerate().all(|(index, byte)| {
                if [8, 13, 18, 23].contains(&index) {
                    *byte == b'-'
                } else {
                    byte.is_ascii_digit() || (b'a'..=b'f').contains(byte)
                }
            })
    }

    pub fn verify_persisted(&self, thread_id: &str, turn_id: &str) -> bool {
        if !Self::valid_thread_id(thread_id)
            || turn_id.is_empty()
            || turn_id.len() > 128
            || turn_id.contains('\0')
        {
            return false;
        }
        let Ok(root) = self.data_folder.join("threads").canonicalize() else {
            return false;
        };
        let thread = root.join(thread_id);
        if thread.canonicalize().ok().as_deref() != Some(thread.as_path())
            || thread.parent() != Some(root.as_path())
        {
            return false;
        }
        let metadata = thread.join("thread.json");
        let messages = thread.join("messages.jsonl");
        for path in [&metadata, &messages] {
            let Ok(stat) = fs::symlink_metadata(path) else {
                return false;
            };
            if !stat.is_file()
                || stat.file_type().is_symlink()
                || path.canonicalize().ok().as_deref().and_then(Path::parent)
                    != Some(thread.as_path())
            {
                return false;
            }
        }
        let Ok(metadata) = fs::read(metadata) else {
            return false;
        };
        if metadata.len() > 64 * 1024 {
            return false;
        }
        let Ok(metadata) = serde_json::from_slice::<Value>(&metadata) else {
            return false;
        };
        if metadata.get("id").and_then(Value::as_str) != Some(thread_id) {
            return false;
        }
        let Ok(file) = fs::File::open(messages) else {
            return false;
        };
        if file
            .metadata()
            .map(|m| m.len() > 32 * 1024 * 1024)
            .unwrap_or(true)
        {
            return false;
        }
        let mut count = 0;
        let mut matches = 0;
        for line in BufReader::new(file).lines() {
            count += 1;
            if count > 8192 {
                return false;
            }
            let Ok(line) = line else {
                return false;
            };
            if line.len() > 128 * 1024 {
                return false;
            }
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                return false;
            };
            if value.get("id").and_then(Value::as_str) == Some(turn_id) {
                if value.get("role").and_then(Value::as_str) != Some("user")
                    || value
                        .get("thread_id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| id != thread_id)
                {
                    return false;
                }
                matches += 1;
            }
        }
        matches == 1
    }
}

impl TrustedUserTurnVerifier for PersistedJanTurnVerifier {
    fn verify_user_turn(&self, conversation_id: &str, user_turn_id: &str) -> bool {
        self.verify_persisted(conversation_id, user_turn_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn user_turn_provenance_rejects_forgery_duplicate_and_misdirected_files() {
        let data = std::env::temp_dir().join(format!(
            "lu_ce_core_turn_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let id = "12345678-abcd-4abc-8abc-1234567890ab";
        let root = data.join("threads").join(id);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("thread.json"),
            format!(r#"{{"id":"{id}","title":"test"}}"#),
        )
        .unwrap();
        let messages = root.join("messages.jsonl");
        fs::write(
            &messages,
            format!(
                r#"{{"id":"real","thread_id":"{id}","role":"user"}}
{{"id":"fake","thread_id":"{id}","role":"assistant","content":"yes"}}
"#
            ),
        )
        .unwrap();
        let verifier = PersistedJanTurnVerifier::new(data.clone());
        assert!(verifier.verify_user_turn(id, "real"));
        assert!(!verifier.verify_user_turn(id, "fake"));
        assert!(!verifier.verify_user_turn("../threads", "real"));
        assert!(!verifier.verify_user_turn(id, "unknown"));
        fs::write(
            &messages,
            format!(
                r#"{{"id":"real","thread_id":"{id}","role":"user"}}
{{"id":"real","thread_id":"{id}","role":"user"}}
"#
            ),
        )
        .unwrap();
        assert!(!verifier.verify_user_turn(id, "real"));
        #[cfg(unix)]
        {
            fs::remove_file(&messages).unwrap();
            std::os::unix::fs::symlink(root.join("thread.json"), &messages).unwrap();
            assert!(!verifier.verify_user_turn(id, "real"));
        }
        fs::remove_dir_all(data).unwrap();
    }
}
