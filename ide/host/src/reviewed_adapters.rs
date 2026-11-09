//! DEV-03.2B: immutable, native-host-authored non-Rust adapter contracts.
//!
//! This is a PRE-ADMISSION review of a registered project, not an executor.
//! Python, Node and CMake files can execute arbitrary project code. None may
//! receive a process, worker grant, toolchain install, or an unrestricted shell
//! until an independently pinned OS-contained native adapter is accepted.
use crate::IdeWorkspaceRegistry;
use serde::Serialize;
use sha2::{Digest, Sha256};

const REQUIRED_GUARDS: [&str; 8] = [
    "reviewed_immutable_interpreter_bundle",
    "registered_snapshot_sha_verified_at_admission",
    "no_network_or_host_credentials",
    "readonly_source_private_output",
    "preexec_os_containment_and_resource_limits",
    "fresh_single_job_native_os_approval",
    "bounded_stdio_artifact_hashes",
    "descendant_kill_and_durable_recovery_receipt",
];

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterStep {
    pub step_id: &'static str,
    pub tool_id: &'static str,
    pub fixed_args: &'static [&'static str],
    pub input_origin: &'static str,
    pub authority: &'static str,
}

const PYTHON_STEPS: [AdapterStep; 1] = [AdapterStep {
    step_id: "pytest_fixed",
    tool_id: "reviewed_python_interpreter",
    fixed_args: &["-I", "-m", "pytest", "-q", "-p", "no:cacheprovider"],
    input_origin: "sealed_workspace_only",
    authority: "contract_only_not_executable",
}];
const NODE_STEPS: [AdapterStep; 1] = [AdapterStep {
    step_id: "node_builtin_test_fixed",
    tool_id: "reviewed_node_runtime",
    fixed_args: &["--test", "--test-reporter=tap"],
    input_origin: "sealed_workspace_only",
    authority: "contract_only_not_executable",
}];
const CMAKE_STEPS: [AdapterStep; 3] = [
    AdapterStep {
        step_id: "cmake_configure_fixed",
        tool_id: "reviewed_cmake_and_ninja_bundle",
        fixed_args: &[
            "-S",
            "/workspace",
            "-B",
            "/output/build",
            "-G",
            "Ninja",
            "-DCMAKE_BUILD_TYPE=Debug",
        ],
        input_origin: "sealed_workspace_only",
        authority: "contract_only_not_executable",
    },
    AdapterStep {
        step_id: "cmake_build_fixed",
        tool_id: "reviewed_cmake_and_ninja_bundle",
        fixed_args: &["--build", "/output/build", "--parallel", "1"],
        input_origin: "sealed_output_from_prior_step_only",
        authority: "contract_only_not_executable",
    },
    AdapterStep {
        step_id: "ctest_fixed",
        tool_id: "reviewed_ctest_bundle",
        fixed_args: &["--test-dir", "/output/build", "--output-on-failure"],
        input_origin: "sealed_output_from_prior_step_only",
        authority: "contract_only_not_executable",
    },
];

