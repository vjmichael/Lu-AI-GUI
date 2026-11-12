// FIN-04R structural rejection tests. These check authored boundaries, NOT
// a Windows Job Object, LLVM patch, native consent or installed acceptance.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const app = new URL('../', import.meta.url);
const root = new URL('../../', app);
const read = path => readFileSync(new URL(path, root), 'utf8');
const supervisorCargo = read('ide/supervisor/Cargo.toml');
const desktopCargo = read('ide/app/src-tauri/Cargo.toml');
const build = read('ide/app/src-tauri/build.rs');
const native = read('ide/supervisor/src/windows_lsp_worker.rs');
const owner = read('ide/supervisor/src/windows_event_owner.rs');
const control = read('ide/supervisor/src/debug_control_dispatch.rs');
const backend = read('ide/app/src-tauri/src/debug_backend.rs');
const tauri = read('ide/app/src-tauri/src/lib.rs');
const ui = read('ide/app/web/main.jsx');
const authority = read('ide/app/web/project_debug_authority.js');
const script = read('ide/llvm18_owner/Build-PinnedOwnerWindows.ps1');

test('release observer is an explicit protected standalone feature, never an action feature',()=>{
 assert.match(supervisorCargo,/fin04-release-owner-evidence = \["dap-transport", "protected-recovery-service"\]/);
 assert.match(desktopCargo,/fin04-release-event-observer = \["fin02-protected-alpha", "lu-ide-execution-supervisor\/fin04-release-owner-evidence"\]/);
 assert.match(backend,/FIN04R_RELEASE_OWNER_OBSERVER_ONLY_SOURCE_AND_ACTION_GATES_OPEN/);
 assert.match(backend,/operation_control_supported: cfg!\(all\(/);
 assert.match(backend,/feature = "dev046d-native-dialog"/);
});
test('built owner sideband is inherited only in the restricted project DAP Job',()=>{
 assert.match(native,/plan\.recipe_id == "dap_lldb_project"/);
 assert.match(native,/feature = "fin04-release-owner-evidence"/);
 assert.match(native,/let event_sideband = if let Some\(\(reader, nonce\)\) = owner_pipe/);
 assert.match(native,/verify_lldb_kernel_event_owner/);
 assert.match(owner,/start_owner_pipe_reader/);
 assert.match(owner,/fn wait_exact_native_continue\(/);
});
test('release owner source cannot be promoted from arbitrary DAP or same-size mutable resource',()=>{
 assert.match(native,/pub fn observe_release_owned_stop\(/);
 assert.match(native,/verify_windows_native_debug_port/);
 assert.match(native,/sample_windows_stopped_thread_rip/);
 assert.match(native,/persist_native_owner_receipt/);
 assert.match(native,/verify_input\(&self\.source_input\)/);
 assert.match(native,/source_line_independently_verified:false/);
 assert.match(native,/debugger_control_authorized:false,recovery_authorized:false/);
 assert.match(build,/fin04_require_reviewed_owner_stage/);
 assert.match(build,/FIN04R_PATCHED_EVENT_OWNER_ANCHOR_MISSING/);
 assert.match(script,/fin04-owner-provenance\.json/);
 assert.match(script,/productionDebuggerActions=\$false/);
});
test('independent result verifier is compiled but release cannot create dispatch authority',()=>{
 assert.match(control,/pub\(crate\) fn verify_independent_step_result/);
 assert.match(control,/if cfg!\(all\(windows, feature = "fin04-release-owner-evidence"\)\)/);
 assert.match(control,/&& !cfg!\(test\)/);
 assert.match(control,/return Err\(ControlDenial::Unreviewed\)/);
 assert.match(backend,/release_owner_control_authorized: false/);
 assert.match(authority,/releaseOwnerControlAuthorized/);
});
test('OS-only interrupted-action recovery cannot silently clear original marker or replay an operation',()=>{
 assert.match(tauri,/async fn ide_debug_action_os_recovery\(/);
 assert.match(tauri,/FIN04REC_OPERATOR_DECLINED/);
 assert.match(backend,/pub fn record_action_os_recovery\(/);
 assert.match(backend,/verify_global_worker_quiescence\(prior_owner,&jobs\)/);
 assert.match(backend,/options\.create_new\(true\)\.write\(true\)/);
 assert.match(backend,/actionOutcome":"unknown_never_replay"/);
 assert.match(backend,/global_marker_cleared:false/);
 assert.match(backend,/post_command_result_independently_verified:false/);
 assert.match(ui,/invoke\('ide_debug_action_os_recovery'/);
 assert.match(ui,/result\.originalActionReplayed!==false/);
});
