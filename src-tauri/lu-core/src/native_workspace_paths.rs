//! Lu-owned native tool workspace addressing.
//!
//! This module deliberately retains the existing on-disk directory layout
//! during migration away from Jan/Cowork workspace helper calls. This
//! compatibility is *data layout only*, never execution or authority.
//! The trusted host still must verify every actual ancestor, Windows reparse
//! point, directory ownership, and each saved USER turn before dispatch.
//!
//! Keep this module independent of Tauri and inherited agent-tool plugins.
use std::path::{Path, PathBuf};

const STORE: &str = "agent-workspace";
const THREADS: &str = "threads";
const SESSIONS: &str = "sessions";

/// Stable legacy-on-disk layout. Renaming requires an explicit, atomic,
/// independently tested profile migration. Never silently move user files.
pub fn permanent_store(data_folder: &Path) -> PathBuf {
    data_folder.join(STORE)
}
pub fn threads_dir(data_folder: &Path) -> PathBuf {
    permanent_store(data_folder).join(THREADS)
}
pub fn sessions_dir(data_folder: &Path) -> PathBuf {
    permanent_store(data_folder).join(SESSIONS)
}

/// This is the compatibility validation used in the legacy workspace
/// addressing code. Authorization never follows from a valid segment.
pub fn workspace_segment(id: &str) -> Result<&str, &'static str> {
    let segment = id.trim();
    if segment.is_empty()
        || segment.len() > 128
        || !segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || segment.bytes().all(|byte| byte == b'.')
    {
        return Err("invalid Lu workspace segment");
    }
    Ok(segment)
}
pub fn thread_workspace(data_folder: &Path, id: &str) -> Result<PathBuf, &'static str> {
    Ok(threads_dir(data_folder).join(workspace_segment(id)?))
}
pub fn session_workspace(data_folder: &Path, id: &str) -> Result<PathBuf, &'static str> {
    Ok(sessions_dir(data_folder).join(workspace_segment(id)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_existing_profile_paths_without_touching_fs() {
        let profile = Path::new("Lu-Profile");
        assert_eq!(permanent_store(profile), profile.join("agent-workspace"));
        assert_eq!(
            threads_dir(profile),
            profile.join("agent-workspace/threads")
        );
        assert_eq!(
            sessions_dir(profile),
            profile.join("agent-workspace/sessions")
        );
        assert_eq!(
            thread_workspace(profile, "abc-123").unwrap(),
            profile.join("agent-workspace/threads/abc-123")
        );
        assert_eq!(
            session_workspace(profile, "abc-123").unwrap(),
            profile.join("agent-workspace/sessions/abc-123")
        );
        assert_ne!(
            thread_workspace(profile, "abc-123").unwrap(),
            session_workspace(profile, "abc-123").unwrap()
        );
    }
    #[test]
    fn rejects_traversal_separators_and_unsafe_ids() {
        for invalid in [
            "",
            "  ",
            ".",
            "..",
            "...",
            "../escape",
            "a/b",
            "a\\b",
            "a:b",
            "C:\\",
            "/absolute",
            "a b",
            "a;b",
            "a$b",
            "évil",
            "evil\0name",
            "prefix/../suffix",
        ] {
            assert!(workspace_segment(invalid).is_err(), "accepted {invalid:?}");
            assert!(thread_workspace(Path::new("home"), invalid).is_err());
            assert!(session_workspace(Path::new("home"), invalid).is_err());
        }
        assert!(workspace_segment(&"a".repeat(129)).is_err());
    }
    #[test]
    fn compatible_with_persisted_thread_and_session_identifiers() {
        for accepted in [
            "01H8XGJWBWBAQ",
            "a",
            "abc-123",
            "f4f5d739-10d9-493b-aa10-0dfccb5d8e18",
            "a.b",
            "a_b",
            "  abc-123  ",
        ] {
            let checked = workspace_segment(accepted).unwrap();
            assert_eq!(checked, accepted.trim());
            assert!(thread_workspace(Path::new("p"), accepted)
                .unwrap()
                .starts_with(threads_dir(Path::new("p"))));
        }
    }
    #[test]
    fn does_not_read_or_write_any_profile_data() {
        let nonexistent = std::env::temp_dir().join(format!(
            "lu-uncreated-{}-{}",
            std::process::id(),
            "workspaces"
        ));
        let dir = thread_workspace(&nonexistent, "thread1").unwrap();
        assert_eq!(dir, nonexistent.join("agent-workspace/threads/thread1"));
        // Path construction cannot create directories, grants, or sandbox
        // state. Directory creation and canonicalization remain host owned.
    }
}
