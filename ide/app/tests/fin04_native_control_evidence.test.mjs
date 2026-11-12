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

test('FIN04 full original-action inventory blocks production admission across every project',()=>{
 assert.match(inspector,/pub global_os_cleanup_required:bool/);
 assert.match(inspector,/global_action_cleanup \|= parsed\.requires_os_cleanup/);
 assert.match(inspector,/let global_os_cleanup_required=global_action_cleanup/);
 assert.match(inspector,/if original\.project_id!=project_id \{continue;\}/);
 assert.match(inspector,/action_directory_fingerprint/);
 assert.match(inspector,/FIN04_ACTION_DIRECTORY_CHANGED_DURING_INSPECTION/);
 assert.match(inspector,/FIN04_ORIGINAL_DEBUG_SUPERVISOR_STATE_STALE/);
 assert.match(native,/let original_action_cleanup_required=/);
 assert.match(native,/action_evidence\.global_os_cleanup_required/);
 assert.match(native,/orphaned_debug_action_required: original_action_cleanup_required/);
 assert.match(native,/FIN04_ORIGINAL_ACTION_OS_RECOVERY_REQUIRED/);
 assert.match(native,/FIN04_ACTION_RECOVERY_REQUIRED_INSIDE_GLOBAL_FENCE/);
 assert.match(native,/release_before_prepare_if_unchanged\(&own_before,&after\)/);
 assert.doesNotMatch(native,/let dangling_action_jobs: Vec<String> = Vec::new\(\)/);
});
test('FIN04 source candidate SHA binds original debugger and action journal identities',()=>{
 assert.match(inspector,/pub original_debug_journal_sha256:String/);
 assert.match(inspector,/pub action_manifest_sha256:String/);
 assert.match(native,/audit\.original_debug_journal_sha256\.as_str\(\)/);
 assert.match(native,/audit\.action_manifest_sha256\.as_str\(\)/);
 assert.match(native,/FIN04_READ_ONLY_CONTROL_SOURCE_CHANGED/);
 assert.match(native,/original_debug_journal_sha256:audit\.original_debug_journal_sha256/);
 assert.match(native,/action_manifest_sha256:audit\.action_manifest_sha256/);
 assert.match(native,/global_action_cleanup_required:audit\.global_os_cleanup_required/);
 assert.match(native,/execution_route_available:false/);
});
test('FIN04 web inspector rejects fabricated global flags and missing native journal digests',()=>{
 assert.match(ui,/typeof evidence\.globalOsCleanupRequired!=='boolean'/);
 assert.match(ui,/test\(evidence\.originalDebugJournalSha256\|\|''\)/);
 assert.match(ui,/test\(evidence\.actionManifestSha256\|\|''\)/);
 assert.match(ui,/fin04Evidence\.globalOsCleanupRequired/);
 assert.match(ui,/typeof result\.globalActionCleanupRequired!=='boolean'/);
 assert.match(ui,/result\.executionRouteAvailable!==false/);
 assert.match(ui,/result\.debuggerExecutionAuthorized!==false/);
});
