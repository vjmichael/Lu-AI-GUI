import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const inspector=read('../../supervisor/src/debug_control_evidence.rs');
const supervisor=read('../../supervisor/src/lib.rs');
const native=read('../src-tauri/src/debug_backend.rs');
const handlers=read('../src-tauri/src/lib.rs');
const ui=read('../web/main.jsx');

test('FIN04 original control evidence is independently validated without importing control dispatch',()=>{
 assert.match(supervisor,/pub mod debug_control_evidence;/);
 assert.match(inspector,/pub fn inspect_debug_control_evidence/);
 assert.match(inspector,/fn read_verified\(/);
 assert.match(inspector,/FIN04_ACTION_JOURNAL_HASH_OR_IDENTITY_INVALID/);
 assert.match(inspector,/FIN04_ACTION_JOURNAL_OWNER_CHANGED/);
 assert.match(inspector,/FIN04_ACTION_JOURNAL_INVALID_TRANSITION/);
 assert.match(inspector,/FIN04_DEBUG_JOURNAL_CHANGED_DURING_ACTION_INSPECTION/);
 assert.match(inspector,/original_debug_journal_digest/);
 assert.doesNotMatch(inspector.split('#[cfg(test)]')[0],
   /debug_control_dispatch|CiControlSlot|\.take_step\(|\.take_breakpoint\(|\.take_variables\(|NativeConsentMarker/);
});
test('FIN04 full foreign action chain is checked before project-specific redaction',()=>{
 assert.match(inspector,/let parsed=read_verified\(&entry\.path\(\),action\)\?/);
 assert.match(inspector,/FIN04_ACTION_ORIGINAL_PLAN_MISMATCH/);
 assert.match(inspector,/if original\.project_id!=project_id \{continue;\}/);
 assert.match(inspector,/project_action_count=all\.len\(\)/);
 assert.match(inspector,/journal_chain_verified:true/);
 assert.match(inspector,/native_owner_event_live_verified:false,global_kernel_custody_accepted:false/);
 assert.match(inspector,/debugger_execution_authorized:false,replay_authorized:false/);
});
test('FIN04 read-only control candidate is not a DAP control or an OS grant',()=>{
 assert.match(native,/pub fn review_control_candidate/);
 assert.match(native,/self\.review_stopped_source\(/);
 assert.match(native,/self\.inspect_control_evidence\(project_id\)\?/);
 assert.match(native,/no_live_native_owner_event/);
 assert.match(native,/debugger_job_already_terminal/);
 assert.match(native,/fin01d_native_kernel_acceptance_not_proven/);
 assert.match(native,/one_use_execution_consent_not_granted/);
 assert.match(native,/execution_route_available:false/);
 assert.match(native,/debugger_execution_authorized:false/);
 assert.match(native,/replay_authorized:false/);
});
test('FIN04 native desktop exposes only review and audit commands from these changes',()=>{
 assert.match(handlers,/async fn ide_debug_control_evidence/);
 assert.match(handlers,/async fn ide_debug_review_control_candidate/);
 assert.match(handlers,/FIN04_UNKNOWN_READ_ONLY_CONTROL_KIND/);
 assert.match(handlers,/ide_debug_control_evidence,/);
 assert.match(handlers,/ide_debug_review_control_candidate,/);
 assert.match(ui,/FIN-04 original debugger action journal read-only evidence/);
 assert.match(ui,/invoke\('ide_debug_control_evidence'/);
 assert.match(ui,/invoke\('ide_debug_review_control_candidate'/);
 assert.match(ui,/Review control candidate \(no execution\)/);
 assert.match(ui,/result\.debuggerExecutionAuthorized!==false/);
 assert.match(ui,/result\.liveNativeOwnerEventVerified!==false/);
});
