//! IDE-05.2: native, source-revision-bound package operation *preview*.
//! Never executes an ecosystem manager, reads project secrets, resolves
//! transitive versions, writes a lockfile, or creates an approval grant.
use crate::{
    package_manager::{PackageFileEvidence, PackageManagerInspection},
    IdeWorkspaceRegistry,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageOperation {
    Add,
    Remove,
    Sync,
}

/// Fixed, typed operation. There are no path, URL, flags, environment,
/// repository, script, package index or executable arguments in this contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePreviewRequest {
    pub manager_id: String,
    pub operation: PackageOperation,
    pub package_name: Option<String>,
    pub exact_version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageProposedChange {
    pub target_relative_path: &'static str,
    pub intended_action: &'static str,
    pub requested_declaration: Option<String>,
    pub observed_direct_declaration: &'static str,
    pub currently_observed_sha256: Option<String>,
    pub expected_result_sha256: Option<String>,
    pub actual_diff_computed: bool,
    pub requires_protected_resolver: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageOperationPreview {
    pub schema_version: u32,
    pub project_id: String,
    pub request: PackagePreviewRequest,
    pub evidence_fingerprint: String,
    pub preview_sha256: String,
    pub evidence: Vec<PackageFileEvidence>,
    pub manager_selection_state: &'static str,
    pub proposed_changes: Vec<PackageProposedChange>,
    pub risks: Vec<&'static str>,
    pub blockers: Vec<&'static str>,
    pub snapshot_consistency: &'static str,
    pub dependency_resolution: &'static str,
    pub ready_for_human_review: bool,
    pub ready_for_approval: bool,
    pub approval_issued: bool,
    pub process_started: bool,
    pub network_accessed: bool,
    pub files_modified: bool,
    pub apply_supported: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePreviewRevalidation {
    pub schema_version: u32,
    pub project_id: String,
    pub preview_sha256: String,
    pub evidence_fingerprint: String,
    pub matched_fresh_native_evidence: bool,
    pub ready_for_human_review: bool,
    pub ready_for_approval: bool,
    pub authorization_issued: bool,
    pub process_started: bool,
    pub files_modified: bool,
}

fn valid_atom(text: &str, max_bytes: usize, first_digit_allowed: bool) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() || bytes.len() > max_bytes || text != text.trim() {
        return false;
    }
    let first = bytes[0];
    if !(first.is_ascii_alphabetic() || (first_digit_allowed && first.is_ascii_digit())) {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(*b, b'-' | b'_' | b'.' | b'+'))
        && !text.contains("..")
        && !text.starts_with('-')
        && !text.ends_with('.')
}

/// Not a general dependency-expression parser. Constraints, URLs, extras,
/// shell words and version ranges are forbidden in a static exact-version
/// preview; a real manager may later require additional supported syntax.
fn validate(request: &PackagePreviewRequest) -> Result<(), &'static str> {
    if !matches!(request.manager_id.as_str(), "uv" | "vcpkg" | "conan2") {
        return Err("IDE052_MANAGER_NOT_SUPPORTED");
    }
    match request.operation {
        PackageOperation::Add => {
            let name = request
                .package_name
                .as_deref()
                .ok_or("IDE052_PACKAGE_NAME_REQUIRED")?;
            let version = request
                .exact_version
                .as_deref()
                .ok_or("IDE052_EXACT_VERSION_REQUIRED")?;
            if !valid_atom(name, 96, true) || !valid_atom(version, 64, true) {
                return Err("IDE052_PACKAGE_OR_VERSION_UNSAFE");
            }
        }
        PackageOperation::Remove => {
            let name = request
                .package_name
                .as_deref()
                .ok_or("IDE052_PACKAGE_NAME_REQUIRED")?;
            if !valid_atom(name, 96, true) || request.exact_version.is_some() {
                return Err("IDE052_REMOVE_REQUIREMENT_INVALID");
            }
        }
        PackageOperation::Sync => {
            if request.package_name.is_some() || request.exact_version.is_some() {
                return Err("IDE052_SYNC_TAKES_NO_PACKAGE_ARGUMENTS");
            }
        }
    }
    Ok(())
}
fn path<'a>(inspection: &'a PackageManagerInspection, name: &str) -> &'a PackageFileEvidence {
    inspection
        .evidence
        .iter()
        .find(|e| e.relative_path == name)
        .expect("IDE05 fixed evidence path must be present")
}
fn selected_manifest(inspection: &PackageManagerInspection, manager: &str) -> &'static str {
    match manager {
        "uv" => "pyproject.toml",
        "vcpkg" => "vcpkg.json",
        "conan2" if path(inspection, "conanfile.py").sha256.is_some() => "conanfile.py",
        "conan2" => "conanfile.txt",
        _ => unreachable!("validated manager"),
    }
}
fn lock_path(manager: &str) -> Option<&'static str> {
    match manager {
        "uv" => Some("uv.lock"),
        "conan2" => Some("conan.lock"),
        "vcpkg" => None, // no vcpkg lockfile is in the fixed v1 scan
        _ => None,
    }
}
fn version_requirement(request: &PackagePreviewRequest) -> Option<String> {
    match request.operation {
        PackageOperation::Add => Some(format!(
            "{} (requested exact version {}; manifest syntax not computed)",
            request.package_name.as_deref().unwrap_or_default(),
            request.exact_version.as_deref().unwrap_or_default(),
        )),
        PackageOperation::Remove => request
            .package_name
            .as_ref()
            .map(|name| format!("remove:{name}")),
        PackageOperation::Sync => None,
    }
}
/// Parse only static vcpkg JSON declaration names. This is not a vcpkg
/// version resolver and does not infer the registry baseline or hooks.
/// The file must still match the SHA from the native registered-root probe.
fn inspect_vcpkg_direct_declaration(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
    expected_sha: &str,
    name: &str,
) -> Result<&'static str, &'static str> {
    let snapshot = registry
        .preview_source_file(project_id, "vcpkg.json")
        .map_err(|_| "IDE052_VCPKG_MANIFEST_UNREADABLE")?;
    if snapshot.project_id != project_id
        || snapshot.relative_path != "vcpkg.json"
        || snapshot.revision != expected_sha
    {
        return Err("IDE052_VCPKG_CHANGED_DURING_PREVIEW");
    }
    let json: serde_json::Value =
        serde_json::from_str(&snapshot.text).map_err(|_| "IDE052_VCPKG_JSON_INVALID")?;
    let object = json.as_object().ok_or("IDE052_VCPKG_JSON_INVALID")?;
    let mut found = false;
    if let Some(deps) = object.get("dependencies") {
        let array = deps
            .as_array()
            .ok_or("IDE052_VCPKG_DEPENDENCIES_UNSUPPORTED")?;
        if array.len() > 4096 {
            return Err("IDE052_VCPKG_DEPENDENCIES_OVER_LIMIT");
        }
        for dep in array {
            let declared = if let Some(text) = dep.as_str() {
                text
            } else if let Some(obj) = dep.as_object() {
                obj.get("name")
                    .and_then(serde_json::Value::as_str)
                    .ok_or("IDE052_VCPKG_DEPENDENCIES_UNSUPPORTED")?
            } else {
                return Err("IDE052_VCPKG_DEPENDENCIES_UNSUPPORTED");
            };
            // Treat package identity as literal; no normalization, guessing
            // or fallback to another manager's matching package name.
            if declared == name {
                // A conditional/feature-qualified declaration cannot be
                // treated as an unconditional dependency by a static preview.
                // Do not execute vcpkg or infer platform/feature evaluation.
                if dep.as_object().is_some_and(|obj| obj.len() != 1) {
                    return Err("IDE052_VCPKG_CONDITIONAL_DECLARATION_UNRESOLVED");
                }
                if found {
                    return Err("IDE052_VCPKG_DUPLICATE_DECLARATION");
                }
                found = true;
            }
        }
    }
    Ok(if name.is_empty() {
        "not_applicable_to_sync"
    } else if found {
        "observed_declared"
    } else {
        "observed_absent"
    })
}
fn create_preview(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
    request: PackagePreviewRequest,
    inspection: PackageManagerInspection,
) -> Result<PackageOperationPreview, &'static str> {
    validate(&request)?;
    let candidate = inspection
        .candidates
        .iter()
        .find(|c| c.manager_id == request.manager_id)
        .ok_or("IDE052_MANAGER_NOT_SUPPORTED")?;
    // A UI-provided manager ID can SELECT a compatible candidate, not
    // override native detection or resolve a conflicting manager choice.
    if candidate.state != "candidate_needs_review" {
        return Err("IDE052_MANAGER_NOT_DETECTED_OR_AMBIGUOUS");
    }
    if !inspection.blockers.is_empty() {
        return Err("IDE052_MANIFEST_OR_MANAGER_BLOCKED");
    }
    let manager_selection_state = candidate.state;
    let target = selected_manifest(&inspection, &request.manager_id);
    let manifest = path(&inspection, target);
    if manifest.state != "bytes_observed" || manifest.sha256.is_none() {
        return Err("IDE052_MANIFEST_NOT_VERIFIED");
    }
    let mut risks = vec![
        "untrusted_package_manager_and_install_hooks_must_not_execute_during_review",
        "registry_source_and_network_egress_not_verified",
        "transitive_dependencies_and_licenses_not_resolved",
        "target_toolchain_and_environment_not_verified",
        "manifest_and_lockfile_must_be_revalidated_before_native_approval",
    ];
    if request.manager_id == "uv" {
        risks.push("python_build_backend_may_execute_code");
    }
    if request.manager_id == "vcpkg" {
        risks.push("vcpkg_registry_baseline_and_native_build_hooks_unverified");
    }
    if request.manager_id == "conan2" {
        risks.push("conan_recipe_or_dependencies_may_execute_code");
        risks.push("conan_major_version_not_verified_by_static_files");
    }
    if matches!(request.operation, PackageOperation::Remove) {
        risks.push("removal_may_break_reverse_dependencies");
    }
    if matches!(request.operation, PackageOperation::Sync) {
        risks.push("sync_may_modify_environment_outside_manifest");
    }
    let observed_direct_declaration = if request.manager_id == "vcpkg" {
        // Sync is not an exemption from parsing the actual manifest: a
        // correctly hashed but malformed JSON file is not a valid preview.
        // Empty name means validate all static dependency entry shapes,
        // without claiming any single package was resolved.
        inspect_vcpkg_direct_declaration(
            registry,
            project_id,
            manifest
                .sha256
                .as_deref()
                .ok_or("IDE052_MANIFEST_NOT_VERIFIED")?,
            request.package_name.as_deref().unwrap_or(""),
        )?
    } else {
        "not_parsed_by_static_preview"
    };
    if request.manager_id != "vcpkg" {
        risks.push("manifest_direct_declaration_not_parsed");
    } else {
        if request.operation == PackageOperation::Add
            && observed_direct_declaration == "observed_declared"
        {
            return Err("IDE052_DEPENDENCY_ALREADY_DECLARED");
        }
        if request.operation == PackageOperation::Remove
            && observed_direct_declaration == "observed_absent"
        {
            return Err("IDE052_DEPENDENCY_NOT_DECLARED");
        }
    }
    let intended_action = match request.operation {
        PackageOperation::Add => "propose_direct_dependency_addition",
        PackageOperation::Remove => "propose_direct_dependency_removal",
        PackageOperation::Sync => "propose_environment_reconciliation",
    };
    let mut proposed = vec![PackageProposedChange {
        target_relative_path: target,
        intended_action,
        requested_declaration: version_requirement(&request),
        observed_direct_declaration,
        currently_observed_sha256: manifest.sha256.clone(),
        expected_result_sha256: None,
        actual_diff_computed: false,
        requires_protected_resolver: true,
    }];
    if let Some(lock) = lock_path(&request.manager_id) {
        proposed.push(PackageProposedChange {
            target_relative_path: lock,
            intended_action: "possible_transitive_lockfile_update",
            requested_declaration: None,
            observed_direct_declaration: "transitive_resolution_not_performed",
            currently_observed_sha256: path(&inspection, lock).sha256.clone(),
            expected_result_sha256: None,
            actual_diff_computed: false,
            requires_protected_resolver: true,
        });
    }
    let mut hash = Sha256::new();
    hash.update(b"lu-ide-package-intent-preview-v1\n");
    hash.update(project_id.as_bytes());
    hash.update(b"\n");
    hash.update(inspection.fingerprint.as_bytes());
    hash.update(b"\n");
    hash.update(serde_json::to_vec(&request).map_err(|_| "IDE052_PREVIEW_SERIALIZATION_FAILED")?);
    hash.update(b"\n");
    hash.update(serde_json::to_vec(&proposed).map_err(|_| "IDE052_PREVIEW_SERIALIZATION_FAILED")?);
    hash.update(b"\n");
    hash.update(serde_json::to_vec(&risks).map_err(|_| "IDE052_PREVIEW_SERIALIZATION_FAILED")?);
    Ok(PackageOperationPreview {
        schema_version: 1,
        project_id: project_id.to_owned(),
        request,
        evidence_fingerprint: inspection.fingerprint,
        preview_sha256: format!("{:x}", hash.finalize()),
        evidence: inspection.evidence,
        manager_selection_state,
        proposed_changes: proposed,
        risks,
        blockers: inspection.blockers,
        snapshot_consistency: "non_atomic_individual_file_sha_must_revalidate",
        dependency_resolution: "static_intent_only_no_transitive_resolution_or_actual_diff",
        ready_for_human_review: true,
        ready_for_approval: false,
        approval_issued: false,
        process_started: false,
        network_accessed: false,
        files_modified: false,
        apply_supported: false,
    })
}
impl IdeWorkspaceRegistry {
    /// A static preview never creates a grant, even after valid file hashes.
    pub fn preview_package_operation(
        &self,
        project_id: &str,
        request: PackagePreviewRequest,
    ) -> Result<PackageOperationPreview, &'static str> {
        validate(&request)?;
        let inspection = self.inspect_package_managers(project_id)?;
        create_preview(self, project_id, request, inspection)
    }

    /// Re-inspects every fixed source/lock file natively. A changed file,
    /// competing manager, different operation or forged digest fails closed.
    /// The caller cannot claim revalidation by supplying an old inspection.
    /// Matching review evidence still never creates execution authority.
    pub fn revalidate_package_operation(
        &self,
        project_id: &str,
        request: PackagePreviewRequest,
        expected_preview_sha256: &str,
        expected_evidence_fingerprint: &str,
    ) -> Result<PackagePreviewRevalidation, &'static str> {
        if !hex64(expected_preview_sha256) || !hex64(expected_evidence_fingerprint) {
            return Err("IDE052_REVALIDATION_DIGEST_INVALID");
        }
        let fresh = self.preview_package_operation(project_id, request)?;
        if fresh.preview_sha256 != expected_preview_sha256
            || fresh.evidence_fingerprint != expected_evidence_fingerprint
        {
            return Err("IDE052_PREVIEW_STALE_REVIEW_AGAIN");
        }
        Ok(PackagePreviewRevalidation {
            schema_version: 1,
            project_id: project_id.to_owned(),
            preview_sha256: fresh.preview_sha256,
            evidence_fingerprint: fresh.evidence_fingerprint,
            matched_fresh_native_evidence: true,
            ready_for_human_review: fresh.ready_for_human_review,
            ready_for_approval: false,
            authorization_issued: false,
            process_started: false,
            files_modified: false,
        })
    }
}
fn hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
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
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Temp, IdeWorkspaceRegistry, String, PathBuf) {
        let root = std::env::temp_dir().join(format!("ide052-{}", rand::random::<u128>()));
        let project = root.join("picked_project");
        fs::create_dir_all(&project).unwrap();
        let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
        let id = registry
            .register_from_native_picker(&mut Picker(project.clone()))
            .unwrap()
            .unwrap()
            .project_id;
        (Temp(root), registry, id, project)
    }
    fn request(manager: &str, operation: PackageOperation) -> PackagePreviewRequest {
        PackagePreviewRequest {
            manager_id: manager.into(),
            operation,
            package_name: Some("example-lib".into()),
            exact_version: if operation == PackageOperation::Add {
                Some("1.2.3".into())
            } else {
                None
            },
        }
    }
    fn uv_files(project: &std::path::Path) {
        fs::write(
            project.join("pyproject.toml"),
            b"[project]\nname = 'test'\n",
        )
        .unwrap();
        fs::write(project.join("uv.lock"), b"version = 1\n").unwrap();
    }
    #[test]
    fn stable_preview_and_revalidation_do_not_grant_execution() {
        let (_t, registry, id, path) = fixture();
        uv_files(&path);
        let r = request("uv", PackageOperation::Add);
        let first = registry.preview_package_operation(&id, r.clone()).unwrap();
        let second = registry.preview_package_operation(&id, r.clone()).unwrap();
        assert_eq!(first.preview_sha256, second.preview_sha256);
        assert_eq!(first.proposed_changes.len(), 2);
        assert_eq!(
            first.proposed_changes[0].requested_declaration.as_deref(),
            Some("example-lib (requested exact version 1.2.3; manifest syntax not computed)")
        );
        assert!(
            !first.ready_for_approval
                && !first.approval_issued
                && !first.apply_supported
                && !first.files_modified
        );
        assert!(first
            .proposed_changes
            .iter()
            .all(|c| !c.actual_diff_computed
                && c.expected_result_sha256.is_none()
                && c.requires_protected_resolver));
        let verified = registry
            .revalidate_package_operation(
                &id,
                r,
                &first.preview_sha256,
                &first.evidence_fingerprint,
            )
            .unwrap();
        assert!(verified.matched_fresh_native_evidence);
        assert!(!verified.authorization_issued && !verified.ready_for_approval);
    }
    #[test]
    fn source_and_lockfile_drift_fail_closed() {
        let (_t, registry, id, path) = fixture();
        uv_files(&path);
        let r = request("uv", PackageOperation::Add);
        let first = registry.preview_package_operation(&id, r.clone()).unwrap();
        fs::write(path.join("uv.lock"), b"version = 2\n").unwrap();
        assert!(matches!(
            registry.revalidate_package_operation(
                &id,
                r.clone(),
                &first.preview_sha256,
                &first.evidence_fingerprint,
            ),
            Err("IDE052_PREVIEW_STALE_REVIEW_AGAIN")
        ));
        let second = registry.preview_package_operation(&id, r.clone()).unwrap();
        fs::write(path.join("pyproject.toml"), b"[project]\nname='changed'\n").unwrap();
        assert!(registry
            .revalidate_package_operation(
                &id,
                r,
                &second.preview_sha256,
                &second.evidence_fingerprint,
            )
            .is_err());
    }
    #[test]
    fn manager_choice_cannot_override_detected_conflict() {
        let (_t, registry, id, path) = fixture();
        fs::write(path.join("vcpkg.json"), b"{\"dependencies\":[]}").unwrap();
        fs::write(
            path.join("conanfile.py"),
            b"raise Exception('execute forbidden')",
        )
        .unwrap();
        for manager in ["vcpkg", "conan2"] {
            assert!(matches!(
                registry.preview_package_operation(&id, request(manager, PackageOperation::Add)),
                Err("IDE052_MANAGER_NOT_DETECTED_OR_AMBIGUOUS")
            ));
        }
        assert!(registry
            .preview_package_operation(&id, request("npm", PackageOperation::Add))
            .is_err());
    }
    #[test]
    fn injection_and_ambiguous_arguments_are_denied() {
        for name in [
            "../../ssh",
            "--help",
            "foo/bar",
            "name;rm -rf",
            "name@evil",
            "name space",
            "http://registry",
        ] {
            let mut r = request("uv", PackageOperation::Add);
            r.package_name = Some(name.to_owned());
            assert!(validate(&r).is_err(), "{name}");
        }
        for version in ["^1.2", ">=1.0", "1;echo", "--script", "x/y", "1 2", ""] {
            let mut r = request("vcpkg", PackageOperation::Add);
            r.exact_version = Some(version.to_owned());
            assert!(validate(&r).is_err(), "{version}");
        }
        let mut r = request("uv", PackageOperation::Remove);
        r.exact_version = Some("1.0".into());
        assert!(validate(&r).is_err());
        let mut r = request("uv", PackageOperation::Sync);
        assert!(validate(&r).is_err());
        r.package_name = None;
        assert!(validate(&r).is_ok());
        let extra = r#"{"managerId":"uv","operation":"sync",
            "packageName":null,"exactVersion":null,"shell":"bash"}"#;
        assert!(serde_json::from_str::<PackagePreviewRequest>(extra).is_err());
    }
    #[test]
    fn operation_changes_identity_and_cannot_be_reused() {
        let (_t, registry, id, path) = fixture();
        uv_files(&path);
        let added = registry
            .preview_package_operation(&id, request("uv", PackageOperation::Add))
            .unwrap();
        let removed = request("uv", PackageOperation::Remove);
        assert!(registry
            .revalidate_package_operation(
                &id,
                removed,
                &added.preview_sha256,
                &added.evidence_fingerprint,
            )
            .is_err());
        let mut bad = request("uv", PackageOperation::Add);
        bad.exact_version = Some("2.0".into());
        assert!(registry
            .revalidate_package_operation(
                &id,
                bad,
                &added.preview_sha256,
                &added.evidence_fingerprint,
            )
            .is_err());
    }
    #[test]
    fn manager_binds_only_to_registered_project() {
        let (_t, registry, id, path) = fixture();
        uv_files(&path);
        assert!(registry
            .preview_package_operation(
                "ide-00000000000000000000000000000000",
                request("uv", PackageOperation::Add),
            )
            .is_err());
        let first = registry
            .preview_package_operation(&id, request("uv", PackageOperation::Add))
            .unwrap();
        assert!(registry
            .revalidate_package_operation(
                "ide-00000000000000000000000000000000",
                request("uv", PackageOperation::Add),
                &first.preview_sha256,
                &first.evidence_fingerprint
            )
            .is_err());
    }

    #[test]
    fn vcpkg_json_exact_declaration_is_observed_but_no_install_is_approved() {
        let (_t, registry, id, dir) = fixture();
        fs::write(
            dir.join("vcpkg.json"),
            br#"{"name":"test","dependencies":["fmt",{"name":"zlib","features":["core"]}]}"#,
        )
        .unwrap();
        let found = registry
            .preview_package_operation(
                &id,
                PackagePreviewRequest {
                    manager_id: "vcpkg".into(),
                    operation: PackageOperation::Remove,
                    package_name: Some("fmt".into()),
                    exact_version: None,
                },
            )
            .unwrap();
        assert_eq!(
            found.proposed_changes[0].observed_direct_declaration,
            "observed_declared"
        );
        assert!(!found.ready_for_approval);
        let absent = registry
            .preview_package_operation(&id, request("vcpkg", PackageOperation::Add))
            .unwrap();
        assert_eq!(
            absent.proposed_changes[0].observed_direct_declaration,
            "observed_absent"
        );
        assert_eq!(absent.proposed_changes[0].expected_result_sha256, None);
    }

    #[test]
    fn vcpkg_duplicate_add_and_absent_removal_fail_closed() {
        let (_t, registry, id, dir) = fixture();
        fs::write(
            dir.join("vcpkg.json"),
            br#"{"dependencies":["example-lib"]}"#,
        )
        .unwrap();
        assert!(matches!(
            registry.preview_package_operation(&id, request("vcpkg", PackageOperation::Add)),
            Err("IDE052_DEPENDENCY_ALREADY_DECLARED")
        ));
        let mut absent = request("vcpkg", PackageOperation::Remove);
        absent.package_name = Some("unlisted".into());
        assert!(matches!(
            registry.preview_package_operation(&id, absent),
            Err("IDE052_DEPENDENCY_NOT_DECLARED")
        ));
    }
    #[test]
    fn conditional_or_duplicate_vcpkg_declarations_are_not_falsely_resolved() {
        let (_t, registry, id, dir) = fixture();
        let mut remove = request("vcpkg", PackageOperation::Remove);
        remove.package_name = Some("fmt".into());
        for data in [
            br#"{"dependencies":[{"name":"fmt","platform":"windows"}]}"#.as_slice(),
            br#"{"dependencies":[{"name":"fmt","features":["core"]}]}"#.as_slice(),
        ] {
            fs::write(dir.join("vcpkg.json"), data).unwrap();
            assert!(matches!(
                registry.preview_package_operation(&id, remove.clone()),
                Err("IDE052_VCPKG_CONDITIONAL_DECLARATION_UNRESOLVED")
            ));
        }
        fs::write(dir.join("vcpkg.json"), br#"{"dependencies":["fmt","fmt"]}"#).unwrap();
        assert!(matches!(
            registry.preview_package_operation(&id, remove),
            Err("IDE052_VCPKG_DUPLICATE_DECLARATION")
        ));
    }
    #[test]
    fn vcpkg_sync_rejects_malformed_manifest_without_invoking_manager() {
        let (_t, registry, id, dir) = fixture();
        // Sync intentionally accepts no package name or version. The
        // generic Add/Remove request fixture supplies a package name.
        let mut sync = request("vcpkg", PackageOperation::Sync);
        sync.package_name = None;
        assert!(matches!(
            registry.preview_package_operation(&id, request("vcpkg", PackageOperation::Sync)),
            Err("IDE052_SYNC_TAKES_NO_PACKAGE_ARGUMENTS")
        ));
        fs::write(dir.join("vcpkg.json"), b"{not-json").unwrap();
        assert!(matches!(
            registry.preview_package_operation(&id, sync.clone()),
            Err("IDE052_VCPKG_JSON_INVALID")
        ));
        fs::write(dir.join("vcpkg.json"), br#"{"dependencies":[123]}"#).unwrap();
        assert!(matches!(
            registry.preview_package_operation(&id, sync.clone()),
            Err("IDE052_VCPKG_DEPENDENCIES_UNSUPPORTED")
        ));
        fs::write(dir.join("vcpkg.json"), br#"{"dependencies":["fmt"]}"#).unwrap();
        let safe = registry.preview_package_operation(&id, sync).unwrap();
        assert_eq!(
            safe.proposed_changes[0].observed_direct_declaration,
            "not_applicable_to_sync"
        );
        assert!(!safe.ready_for_approval && !safe.process_started);
    }
    #[test]
    fn malformed_vcpkg_fails_closed_without_forged_diff() {
        let (_t, registry, id, dir) = fixture();
        fs::write(dir.join("vcpkg.json"), b"{invalid").unwrap();
        assert!(matches!(
            registry.preview_package_operation(&id, request("vcpkg", PackageOperation::Add)),
            Err("IDE052_VCPKG_JSON_INVALID")
        ));
        fs::write(dir.join("vcpkg.json"), br#"{"dependencies":["fmt",7]}"#).unwrap();
        assert!(matches!(
            registry.preview_package_operation(&id, request("vcpkg", PackageOperation::Add)),
            Err("IDE052_VCPKG_DEPENDENCIES_UNSUPPORTED")
        ));
    }
    #[test]
    fn conan_python_recipe_is_data_only() {
        let (_t, registry, id, path) = fixture();
        fs::write(
            path.join("conanfile.py"),
            b"raise RuntimeError('must_not_run')",
        )
        .unwrap();
        let before = fs::read(path.join("conanfile.py")).unwrap();
        let preview = registry
            .preview_package_operation(&id, request("conan2", PackageOperation::Add))
            .unwrap();
        assert!(preview
            .risks
            .contains(&"conan_recipe_or_dependencies_may_execute_code"));
        assert_eq!(
            preview.proposed_changes[0].target_relative_path,
            "conanfile.py"
        );
        assert!(!preview.process_started && !preview.apply_supported);
        assert_eq!(before, fs::read(path.join("conanfile.py")).unwrap());
    }
    #[cfg(unix)]
    #[test]
    fn linked_lockfile_cannot_be_previewed_or_revalidated() {
        use std::os::unix::fs::symlink;
        let (_t, registry, id, path) = fixture();
        uv_files(&path);
        let r = request("uv", PackageOperation::Add);
        let original = registry.preview_package_operation(&id, r.clone()).unwrap();
        fs::remove_file(path.join("uv.lock")).unwrap();
        let outside = path.parent().unwrap().join("outside.lock");
        fs::write(&outside, b"secret").unwrap();
        symlink(outside, path.join("uv.lock")).unwrap();
        assert!(registry
            .revalidate_package_operation(
                &id,
                r,
                &original.preview_sha256,
                &original.evidence_fingerprint,
            )
            .is_err());
    }
}
