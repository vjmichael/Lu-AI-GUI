import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const source=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const shared=source('../src-tauri/src/global_native_custody.rs');
const build=source('../src-tauri/src/build_backend.rs');
const debug=source('../src-tauri/src/debug_backend.rs');
const backend=source('../src-tauri/src/lib.rs');

test('FIN01 build and debugger share the exact profile-wide cross-process admission fence',()=>{
 assert.match(shared,/options\.create_new\(true\)\.write\(true\)/);
 assert.match(shared,/fin01-global-native-custody-v1\.lock/);
 assert.match(shared,/fn sibling_rows/);
 assert.match(shared,/debug-fixture-receipts\.jsonl/);
 assert.match(shared,/build-job-receipts\.jsonl/);
 assert.match(build,/GlobalAdmissionFence\s*::acquire\(&self\.global_custody_profile,"build"\)/);
 assert.match(debug,/GlobalAdmissionFence\s*::acquire\(&self\.global_custody_profile,"debug"\)/);
 assert.match(backend,/mod global_native_custody;/);
});
test('FIN01 all terminal receipts required before global fence release',()=>{
 assert.match(shared,/pub fn release_if_terminal/);
 assert.match(shared,/durable_busy\(own\) \|\| durable_busy\(other\)/);
 assert.match(shared,/FIN01_GLOBAL_CUSTODY_NOT_RECONCILED/);
 assert.match(build,/global_fence\.release_if_terminal\(&own,&sibling\)/);
 assert.match(debug,/global_fence\.release_if_terminal\(&own,&sibling\)/);
 assert.doesNotMatch(shared,/impl Drop for GlobalAdmissionFence/);
});
test('FIN01 installed UI readiness observes interrupted fence and other supervisor',()=>{
 for(const s of [build,debug]){
  assert.match(s,/global_custody_unresolved\(&self\.global_custody_profile\)/);
  assert.match(s,/sibling_rows\(/);
  assert.match(s,/other_custody\.recovery_required/);
 }
 assert.match(build,/global_worker_busy = global_worker_busy \|\|/);
 assert.match(debug,/!other_custody\.worker_busy_or_unreconciled/);
});

const lsp=source('../src-tauri/src/lsp_backend.rs');
const main=source('../web/main.jsx');

test('FIN01D all three original supervisors participate in admission and recovery',()=>{
 assert.match(shared,/lsp-job-receipts\.jsonl/);
 assert.match(shared,/debug-fixture-receipts\.jsonl/);
 assert.match(shared,/build-job-receipts\.jsonl/);
 assert.match(shared,/for \(id,_\) in DOMAINS/);
 assert.match(shared,/pub fn preflight_global_idle/);
 assert.match(lsp,/GlobalAdmissionFence\s*::acquire\(&self\.global_custody_profile,"lsp"\)/);
 assert.match(lsp,/global_admission: Mutex<Option<crate::global_native_custody::GlobalAdmissionFence>>/);
 assert.match(lsp,/fn release_global_if_terminal/);
 assert.match(lsp,/pub fn reconcile\(/);
 assert.match(lsp,/reconcile_linux_interrupted/);
 assert.match(lsp,/reconcile_windows_interrupted_via_service/);
 assert.match(backend,/ide_lsp_recoverable_jobs,/);
 assert.match(backend,/ide_lsp_reconcile,/);
 assert.match(backend,/feature = "native-lsp"\)\)\]/);
 assert.match(main,/LSP pending: \{fin01Recovery\.lspJournalPending\}/);
});
test('FIN01D no-worker denied admission releases only after unchanged original journal',()=>{
 assert.match(shared,/release_before_prepare_if_unchanged/);
 assert.match(shared,/FIN01_NEW_JOURNAL_EVENT_BEFORE_ADMISSION_RELEASE/);
 for(const impl of [build,debug]){
  assert.match(impl,/preflight_global_idle\(/);
  assert.match(impl,/release_before_prepare_if_unchanged\(&own_before,&after\)/);
 }
 assert.match(lsp,/release_before_prepare_if_unchanged\(&own,&after\)/);
 assert.doesNotMatch(shared,/impl Drop for GlobalAdmissionFence/);
});
test('FIN01D no operator UI control can release a global marker from a journal count',()=>{
 assert.match(shared,/marker_removal_authorized:false/);
 assert.match(shared,/auto_retry_authorized:false/);
 assert.match(main,/Request native OS-only cleanup/);
 assert.match(lsp,/FIN01_LSP_RECOVERY_NOT_APPROVED/);
 assert.match(lsp,/FIN01_LSP_OS_RECOVERY_UNVERIFIED/);
 assert.doesNotMatch(main,/invoke\(['"]ide_fin01_release_marker/);
});

test('FIN01D live LSP commands demand matching owner marker',()=>{
 assert.match(shared,/pub fn marker_owner_intact/);
 assert.match(lsp,/fn require_current_global_owner/);
 assert.match(lsp,/self\.require_current_global_owner\(\)\?/);
 assert.match(lsp,/self\.require_current_global_owner\(\)\.is_err\(\)/);
 assert.match(shared,/FIN01_OWNER_JOURNAL_BYTES_CHANGED_BEFORE_ADMISSION_RELEASE/);
 assert.match(shared,/fn journal_fingerprint/);
 assert.match(shared,/options\.mode\(0o600\)/);
});

test('FIN01D only original strictly pre-OS jobs receive no-worker cancellation',()=>{
 for(const code of [build,debug]){
  assert.match(code,/if result\.is_err\(\)/);
  assert.match(code,/matches!\(state\.phase,JobPhase::Prepared\|JobPhase::Authorized\)/);
  assert.match(code,/self\.supervisor\.cancel_before_start\(&id\)/);
 }
 assert.match(lsp,/let mut admitted_this_call=false/);
 assert.match(lsp,/let mut prepared_this_call:Option<String>=None/);
 assert.match(lsp,/JobPhase::Prepared\|JobPhase::Authorized/);
 assert.match(lsp,/self\.supervisor\.cancel_before_start\(id\)/);
 assert.match(lsp,/no_active && admitted_this_call/);
 assert.match(shared,/release_if_terminal/);
});

const windows=source('../../supervisor/src/windows_worker.rs');
const linux=source('../../supervisor/src/linux_worker.rs');
test('FIN01D stale clearance must query real kernel objects, not renderer booleans',()=>{
 assert.match(shared,/pub fn clear_stale_marker_with_kernel_proof/);
 assert.match(shared,/verify_global_worker_quiescence\(owner_pid,&jobs\)/);
 assert.match(shared,/original_journals_sha256:baseline/);
 assert.match(shared,/FIN01_JOURNAL_CHANGED_AFTER_KERNEL_WITNESS/);
 assert.match(shared,/FIN01_STALE_MARKER_CHANGED/);
 assert.match(shared,/FIN01_RECOVERY_AUDIT_SYNC_FAILED/);
 assert.match(windows,/pub fn verify_global_worker_quiescence/);
 assert.match(windows,/OpenProcess\(PROCESS_QUERY_LIMITED_INFORMATION/);
 assert.match(windows,/OpenJobObjectW\(JOB_OBJECT_QUERY_ACCESS/);
 assert.match(windows,/job\.empty\(\)/);
 assert.match(windows,/JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE/);
 assert.match(linux,/pub fn verify_global_worker_quiescence/);
 assert.match(linux,/current_cgroup_parent\(\)/);
 assert.match(linux,/parse_empty_cgroup_events/);
 assert.match(linux,/cgroup\.procs/);
 assert.match(linux,/pids\.current/);
});
test('FIN01D host requires physical dialog and never promotes replay authority',()=>{
 assert.match(backend,/async fn ide_fin01_clear_stale_marker/);
 assert.match(backend,/FIN01_GLOBAL_RECOVERY_NOT_APPROVED/);
 assert.match(backend,/Verify kernel and clear only if empty/);
 assert.match(backend,/ide_fin01_clear_stale_marker,/);
 assert.match(shared,/replay_authorized:false,execution_authorized:false/);
 assert.match(main,/Request OS-verified stale-marker clearance/);
 assert.match(main,/FIN01_NATIVE_KERNEL_RECEIPT_UNTRUSTED/);
});
