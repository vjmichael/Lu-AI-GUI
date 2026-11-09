import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const host = read('../../host/src/package_preview.rs');
const registry = read('../../host/src/lib.rs');
const tauri = read('../src-tauri/src/lib.rs');
const ui = read('../web/main.jsx');

test('IDE-05.2 only accepts typed fixed operations and literal manager identities',()=>{
  assert.match(host, /enum PackageOperation \{[\s\S]*?Add,[\s\S]*?Remove,[\s\S]*?Sync,/);
  assert.match(host, /serde\(rename_all = "camelCase", deny_unknown_fields\)/);
  assert.match(host, /"uv" \| "vcpkg" \| "conan2"/);
  assert.match(host, /IDE052_PACKAGE_OR_VERSION_UNSAFE/);
  assert.match(host, /IDE052_REMOVE_REQUIREMENT_INVALID/);
  assert.match(host, /IDE052_SYNC_TAKES_NO_PACKAGE_ARGUMENTS/);
  assert.doesNotMatch(host, /std::process::Command|Command::new|std::net::|ureq::|reqwest::/);
  assert.match(registry, /pub mod package_preview;/);
});
test('native preview is SHA-bound, rejects ambiguous managers and never grants apply',()=>{
  assert.match(host, /self\.inspect_package_managers\(project_id\)/);
  assert.match(host, /candidate\.state != "candidate_needs_review"/);
  assert.match(host, /!inspection\.blockers\.is_empty\(\)/);
  assert.match(host, /manifest\.sha256\.is_none\(\)/);
  assert.match(host, /inspection\.fingerprint\.as_bytes\(\)/);
  assert.match(host, /ready_for_approval: false/);
  assert.match(host, /approval_issued: false/);
  assert.match(host, /apply_supported: false/);
  assert.match(host, /actual_diff_computed: false/);
  assert.match(host, /requires_protected_resolver: true/);
});
test('the vcpkg direct-declaration check parses saved JSON and binds source revision',()=>{
  assert.match(host, /registry\.preview_source_file\(project_id, "vcpkg\.json"\)/);
  assert.match(host, /snapshot\.revision != expected_sha/);
  assert.match(host, /serde_json::from_str\(&snapshot\.text\)/);
  assert.match(host, /IDE052_DEPENDENCY_ALREADY_DECLARED/);
  assert.match(host, /IDE052_DEPENDENCY_NOT_DECLARED/);
  assert.match(host, /IDE052_VCPKG_DEPENDENCIES_UNSUPPORTED/);
  assert.match(host, /fn vcpkg_json_exact_declaration_is_observed_but_no_install_is_approved/);
  assert.match(host, /fn malformed_vcpkg_fails_closed_without_forged_diff/);
});
test('the native revalidation re-inspects current files and never becomes authority',()=>{
  assert.match(host, /self\.preview_package_operation\(project_id, request\)/);
  assert.match(host, /fresh\.preview_sha256 != expected_preview_sha256/);
  assert.match(host, /fresh\.evidence_fingerprint != expected_evidence_fingerprint/);
  assert.match(host, /IDE052_PREVIEW_STALE_REVIEW_AGAIN/);
  assert.match(host, /authorization_issued: false/);
  assert.match(host, /ready_for_approval: false/);
  assert.match(host, /fn source_and_lockfile_drift_fail_closed/);
  assert.match(host, /fn operation_changes_identity_and_cannot_be_reused/);
  assert.match(host, /fn linked_lockfile_cannot_be_previewed_or_revalidated/);
});
test('Tauri has review/revalidation only: no package manager execution command',()=>{
  assert.match(tauri, /async fn ide_preview_package_operation\(/);
  assert.match(tauri, /async fn ide_revalidate_package_operation\(/);
  assert.match(tauri, /registry\.preview_package_operation\(&project_id, request\)/);
  assert.match(tauri, /registry\.revalidate_package_operation\(/);
  assert.match(tauri, /ide_preview_package_operation,\s*ide_revalidate_package_operation,/);
  assert.doesNotMatch(tauri, /async fn ide_(?:apply|install|execute)_package_operation\(/);
});
test('UI invalidates stale plans on changes and verifies native read-only flags',()=>{
  assert.match(ui, /function invalidatePackagePreview\(\)/);
  assert.match(ui, /packagePreviewRequest\.current \+= 1/);
  assert.match(ui, /invoke\('ide_preview_package_operation',\{projectId,request\}\)/);
  assert.match(ui, /invoke\('ide_revalidate_package_operation',\{/);
  assert.match(ui, /result\.readyForApproval!==false/);
  assert.match(ui, /result\.approvalIssued!==false/);
  assert.match(ui, /result\.authorizationIssued!==false/);
  assert.match(ui, /setPackagePreview\(null\)/);
  assert.match(ui, /Install\/Apply unavailable/);
});
