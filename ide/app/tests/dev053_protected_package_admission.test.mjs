import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

const tauri = read('../src-tauri/src/lib.rs');
const cargo = read('../src-tauri/Cargo.toml');
const backend = read('../src-tauri/src/build_backend.rs');
const supervisor = read('../../supervisor/src/lib.rs');
const supCargo = read('../../supervisor/Cargo.toml');
const snapshot = read('../../supervisor/src/snapshot.rs');
const evidence = read('../../supervisor/src/wo6_build_test_evidence.rs');
const runtime = read('../../supervisor/src/runtime_bundles.rs');
const linux = read('../../supervisor/src/linux_worker.rs');
const windows = read('../../supervisor/src/windows_worker.rs');
const ui = read('../web/main.jsx');

test('IDE053 package worker is strictly opted in and no default apply authority exists',()=>{
  assert.match(cargo, /native-package-uv = \["native-multilang", "lu-ide-execution-supervisor\/package-uv-worker"\]/);
  assert.match(supCargo, /package-uv-worker = \["multilang-worker", "dep:toml", "dep:base64"\]/);
  assert.match(cargo, /default = \[\]/);
  assert.match(supCargo, /default = \[\]/);
  assert.match(tauri, /IDE053_PROTECTED_PACKAGE_RUNTIME_NOT_APPROVED/);
  assert.match(tauri, /async fn ide_package_run_reviewed_uv\(/);
  const command=tauri.match(/async fn ide_package_run_reviewed_uv\([\s\S]*?\n\}/)?.[0];
  assert.ok(command);
  assert.doesNotMatch(command, /recipe_id: String|manager_id: String|command: String|executable: String|path: String|grant: String/);
  assert.match(command, /runtime\.run_reviewed_uv_sync\(/);
  assert.doesNotMatch(tauri, /async fn ide_package_run_arbitrary\(/);
});
test('native launch binds one exact Sync to preview identity in the D3 JobPlan',()=>{
  assert.match(backend, /pub fn run_reviewed_uv_sync\(/);
  assert.match(backend, /manager_id:\s*"uv"\.into\(\),\s*operation:\s*PackageOperation::Sync/);
  assert.match(backend, /registry\s*\.revalidate_package_operation\(/);
  assert.match(backend, /sha256_private_staged_file\(&input\.root, "uv\.lock"\)/);
  assert.match(backend, /input\.seal\.manifest_sha256 != py/);
  assert.match(backend, /IDE053_FROZEN_SOURCE_DIFFERS_FROM_APPROVED_REVIEW/);
  assert.match(backend, /options\.custom_flags\(0x20000\)/);
  assert.match(backend, /options\.custom_flags\(0x00200000\)/);
  assert.match(backend, /run_with_review\(\s*app,\s*registry,\s*project_id,\s*"uv_sync_offline"/);
  assert.match(backend, /recipe_id != "uv_sync_offline"/);
  assert.match(backend, /format!\(\s*"package_uv_sync_offline:\{\}"/);
  assert.match(supervisor, /strip_prefix\("package_uv_sync_offline:"\)/);
  assert.match(supervisor, /value\.runtime_limit_ms <= 3 \* 60_000/);
  assert.match(supervisor, /value\.process_limit <= 8/);
  assert.match(supervisor, /fn ide053_uv_recipe_binds_one_review_hash_and_strict_offline_limits/);
});
test('trusted bundled uv and Python identity is required before a grant or OS launch',()=>{
  assert.match(runtime, /UvSyncOffline/);
  assert.match(runtime, /"bin\/uv"/);
  assert.match(runtime, /"bin\/python3"/);
  assert.match(runtime, /"LICENSES\/uv\.txt"/);
  assert.match(backend, /LU_IDE05_WINDOWS_UV_SHA256/);
  assert.match(backend, /LU_IDE05_LINUX_UV_SHA256/);
  assert.match(snapshot, /"uv_sync_offline" if cfg!\(feature = "package-uv-worker"\)/);
});
test('Linux uv uses existing native cgroup and bwrap, immutable input, offline no-build',()=>{
  assert.match(linux, /"uv_sync_offline" if cfg!\(feature = "package-uv-worker"\)/);
  assert.match(linux, /\.arg\("--unshare-all"\)/);
  assert.match(linux, /\.arg\(&input\.root\)\s*\.arg\("\/workspace"\)/);
  assert.match(linux, /\.arg\("--bind"\)\s*\.arg\(output\)\s*\.arg\("\/output"\)/);
  for(const flag of ['--offline','--locked','--no-build','--no-install-workspace',
                     '--no-dev','--no-config','--no-python-downloads'])
    assert.ok(linux.includes('"'+flag+'"'),flag);
  assert.match(linux, /UV_PROJECT_ENVIRONMENT/);
  assert.match(linux, /UV_CACHE_DIR/);
  assert.match(linux, /UV_PYTHON_DOWNLOADS/);
  assert.match(linux, /pre_exec\(move \|\| attach_pid_in_pre_exec\(fd\)\)/);
  assert.match(linux, /require_kernel_child_membership/);
});
test('Windows deliberately denies package execution until staged readonly ACL is proven',()=>{
  assert.match(windows, /if plan\.recipe_id == "uv_sync_offline" \{\s*return Err\(SupervisorError::NoContainment\);/);
  assert.match(windows, /A copied source tree alone/);
  assert.match(windows, /bootstrap_multilang_environment/);
  assert.match(windows, /UV_PROJECT_ENVIRONMENT/);
  assert.match(windows, /"bin\/uv.exe"/);
  assert.match(windows, /network_access_limited: true/);
  assert.match(windows, /child_contained_before_resume: true/);
});
test('OS native human consent is fresh and bounded; historical review cannot grant a run',()=>{
  assert.match(backend, /fn native_package_consent\(/);
  assert.match(backend, /\.show\(move \|confirmed\|/);
  assert.match(backend, /Duration::from_secs\(45\)/);
  assert.match(backend, /requested\.load\(Ordering::SeqCst\)/);
  assert.match(backend, /registry\s*\.revalidate_package_operation\(/);
  assert.match(backend, /self\s*\.supervisor\s*\.record_human_approval\(/);
  assert.match(backend, /execute_linux_worker_with_output/);
  assert.match(backend, /execute_windows_worker_after_staging_approval_with_output/);
  assert.match(backend, /cancel_before_start\(&prepared\.job_id\)/);
  assert.match(supervisor, /self\.consume_grant_after_attestation/);
  assert.match(backend, /review_protected_package_evidence/);
  assert.match(evidence, /pub fn review_protected_package_evidence/);
  assert.match(evidence, /package_worker: bool/);
  assert.match(evidence, /job\.plan\.recipe_id == "uv_sync_offline"/);
  assert.match(evidence, /job\.plan\.effect_class == "test_build_isolated"/);
});
test('cancellation and crash recovery use existing durable original Job not a second launcher',()=>{
  assert.match(tauri, /async fn ide_build_cancel\(/);
  assert.match(tauri, /async fn ide_build_reconcile\(/);
  assert.match(backend, /pub fn cancel\(/);
  assert.match(backend, /pub fn reconcile\(/);
  assert.match(supervisor, /pub fn cancel_before_start\(/);
  assert.match(supervisor, /pub fn record_human_approval\(/);
  assert.match(backend, /requires_reconciliation/);
  assert.match(ui, /invoke\('ide_package_run_reviewed_uv',\{/);
  assert.match(ui, /onClick=\{\(\)=>void cancelProtectedBuild\(\)\}/);
  assert.match(ui, /onClick=\{\(\)=>void reconcileProtectedBuild\(job\)\}/);
  assert.match(ui, /No automatic replay or/);
  assert.match(ui, /result\.installedAppAccepted!==false/);
});