struct FixedAdapter {
    profile_id: &'static str,
    manifest_path: &'static str,
    bundle_id: &'static str,
    steps: &'static [AdapterStep],
}
const ADAPTERS: [FixedAdapter; 3] = [
    FixedAdapter {
        profile_id: "python_pytest",
        manifest_path: "pyproject.toml",
        bundle_id: "python_pytest_bundle_unverified",
        steps: &PYTHON_STEPS,
    },
    FixedAdapter {
        profile_id: "js_tests",
        manifest_path: "package.json",
        bundle_id: "node_builtin_test_bundle_unverified",
        steps: &NODE_STEPS,
    },
    FixedAdapter {
        profile_id: "cpp_cmake_build",
        manifest_path: "CMakeLists.txt",
        bundle_id: "cmake_ninja_ctest_bundle_unverified",
        steps: &CMAKE_STEPS,
    },
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedAdapterContract {
    pub schema_version: u32,
    pub project_id: String,
    pub profile_id: &'static str,
    pub manifest_relative_path: &'static str,
    pub observed_saved_manifest_sha256: String,
    pub observed_clean_git_head: String,
    pub review_fingerprint: String,
    pub toolchain_bundle_id: &'static str,
    pub steps: &'static [AdapterStep],
    pub required_guards: &'static [&'static str],
    pub review_state: &'static str,
    pub snapshot_state: &'static str,
    pub toolchain_state: &'static str,
    pub native_worker_state: &'static str,
    pub execution_authorized: bool,
    pub command_dispatched: bool,
    pub source_text_exported: bool,
    pub installed_app_accepted: bool,
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn valid_git_head(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn fixed_adapter(profile_id: &str) -> Option<&'static FixedAdapter> {
    ADAPTERS.iter().find(|item| item.profile_id == profile_id)
}

impl IdeWorkspaceRegistry {
    /// An explicit UI choice can inspect one of three compiled-in adapter
    /// contracts, with a revision CAS. A review fingerprint is NOT a token.
    /// No native worker reads this object as permission to spawn anything.
    pub fn review_multilang_adapter(
        &self,
        project_id: &str,
        profile_id: &str,
        expected_saved_manifest_sha256: &str,
    ) -> Result<ReviewedAdapterContract, &'static str> {
        let adapter = fixed_adapter(profile_id).ok_or("DEV03_ADAPTER_NOT_REVIEWED")?;
        if !valid_sha256(expected_saved_manifest_sha256) {
            return Err("DEV03_ADAPTER_INVALID_REVISION");
        }

        // No project ID can become a source path. Registry gates active native
        // OS-picker registration, Git root identity, and read-only cap-std IO.
        self.source_capability(project_id)
            .map_err(|_| "DEV03_ADAPTER_PROJECT_UNAVAILABLE")?;
        self.registered_git_root(project_id)
            .map_err(|_| "DEV03_ADAPTER_PROJECT_UNAVAILABLE")?;

        let before = self.inspect_git_worktree(project_id);
        if before.state != "local_repository_inspected"
            || before.worktree != "clean"
            || before.modified_or_staged_count != 0
            || before.untracked_count != 0
            || before.truncated
            || !before.local_head.as_deref().is_some_and(valid_git_head)
        {
            return Err("DEV03_ADAPTER_REQUIRES_CLEAN_GIT_HEAD");
        }
        let manifest = self
            .preview_source_file(project_id, adapter.manifest_path)
            .map_err(|_| "DEV03_ADAPTER_MANIFEST_UNAVAILABLE")?;
        if manifest.project_id != project_id
            || manifest.relative_path != adapter.manifest_path
            || manifest.revision != expected_saved_manifest_sha256
        {
            return Err("DEV03_ADAPTER_SOURCE_CHANGED");
        }

        // A second OS source read prevents silently labeling an edited file as
        // the one inspected. A later worker would STILL have to seal and recheck
        // its own entire source tree before independent authorization.
        let after = self.inspect_git_worktree(project_id);
        let rechecked = self
            .preview_source_file(project_id, adapter.manifest_path)
            .map_err(|_| "DEV03_ADAPTER_SOURCE_CHANGED")?;
        if after.state != before.state
            || after.worktree != "clean"
            || after.local_head != before.local_head
            || after.modified_or_staged_count != 0
            || after.untracked_count != 0
            || after.truncated
            || rechecked.revision != manifest.revision
            || rechecked.project_id != project_id
            || rechecked.relative_path != adapter.manifest_path
        {
            return Err("DEV03_ADAPTER_SOURCE_CHANGED");
        }

        let head = before.local_head.ok_or("DEV03_ADAPTER_SOURCE_CHANGED")?;
        let digest_input = serde_json::to_vec(&(
            "lu.ide.dev03.2b.adapter_review.v1",
            project_id,
            adapter.profile_id,
            adapter.manifest_path,
            &manifest.revision,
            &head,
            adapter.bundle_id,
            adapter.steps,
        ))
        .map_err(|_| "DEV03_ADAPTER_REVIEW_UNAVAILABLE")?;
        let fingerprint = format!("{:x}", Sha256::digest(digest_input));
        Ok(ReviewedAdapterContract {
            schema_version: 1,
            project_id: project_id.to_owned(),
            profile_id: adapter.profile_id,
            manifest_relative_path: adapter.manifest_path,
            observed_saved_manifest_sha256: manifest.revision,
            observed_clean_git_head: head,
            review_fingerprint: fingerprint,
            toolchain_bundle_id: adapter.bundle_id,
            steps: adapter.steps,
            required_guards: &REQUIRED_GUARDS,
            review_state: "native_contract_observed_not_approved",
            snapshot_state: "not_sealed",
            toolchain_state: "not_pinned_or_installed",
            native_worker_state: "no_reviewed_platform_adapter",
            execution_authorized: false,
            command_dispatched: false,
            source_text_exported: false,
            installed_app_accepted: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_three_fixed_restricted_adapter_contracts() {
        assert_eq!(ADAPTERS.len(), 3);
        for adapter in ADAPTERS.iter() {
            assert!(matches!(
                adapter.profile_id,
                "python_pytest" | "js_tests" | "cpp_cmake_build"
            ));
            assert!(!adapter.steps.is_empty());
            assert!(adapter
                .steps
                .iter()
                .all(|step| step.authority == "contract_only_not_executable"));
        }
        assert!(fixed_adapter("rust_build").is_none());
        assert!(fixed_adapter("npm run build").is_none());
        assert!(fixed_adapter("../../outside").is_none());
        assert!(!PYTHON_STEPS[0].fixed_args.contains(&"-c"));
        assert!(!NODE_STEPS[0].fixed_args.contains(&"-e"));
        assert!(CMAKE_STEPS
            .iter()
            .all(|step| !step.fixed_args.contains(&"--install")));
    }
}
