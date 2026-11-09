//! IDE-05.1: capability-scoped, read-only package-manager identification.
//! A manifest/lockfile is data, NEVER permission to execute its ecosystem.
//! No registry requests, PATH lookup, script evaluation, system installation,
//! package-manager command invocation or project file writes occur here.
use crate::IdeWorkspaceRegistry;
use serde::Serialize;
use sha2::{Digest, Sha256};

const FIXED_FILES: [&str; 6] = [
    "pyproject.toml",
    "uv.lock",
    "vcpkg.json",
    "conanfile.py",
    "conanfile.txt",
    "conan.lock",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFileEvidence {
    pub relative_path: String,
    pub state: &'static str,
    pub sha256: Option<String>,
    pub byte_length: Option<u64>,
}

impl PackageFileEvidence {
    fn hashed(&self) -> bool {
        self.state == "bytes_observed"
            && self
                .sha256
                .as_ref()
                .is_some_and(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
    }
    fn exists_or_unverifiable(&self) -> bool {
        self.state != "missing_in_registered_project"
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageManagerCandidate {
    pub manager_id: &'static str,
    pub ecosystem: &'static str,
    pub state: &'static str,
    pub evidence_paths: Vec<&'static str>,
    pub executable_manifest: bool,
    pub resolution_supported: bool,
    pub apply_supported: bool,
    pub execution_authorized: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageManagerInspection {
    pub schema_version: u32,
    pub project_id: String,
    pub fingerprint: String,
    pub evidence: Vec<PackageFileEvidence>,
    pub candidates: Vec<PackageManagerCandidate>,
    pub blockers: Vec<&'static str>,
    pub inspection_scope: &'static str,
    pub snapshot_consistency: &'static str,
    pub network_accessed: bool,
    pub process_started: bool,
    pub files_modified: bool,
    pub resolution_supported: bool,
    pub apply_supported: bool,
    pub execution_authorized: bool,
    pub release_accepted: bool,
}

fn candidate(
    manager_id: &'static str,
    ecosystem: &'static str,
    state: &'static str,
    paths: &[&'static str],
    executable_manifest: bool,
) -> PackageManagerCandidate {
    PackageManagerCandidate {
        manager_id,
        ecosystem,
        state,
        evidence_paths: paths.to_vec(),
        executable_manifest,
        resolution_supported: false,
        apply_supported: false,
        execution_authorized: false,
    }
}
fn evidence<'a>(all: &'a [PackageFileEvidence], path: &str) -> &'a PackageFileEvidence {
    // All accesses come from the fixed built-in list; not a WebView path.
    all.iter()
        .find(|row| row.relative_path == path)
        .expect("fixed package evidence must exist")
}

impl IdeWorkspaceRegistry {
    /// Inspects ONLY six fixed filenames beneath an already registered,
    /// open project. Uses the existing capability-scoped SHA probe; missing
    /// files are not inferred from a failed I/O operation. No permission is
    /// created by this observation or its fingerprint.
    pub fn inspect_package_managers(
        &self,
        project_id: &str,
    ) -> Result<PackageManagerInspection, &'static str> {
        self.source_capability(project_id)
            .map_err(|_| "IDE05_PROJECT_NOT_OPEN")?;
        let mut files = Vec::with_capacity(FIXED_FILES.len());
        for path in FIXED_FILES {
            let resource = self
                .probe_project_resource(project_id, path)
                .map_err(|_| "IDE05_NATIVE_FILE_PROBE_FAILED")?;
            if resource.project_id != project_id || resource.relative_path != path {
                return Err("IDE05_PROBE_IDENTITY_MISMATCH");
            }
            files.push(PackageFileEvidence {
                relative_path: path.to_owned(),
                state: resource.state,
                sha256: resource.sha256,
                byte_length: resource.byte_length,
            });
        }
        let py = evidence(&files, "pyproject.toml");
        let uv = evidence(&files, "uv.lock");
        let vcpkg = evidence(&files, "vcpkg.json");
        let conan_py = evidence(&files, "conanfile.py");
        let conan_txt = evidence(&files, "conanfile.txt");
        let conan_lock = evidence(&files, "conan.lock");

        let python_present = py.exists_or_unverifiable() || uv.exists_or_unverifiable();
        let cpp_vcpkg = vcpkg.exists_or_unverifiable();
        let cpp_conan = conan_py.exists_or_unverifiable() || conan_txt.exists_or_unverifiable();
        let mut blockers = Vec::new();
        if files
            .iter()
            .any(|row| row.exists_or_unverifiable() && !row.hashed())
        {
            blockers.push("unverifiable_manifest_or_lockfile");
        }
        if cpp_vcpkg && cpp_conan {
            blockers.push("cpp_conflicting_package_managers");
        }
        if conan_py.exists_or_unverifiable() && conan_txt.exists_or_unverifiable() {
            blockers.push("conan_multiple_recipe_manifests");
        }
        if conan_lock.exists_or_unverifiable() && !cpp_conan {
            blockers.push("conan_orphan_lockfile");
        }
        if uv.exists_or_unverifiable() && !py.exists_or_unverifiable() {
            blockers.push("uv_orphan_lockfile");
        }
        let py_state = if !python_present {
            "not_detected"
        } else if !py.hashed() || (uv.exists_or_unverifiable() && !uv.hashed()) {
            "unverifiable"
        } else if uv.hashed() {
            "candidate_needs_review"
        } else {
            "pyproject_present_uv_not_proven"
        };
        let vcpkg_state = if !cpp_vcpkg {
            "not_detected"
        } else if !vcpkg.hashed() {
            "unverifiable"
        } else if cpp_conan {
            "conflicting_cpp_managers"
        } else {
            "candidate_needs_review"
        };
        let conan_state = if !cpp_conan {
            if conan_lock.exists_or_unverifiable() {
                "orphan_lock_without_recipe"
            } else {
                "not_detected"
            }
        } else if cpp_vcpkg
            || (conan_py.exists_or_unverifiable() && conan_txt.exists_or_unverifiable())
        {
            "conflicting_cpp_managers"
        } else if !(conan_py.hashed() || conan_txt.hashed())
            || (conan_lock.exists_or_unverifiable() && !conan_lock.hashed())
        {
            "unverifiable"
        } else {
            "candidate_needs_review"
        };
        let candidates = vec![
            candidate(
                "uv",
                "python",
                py_state,
                &["pyproject.toml", "uv.lock"],
                false,
            ),
            candidate("vcpkg", "cpp", vcpkg_state, &["vcpkg.json"], false),
            candidate(
                "conan2",
                "cpp",
                conan_state,
                &["conanfile.py", "conanfile.txt", "conan.lock"],
                conan_py.exists_or_unverifiable(),
            ),
        ];

        // Stable evidence digest, not an authorization challenge. Different
        // lockfile bytes produce different values; no derived operation rights.
        let mut hash = Sha256::new();
        hash.update(b"lu-ide-package-evidence-v1\n");
        hash.update(project_id.as_bytes());
        hash.update(b"\n");
        for file in &files {
            hash.update(file.relative_path.as_bytes());
            hash.update(b"\n");
            hash.update(file.state.as_bytes());
            hash.update(b"\n");
            hash.update(file.sha256.as_deref().unwrap_or("-").as_bytes());
            hash.update(b"\n");
            hash.update(file.byte_length.unwrap_or(0).to_le_bytes());
        }
        for c in &candidates {
            hash.update(c.manager_id.as_bytes());
            hash.update(c.state.as_bytes());
        }
        Ok(PackageManagerInspection {
            schema_version: 1,
            project_id: project_id.to_owned(),
            fingerprint: format!("{:x}", hash.finalize()),
            evidence: files,
            candidates,
            blockers,
            inspection_scope: "native_registered_project_fixed_manifests_only",
            snapshot_consistency: "per_file_sha_not_atomic_resolve_requires_reinspection",
            network_accessed: false,
            process_started: false,
            files_modified: false,
            resolution_supported: false,
            apply_supported: false,
            execution_authorized: false,
            release_accepted: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, NativeDirectoryPicker};
    use std::{fs, path::PathBuf};

    struct Picker(PathBuf);
    impl NativeDirectoryPicker for Picker {
        fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
            Ok(Some(self.0.clone()))
        }
    }
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Fixture, IdeWorkspaceRegistry, String, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("lu-ide-manager-review-{}", rand::random::<u128>()));
        let project = root.join("picker-selected-project");
        fs::create_dir_all(&project).unwrap();
        let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
        let id = registry
            .register_from_native_picker(&mut Picker(project.clone()))
            .unwrap()
            .unwrap()
            .project_id;
        (Fixture(root), registry, id, project)
    }
    fn state<'a>(result: &'a PackageManagerInspection, manager: &str) -> &'a str {
        result
            .candidates
            .iter()
            .find(|x| x.manager_id == manager)
            .unwrap()
            .state
    }
    #[test]
    fn no_manifests_is_read_only_no_grants() {
        let (_t, reg, id, _p) = fixture();
        let result = reg.inspect_package_managers(&id).unwrap();
        assert_eq!(result.evidence.len(), 6);
        assert!(result
            .candidates
            .iter()
            .all(|c| c.state == "not_detected" && !c.execution_authorized && !c.apply_supported));
        assert!(
            !result.files_modified
                && !result.network_accessed
                && !result.process_started
                && !result.release_accepted
        );
        assert!(result.blockers.is_empty());
        assert_eq!(result.fingerprint.len(), 64);
    }
    #[test]
    fn uv_detection_requires_real_lock_and_revisions_change_on_edit() {
        let (_t, reg, id, p) = fixture();
        fs::write(p.join("pyproject.toml"), b"[project]\nname='example'\n").unwrap();
        let without = reg.inspect_package_managers(&id).unwrap();
        assert_eq!(state(&without, "uv"), "pyproject_present_uv_not_proven");
        fs::write(p.join("uv.lock"), b"version = 1\n").unwrap();
        let before = reg.inspect_package_managers(&id).unwrap();
        assert_eq!(state(&before, "uv"), "candidate_needs_review");
        fs::write(p.join("uv.lock"), b"version = 2\n").unwrap();
        let after = reg.inspect_package_managers(&id).unwrap();
        assert_ne!(before.fingerprint, after.fingerprint);
        assert!(!after.apply_supported && !after.execution_authorized);
    }
    #[test]
    fn cpp_conflict_and_python_script_are_not_executed() {
        let (_t, reg, id, p) = fixture();
        fs::write(p.join("vcpkg.json"), b"{\"dependencies\":[]}").unwrap();
        fs::write(
            p.join("conanfile.py"),
            b"raise RuntimeError('must not run')",
        )
        .unwrap();
        let before = fs::read(p.join("conanfile.py")).unwrap();
        let result = reg.inspect_package_managers(&id).unwrap();
        assert!(result
            .blockers
            .contains(&"cpp_conflicting_package_managers"));
        assert_eq!(state(&result, "vcpkg"), "conflicting_cpp_managers");
        assert_eq!(state(&result, "conan2"), "conflicting_cpp_managers");
        assert!(
            result
                .candidates
                .iter()
                .find(|c| c.manager_id == "conan2")
                .unwrap()
                .executable_manifest
        );
        assert_eq!(fs::read(p.join("conanfile.py")).unwrap(), before);
        assert!(!result.process_started && !result.apply_supported);
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_lockfile_is_unverifiable_and_not_selected() {
        use std::os::unix::fs::symlink;
        let (_t, reg, id, p) = fixture();
        fs::write(p.join("pyproject.toml"), b"[project]\nname='test'\n").unwrap();
        let external = p.parent().unwrap().join("external.lock");
        fs::write(&external, b"host-secret-outside-project").unwrap();
        symlink(external, p.join("uv.lock")).unwrap();
        let result = reg.inspect_package_managers(&id).unwrap();
        assert_eq!(state(&result, "uv"), "unverifiable");
        assert!(result
            .blockers
            .contains(&"unverifiable_manifest_or_lockfile"));
        assert!(result
            .evidence
            .iter()
            .find(|f| f.relative_path == "uv.lock")
            .unwrap()
            .sha256
            .is_none());
    }
    #[test]
    fn conan_text_manifest_is_not_a_python_recipe_or_manager_permission() {
        let (_t, reg, id, p) = fixture();
        fs::write(p.join("conanfile.txt"), b"[requires]\nfmt/11.0\n").unwrap();
        let result = reg.inspect_package_managers(&id).unwrap();
        assert_eq!(state(&result, "conan2"), "candidate_needs_review");
        assert!(
            !result
                .candidates
                .iter()
                .find(|c| c.manager_id == "conan2")
                .unwrap()
                .executable_manifest
        );
        assert!(!result.resolution_supported && !result.execution_authorized);
    }
}
