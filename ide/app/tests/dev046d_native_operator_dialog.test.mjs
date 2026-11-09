import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

test('debugger actions are fixed proposals; no WebView-supplied DAP control, PID or tokens', () => {
  const commands=read('../src-tauri/src/lib.rs');
  assert.match(commands, /async fn ide_debug_step_project\(/);
  assert.match(commands, /async fn ide_debug_breakpoint_project\(/);
  assert.match(commands, /ReviewedNativeAction::StepOver/);
  assert.match(commands, /ReviewedNativeAction::SameLineBreakpoint/);
  assert.match(commands, /DEV046D_NATIVE_ACTIONS_NOT_INSTALLED/);
  assert.match(commands, /compile_error!\("DEV-04\.6D native dialog must not ship in release/);
  const web=read('../web/main.jsx');
  assert.match(web, /invoke\(command,\{projectId\}\)/);
  assert.match(web, /debugStatus\?\.operationControlSupported/);
  assert.doesNotMatch(web, /invoke\(command,\{projectId,\s*(?:pid|threadId|path|token|dap)/);
});
test('native callback requires verified LLDB owner and one fresh per-action OS dialog', () => {
  const worker=read('../../supervisor/src/windows_lsp_worker.rs');
  assert.match(worker, /execute_one_protected_native_step/);
  assert.match(worker, /attempt_one_protected_native_breakpoint/);
  assert.match(worker, /One-use challenge:/);
  assert.match(worker, /verify_lldb_kernel_event_owner\(/);
  assert.match(worker, /NativeConsentMarker::for_trusted_modal/);
  assert.match(worker, /action\.consume_for_receipt_test\(/);
  assert.match(worker, /live_after_dialog\.phase != JobPhase::Running/);
  assert.match(worker, /cancelled\.is_some_and/);
  const backend=read('../src-tauri/src/debug_backend.rs');
  assert.match(backend, /session\s*\.verify_real_lldb_event_owner_for_ci\(/);
  assert.match(backend, /app\.dialog\(\)/);
  assert.match(backend, /\.show\(move \|confirmed\|/);
  assert.match(backend, /receiver\.recv_timeout\(/);
  assert.match(backend, /Duration::from_secs\(30\)/);
  assert.match(backend, /confirmed && !cancel\.load\(Ordering::SeqCst\)/);
  assert.match(backend, /bounded_native_debugger_consent/);
  assert.match(backend, /session\s*\.close_with_real_owner_continue_for_ci/);
});
test('CI simulated consent and real modal consent remain distinct in audit', () => {
  const receipts=read('../../supervisor/src/debug_action_receipts.rs');
  assert.match(receipts, /ci_fixture_only: false/);
  assert.match(receipts, /"ci_only_not_installed_consent"/);
  assert.match(receipts, /"native_human_confirmed"/);
  assert.match(receipts, /"native_dialog_independent_os_step"/);
  assert.match(receipts, /self\.consent_ci_only = marker\.ci_fixture_only/);
  const dispatch=read('../../supervisor/src/debug_control_dispatch.rs');
  assert.match(dispatch, /!claim\.ci_fixture_only && !trusted_native_dialog/);
  assert.match(dispatch, /claim\.execution_authorized/);
});
test('breakpoint acknowledgement is not independently verified installation', () => {
  const worker=read('../../supervisor/src/windows_lsp_worker.rs');
  assert.match(worker, /action\s*\.record_uncertain_result\(/);
  const backend=read('../src-tauri/src/debug_backend.rs');
  assert.match(backend, /audit\.3 != "ResultUncertain"/);
  assert.match(backend, /native_dialog_bp_uncertain/);
  assert.match(backend, /installed_app_accepted: false/);
});
