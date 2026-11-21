//! DEV-04.1: compile-time fixed source-to-debug-target PREPARATION ONLY.
//! A clean Git HEAD is NOT a binary identity. This review cannot launch,
//! locate, trust or sign an executable; no project file may supply a command.
use crate::IdeWorkspaceRegistry;
use serde::Serialize;
use sha2::{Digest, Sha256};

const MANIFEST: &str = "Cargo.toml";
const DEBUG_PROFILE: &str = "rust_cargo_debug_candidate";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugTargetReview {
    pub schema_version: u32,
    pub project_id: String,
    pub profile_id: &'static str,
    pub manifest_relative_path: &'static str,
    pub manifest_sha256: String,
    pub clean_git_head: String,
    pub review_fingerprint: String,
    pub source_snapshot_state: &'static str,
    pub build_receipt_state: &'static str,
    pub binary_identity_state: &'static str,
    pub debug_symbols_state: &'static str,
    pub debugger_adapter_state: &'static str,
    pub target_executable: Option<String>,
    pub source_text_exported: bool,
    pub binary_executed: bool,
    pub worker_grant_issued: bool,
    pub debugger_authorized: bool,
    pub installed_app_accepted: bool,
}

fn valid_sha(value: &str, count: usize) -> bool {
    value.len() == count
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
impl IdeWorkspaceRegistry {
    /// Native registered project + exact saved manifest CAS + clean Git.
    /// A second observation prevents stale review results; a future
    /// executable runner must STILL independently seal and attest a binary.
    pub fn review_project_debug_target(
        &self,
        project_id: &str,
        expected_manifest_sha256: &str,
    ) -> Result<DebugTargetReview, &'static str> {
        if !valid_sha(expected_manifest_sha256, 64) {
            return Err("DEV041_MANIFEST_SHA_INVALID");
        }
        self.source_capability(project_id)
            .map_err(|_| "DEV041_PROJECT_NOT_ACTIVE")?;
        self.registered_git_root(project_id)
            .map_err(|_| "DEV041_PROJECT_NOT_ACTIVE")?;
        let before = self.inspect_git_worktree(project_id);
        if before.state != "local_repository_inspected"
            || before.worktree != "clean"
            || before.truncated
            || before.modified_or_staged_count != 0
            || before.untracked_count != 0
            || !before
                .local_head
                .as_deref()
                .is_some_and(|sha| valid_sha(sha, 40))
        {
            return Err("DEV041_CLEAN_GIT_REQUIRED");
        }
        let saved = self
            .preview_source_file(project_id, MANIFEST)
            .map_err(|_| "DEV041_MANIFEST_UNREADABLE")?;
        if saved.project_id != project_id
            || saved.relative_path != MANIFEST
            || saved.revision != expected_manifest_sha256
        {
            return Err("DEV041_STALE_MANIFEST");
        }
        let after = self.inspect_git_worktree(project_id);
        let manifest = self
            .preview_source_file(project_id, MANIFEST)
            .map_err(|_| "DEV041_STALE_MANIFEST")?;
        if after.state != before.state
            || after.worktree != "clean"
            || after.local_head != before.local_head
            || after.truncated
            || after.modified_or_staged_count != 0
            || after.untracked_count != 0
            || manifest.project_id != project_id
            || manifest.relative_path != MANIFEST
            || manifest.revision != saved.revision
        {
            return Err("DEV041_SOURCE_CHANGED");
        }
        let head = before.local_head.ok_or("DEV041_SOURCE_CHANGED")?;
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    "lu.ide.dev04.1.debug_review.v1",
                    project_id,
                    DEBUG_PROFILE,
                    MANIFEST,
                    &saved.revision,
                    &head,
                ))
                .map_err(|_| "DEV041_REVIEW_UNAVAILABLE")?
            )
        );
        Ok(DebugTargetReview {
            schema_version: 1,
            project_id: project_id.to_owned(),
            profile_id: DEBUG_PROFILE,
            manifest_relative_path: MANIFEST,
            manifest_sha256: saved.revision,
            clean_git_head: head,
            review_fingerprint: fingerprint,
            source_snapshot_state: "not_sealed",
            build_receipt_state: "no_completed_native_build_bound",
            binary_identity_state: "no_binary_sha_or_provenance",
            debug_symbols_state: "not_attested",
            debugger_adapter_state: "fixed_self_test_only_no_project_target",
            target_executable: None,
            source_text_exported: false,
            binary_executed: false,
            worker_grant_issued: false,
            debugger_authorized: false,
            installed_app_accepted: false,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_canonical_sha_lengths() {
        assert!(valid_sha(&"a".repeat(40), 40));
        assert!(valid_sha(&"b".repeat(64), 64));
        assert!(!valid_sha(&"A".repeat(64), 64));
        assert!(!valid_sha("../../target/debug/evil", 64));
    }
}
