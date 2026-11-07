//! WO9: multi-project release/workflow planning from trusted local evidence.
//! No paths, commands, arbitrary executables, Git mutations or Lu authority
//! cross the WebView boundary. A preview/hash is never a release certificate.
use crate::IdeWorkspaceRegistry;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs::{self, File, OpenOptions}, io::Write, path::Path};

const MAX_WORKFLOW_PROJECTS: usize = 8;
const MAX_ARTIFACTS_PER_PROJECT: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowArtifact {
    pub relative_path: String,
    pub category: String,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowProject {
    pub project_id: String,
    pub display_name: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub worktree: String,
    pub artifacts: Vec<WorkflowArtifact>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowInspection {
    pub schema_version: u32,
    pub kind: String,
    pub projects: Vec<WorkflowProject>,
    pub fingerprint: String,
    pub ready_for_review: bool,
    pub ready_for_release: bool,
    pub live_remote_verified: bool,
    pub execution_performed: bool,
    pub package_created: bool,
}

/// Exported manifest is documentation of observed inputs, not an artifact
/// bundle, a signoff or an authorization to execute package scripts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowManifestExport {
    pub schema_version: u32,
    pub file_name: String,
    pub sha256: String,
    pub byte_length: usize,
    pub project_count: usize,
    pub source_fingerprint: String,
    pub project_files_exported: bool,
    pub release_approved: bool,
    pub execution_performed: bool,
}

fn fingerprint(projects: &[WorkflowProject]) -> Result<String, &'static str> {
    let bytes = serde_json::to_vec(projects).map_err(|_| "WORKFLOW_SERIALIZE_FAILED")?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn workflow_status(projects: Vec<WorkflowProject>) -> Result<WorkflowInspection, &'static str> {
    let hash = fingerprint(&projects)?;
    let ready_for_review = projects.iter().all(|item| item.blockers.is_empty());
    Ok(WorkflowInspection {
        schema_version: 1,
        kind: "multi_project_release_review".into(),
        projects,
        fingerprint: hash,
        ready_for_review,
        // Explicit signed installed acceptance + authenticated GitHub receipts
        // are never available from this local-only inspection.
        ready_for_release: false,
        live_remote_verified: false,
        execution_performed: false,
        package_created: false,
    })
}

impl IdeWorkspaceRegistry {
    /// Gather a bounded deterministic project-scoped release review. Only
    /// project IDs selected from the trusted picker registry may be supplied.
    /// Files are inspected through existing capability-based inventory scans;
    /// Git is read via the existing no-subprocess libgit2 inspection.
    pub fn inspect_multi_project_workflow(
        &self,
        project_ids: &[String],
    ) -> Result<WorkflowInspection, &'static str> {
        if project_ids.is_empty() || project_ids.len() > MAX_WORKFLOW_PROJECTS {
            return Err("WORKFLOW_PROJECT_COUNT_DENIED");
        }
        let mut unique = BTreeSet::new();
        let catalog = self.projects().map_err(|_| "WORKFLOW_CATALOG_UNAVAILABLE")?;
        let mut out = Vec::with_capacity(project_ids.len());
        for project_id in project_ids {
            if !unique.insert(project_id.clone()) {
                return Err("WORKFLOW_DUPLICATE_PROJECT");
            }
            let entry = catalog.iter().find(|p| &p.project_id == project_id)
                .ok_or("WORKFLOW_UNKNOWN_PROJECT")?;
            if !entry.active || !entry.available {
                return Err("WORKFLOW_PROJECT_NOT_OPEN");
            }
            let git = self.inspect_git_worktree(project_id);
            let inventory = self.inspect_project_evidence(project_id)
                .map_err(|_| "WORKFLOW_INVENTORY_UNAVAILABLE")?;
            let mut blockers = Vec::new();
            if git.state != "local_repository_inspected"
                || git.local_head.as_deref().is_none_or(|h| h.len() != 40
                    || !h.as_bytes().iter().all(u8::is_ascii_hexdigit)) {
                blockers.push("git_head_unverified".into());
            }
            if git.worktree != "clean" || git.truncated {
                blockers.push("dirty_or_truncated_git_worktree".into());
            }
            if !inventory.discovery_complete {
                blockers.push("artifact_inventory_truncated".into());
            }
            // Include only independently hashed regular files. Never package
            // a symlink, an oversized/unhashed candidate, or a source path
            // provided by JavaScript.
            let mut candidates = Vec::new();
            for item in inventory.files {
                if item.digest_status != "sha256_observed" || item.sha256.is_none() {
                    blockers.push("unhashed_or_unverifiable_artifact".into());
                    continue;
                }
                if candidates.len() == MAX_ARTIFACTS_PER_PROJECT {
                    blockers.push("artifact_candidate_limit".into());
                    break;
                }
                candidates.push(WorkflowArtifact {
                    relative_path: item.relative_path,
                    category: item.category.to_owned(),
                    byte_length: item.size_bytes,
                    sha256: item.sha256.expect("verified above"),
                });
            }
            if candidates.is_empty() {
                blockers.push("no_verified_packaging_artifacts".into());
            }
            candidates.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
            blockers.sort();
            blockers.dedup();
            out.push(WorkflowProject {
                project_id: project_id.clone(),
                display_name: entry.display_name.clone(),
                branch: git.branch,
                head: git.local_head,
                worktree: git.worktree.to_owned(),
                artifacts: candidates,
                blockers,
            });
        }
        workflow_status(out)
    }

    /// A *native OS folder chooser* supplies destination; no WebView path.
    /// Export a descriptive, immutable source manifest, not an executable
    /// archive. Existing files are never overwritten and the user's project
    /// directory cannot silently become dirty from this export.
    pub fn export_workflow_manifest(
        &self,
        project_ids: &[String],
        expected_fingerprint: &str,
        selected_folder: &Path,
    ) -> Result<WorkflowManifestExport, &'static str> {
        let current = self.verify_multi_project_workflow(project_ids, expected_fingerprint)?;
        let metadata = fs::symlink_metadata(selected_folder)
            .map_err(|_| "WORKFLOW_EXPORT_FOLDER_UNAVAILABLE")?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("WORKFLOW_EXPORT_FOLDER_UNTRUSTED");
        }
        let folder = fs::canonicalize(selected_folder)
            .map_err(|_| "WORKFLOW_EXPORT_FOLDER_UNAVAILABLE")?;
        if !folder.is_dir() {
            return Err("WORKFLOW_EXPORT_FOLDER_UNAVAILABLE");
        }
        for id in project_ids {
            let root = self.registered_git_root(id)
                .map_err(|_| "WORKFLOW_PROJECT_NOT_OPEN")?;
            if folder.starts_with(&root) {
                return Err("WORKFLOW_EXPORT_INSIDE_SOURCE_PROJECT");
            }
        }
        let file_name = format!("lu-ide-workflow-{}.json", &current.fingerprint[..20]);
        let output = folder.join(&file_name);
        let bytes = serde_json::to_vec_pretty(&current)
            .map_err(|_| "WORKFLOW_EXPORT_SERIALIZE_FAILED")?;
        if bytes.len() > 256 * 1024 {
            return Err("WORKFLOW_EXPORT_TOO_LARGE");
        }
        // A preexisting file, including a symlink/reparse point, always denies.
        let mut file = OpenOptions::new().write(true).create_new(true).open(&output)
            .map_err(|_| "WORKFLOW_EXPORT_ALREADY_EXISTS_OR_DENIED")?;
        let result = (|| {
            file.write_all(&bytes)
                .map_err(|_| "WORKFLOW_EXPORT_WRITE_FAILED")?;
            file.sync_all().map_err(|_| "WORKFLOW_EXPORT_SYNC_FAILED")?;
            drop(file);
            // Exact Git/artifact re-check *after* the write: changes while the
            // manifest was being saved cannot silently become accepted.
            self.verify_multi_project_workflow(project_ids, expected_fingerprint)?;
            #[cfg(unix)]
            File::open(&folder).and_then(|dir| dir.sync_all())
                .map_err(|_| "WORKFLOW_EXPORT_DIRECTORY_SYNC_FAILED")?;
            Ok::<(), &'static str>(())
        })();
        if let Err(code) = result {
            let _ = fs::remove_file(&output);
            return Err(code);
        }
        Ok(WorkflowManifestExport {
            schema_version: 1,
            file_name,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            byte_length: bytes.len(),
            project_count: project_ids.len(),
            source_fingerprint: current.fingerprint,
            project_files_exported: false,
            release_approved: false,
            execution_performed: false,
        })
    }

    /// TOCTOU guard. A stale plan never permits promotion, packaging or Git
    /// mutation; the caller must obtain a fresh review on changed evidence.
    pub fn verify_multi_project_workflow(
        &self,
        project_ids: &[String],
        expected_fingerprint: &str,
    ) -> Result<WorkflowInspection, &'static str> {
        if expected_fingerprint.len() != 64
            || !expected_fingerprint.as_bytes().iter().all(u8::is_ascii_hexdigit)
        {
            return Err("WORKFLOW_INVALID_FINGERPRINT");
        }
        let current = self.inspect_multi_project_workflow(project_ids)?;
        if !current.fingerprint.eq_ignore_ascii_case(expected_fingerprint) {
            return Err("WORKFLOW_SOURCE_OR_ARTIFACT_DRIFT");
        }
        if !current.ready_for_review {
            return Err("WORKFLOW_PREVIEW_BLOCKED");
        }
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn project(name: &str) -> WorkflowProject {
        WorkflowProject {
            project_id: name.into(),
            display_name: name.into(),
            branch: Some("main".into()),
            head: Some("a".repeat(40)),
            worktree: "clean".into(),
            artifacts: vec![WorkflowArtifact {
                relative_path: "dist/package.zip".into(),
                category: "archive".into(),
                byte_length: 5,
                sha256: "b".repeat(64),
            }],
            blockers: vec![],
        }
    }
    struct FakePicker(std::path::PathBuf);
    impl crate::NativeDirectoryPicker for FakePicker {
        fn choose_project_directory(&mut self) -> Result<Option<std::path::PathBuf>, crate::IdeError> {
            Ok(Some(self.0.clone()))
        }
    }

    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }

    fn native_fixture() -> (Fixture, IdeWorkspaceRegistry, String) {
        use git2::{Repository, Signature};
        let root = std::env::temp_dir().join(format!(
            "wo9-native-workflow-{}", rand::random::<u64>()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let project = root.join("picked-by-native-dialog");
        std::fs::create_dir_all(project.join("dist")).unwrap();
        std::fs::write(project.join("dist").join("verified.tar"), b"frozen archive data").unwrap();
        let repo = Repository::init(&project).unwrap();
        let mut idx = repo.index().unwrap();
        idx.add_path(std::path::Path::new("dist/verified.tar")).unwrap();
        idx.write().unwrap();
        let tree_id = idx.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let who = Signature::now("WO9 test", "test@localhost.invalid").unwrap();
        repo.commit(Some("HEAD"), &who, &who, "fixed fixture", &tree, &[]).unwrap();
        drop(tree);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
        let project_id = registry.register_from_native_picker(&mut FakePicker(project))
            .unwrap().unwrap().project_id;
        (Fixture(root), registry, project_id)
    }

    #[test]
    fn native_registered_project_freezes_real_git_and_artifact_digests() {
        let (_temp, registry, project_id) = native_fixture();
        let selected = vec![project_id.clone()];
        let plan = registry.inspect_multi_project_workflow(&selected).unwrap();
        assert!(plan.ready_for_review, "{:?}", plan.projects[0].blockers);
        assert_eq!(plan.projects[0].artifacts.len(), 1);
        assert_eq!(plan.projects[0].head.as_deref().map(str::len), Some(40));
        assert!(registry.verify_multi_project_workflow(&selected, &plan.fingerprint).is_ok());
        assert!(registry.inspect_multi_project_workflow(
            &[project_id.clone(), project_id],
        ).is_err());
        assert!(!plan.ready_for_release);
        assert!(!plan.execution_performed);
    }

    #[test]
    fn native_export_is_new_only_and_source_drift_denied() {
        let (temp, registry, project_id) = native_fixture();
        let selected = vec![project_id.clone()];
        let plan = registry.inspect_multi_project_workflow(&selected).unwrap();
        let destination = temp.0.join("exported-by-native-picker");
        std::fs::create_dir_all(&destination).unwrap();
        let report = registry.export_workflow_manifest(
            &selected, &plan.fingerprint, &destination,
        ).unwrap();
        assert!(destination.join(&report.file_name).exists());
        assert!(!report.project_files_exported);
        assert!(!report.release_approved);
        assert!(registry.export_workflow_manifest(
            &selected, &plan.fingerprint, &destination,
        ).is_err(), "overwrite of a reviewed plan must be denied");
        let registered_root = registry.trusted_native_worker_root(&project_id).unwrap();
        assert!(registry.export_workflow_manifest(
            &selected, &plan.fingerprint, &registered_root,
        ).is_err(), "export must not silently dirty a project root");
        std::fs::write(registered_root.join("dist/verified.tar"), b"changed externally").unwrap();
        assert_eq!(
            registry.verify_multi_project_workflow(&selected, &plan.fingerprint)
                .unwrap_err(),
            "WORKFLOW_SOURCE_OR_ARTIFACT_DRIFT",
        );
        assert!(registry.export_workflow_manifest(
            &selected, &plan.fingerprint, &destination,
        ).is_err());
    }

    #[test]
    fn deterministic_multi_project_plan_not_a_release() {
        let a = workflow_status(vec![project("a"), project("b")]).unwrap();
        let b = workflow_status(vec![project("a"), project("b")]).unwrap();
        assert_eq!(a.fingerprint, b.fingerprint);
        assert!(a.ready_for_review);
        assert!(!a.ready_for_release);
        assert!(!a.package_created);
        assert!(!a.execution_performed);
        assert!(!a.live_remote_verified);
    }
    #[test]
    fn stale_artifact_or_git_head_changes_fingerprint() {
        let a = workflow_status(vec![project("a")]).unwrap();
        let mut drift = project("a");
        drift.artifacts[0].sha256 = "c".repeat(64);
        assert_ne!(a.fingerprint, workflow_status(vec![drift.clone()]).unwrap().fingerprint);
        drift.artifacts[0].sha256 = "b".repeat(64);
        drift.head = Some("d".repeat(40));
        assert_ne!(a.fingerprint, workflow_status(vec![drift]).unwrap().fingerprint);
    }
    #[test]
    fn block_unreviewable_state_without_fabricating_release_acceptance() {
        let mut p = project("blocked");
        p.blockers.push("dirty_or_truncated_git_worktree".into());
        let result = workflow_status(vec![p]).unwrap();
        assert!(!result.ready_for_review);
        assert!(!result.ready_for_release);
    }
}
