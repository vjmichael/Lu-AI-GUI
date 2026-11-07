import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=(path)=>readFileSync(new URL(path,import.meta.url),'utf8');
const tasks=read('../../src-tauri/lu-core/src/tasks.rs');
const lane=read('../../src-tauri/lu-core/src/native_awareness_lane.rs');
const host=read('../src-tauri/src/observe_desktop.rs');
const desktop=read('../src-tauri/src/lib.rs');
const ui=read('../web/main.jsx');
const native=read('../../src-tauri/lu-core/src/native_observe_learn.rs');

test('P06 observation is off-by-default and never grants ambient desktop capture',()=>{
  assert.match(native,/mode:Mode::Off/);
  assert.match(host,/can_see_desktop:false/);
  assert.match(host,/screen_capture_enabled:false/);
  assert.match(host,/allow_redacted_snapshots:false/);
  assert.match(host,/inference_performed:0/);
  assert.match(desktop,/lu_observe_start/);
  assert.match(desktop,/blocking_show\(\)/);
});
test('P06 old native_runtime CHECK schema is atomically migrated without dropping rows',()=>{
  assert.match(tasks,/source TEXT NOT NULL CHECK/);
  assert.match(tasks,/native_accessibility','native_runtime','native_test_receipt/);
  assert.match(tasks,/if !awareness_schema\.contains\("'native_runtime'"\)/);
  assert.match(tasks,/INSERT INTO lu_awareness_notes_p06/);
  assert.match(tasks,/FROM lu_awareness_notes;/);
  assert.match(tasks,/p06_native_runtime_notes_migrate_and_private_deletion_is_chat_scoped/);
  assert.match(host,/p06_trusted_runtime_error_persists_but_cannot_claim_verified_gui_failure/);
});
test('P06 seven-day retention and chat-only deletion cannot mint tool or learning grant',()=>{
  assert.match(tasks,/datetime\('now','-7 days'\)/);
  assert.match(tasks,/delete_awareness_notes_for_workspace/);
  assert.match(tasks,/WHERE workspace_id=\?1 AND task_id LIKE 'obs-%'/);
  assert.match(tasks,/p06_note_retention_prunes_stale_only/);
  assert.match(host,/pub\(crate\) fn forget_notes/);
  assert.match(host,/awareness\.discard_observations\(\)/);
  assert.match(lane,/pub fn discard_observations/);
  assert.match(lane,/p06_forget_clears_handoff_without_regrant_or_sequence_replay/);
  assert.match(desktop,/async fn lu_observe_forget_notes/);
  assert.match(desktop,/state\.chats\.history\(&chat_id\)/);
  assert.match(desktop,/Forget diagnostic notes/);
  assert.match(ui,/invoke\('lu_observe_forget_notes'/);
  assert.match(ui,/Forget observation notes for this chat/);
  assert.match(ui,/diagnostic codes expire after seven days/i);
  assert.doesNotMatch(host,/grant_learning_review\(true\)/);
});
test('P06 capture receipts never imply verified visual interpretation or retained pixels',()=>{
  assert.match(host,/source_identity_attested:source_attested/);
  assert.match(host,/capture_guardian:Option<GuardianLease>/);
  assert.match(host,/session\.capture_guardian\.take\(\)/);
  assert.match(host,/release_guardian_when_reaped/);
  assert.match(host,/fn reconcile_revoked\(&self,session:&mut Session\)/);
  assert.match(host,/p06_revoked_consent_task_receipt_reconciles_after_native_cleanup/);
  assert.match(host,/p06_rejected_capture_plan_drops_guardian_reservation_without_spawning_worker/);  assert.match(host,/release_guardian_when_reaped/);
  assert.match(host,/capture_processes\.poll\(\)\s*\.map_err\(/);
  assert.doesNotMatch(host,/let _=self\.capture_processes\.poll\(\)/);
  assert.match(host,/let receipt_ready=verified_reaped_frame_receipt\(/);
  assert.match(host,/let worker_reaped=!self\.capture_processes\.can_cancel\(&task_id\)/);
  assert.match(host,/frame_sha256:if receipt_ready\{sha\}else\{None\}/);
  assert.match(host,/p06_capture_receipt_requires_supervised_os_reap_before_evidence/);
  assert.match(host,/fully_redacted_receipt:receipt_ready/);
  assert.match(host,/pixels_retained:false/);
  assert.match(host,/visual_interpretation_verified:false/);
  assert.match(desktop,/lu_observe_capture_one_shot/);
  assert.match(ui,/No observation data/);
});

test('P06 native WGC process instance is verified, generic picker stays unverified',()=>{
  const worker=read('../../src-tauri/lu-observe-capture-worker/src/main.rs');
  const scheduler=read('../../src-tauri/lu-core/src/native_background_scheduler.rs');
  assert.match(worker,/OpenProcess\(PROCESS_QUERY_LIMITED_INFORMATION,false,pid\)/);
  assert.match(worker,/GetProcessTimes/);
  assert.match(worker,/struct ProcessQueryHandle\(HANDLE\)/);
  assert.match(worker,/CloseHandle\(self\.0\)/);
  assert.match(worker,/process_instance::born_at\(pid\)\?!=born/);
  assert.equal((worker.match(/process_instance::born_at\(pid\)\?!=born/g)||[]).length,2);
  assert.match(worker,/Foreground window instance changed during readback/);
  assert.match(worker,/foreground_identity:Option<\(usize,u32,u64\)>/);
  assert.match(worker,/\(nonce\.into_owned\(\),None\)/);
  assert.match(worker,/padded\.fill\(255\)/);
  assert.match(worker,/Lu-WGC-whole-frame-redacted-v2/);
  assert.match(scheduler,/pub fn capture_one_shot\(\)/);
  const supervisor=read('../../src-tauri/lu-core/src/native_background_process.rs');
  assert.match(supervisor,/--lu-observe-one-shot"\|"--lu-observe-foreground-one-shot/);
  assert.match(supervisor,/job\.set_memory_limit\(512\*1024\*1024\)/);  assert.match(scheduler,/pub fn capture_one_shot\(\)/);
  assert.match(desktop,/guardian\.admit_host_worker\(/);
  assert.match(desktop,/observer\.launch_one_shot\(&chat_id,plan,lease\)/);
  assert.match(ui,/fullyRedactedReceipt/);
  assert.match(ui,/sourceIdentityAttested/);
});

test('P06 denied Windows WGC child is never misreported as workflow failure',()=>{
  const focused=read('../../.github/workflows/lu-p06-observe-privacy.yml');
  const legacy=read('../../.github/workflows/lu-v1-observe-learn.yml');
  assert.match(focused,/if\(\$LASTEXITCODE -ne 1\)\{throw 'Native WGC worker failed closedness check'\}[\s\S]*?exit 0/);
  assert.match(legacy,/throw 'Capture worker did not reject unauthorized invocation without pixel output'[\s\S]*?exit 0/);
});

test('P06 physical Windows child-process evidence is opt-in and cannot claim WGC UI or redaction',()=>{
  const script=read('../../scripts/verify-observe-installed-windows.ps1');
  assert.match(script,/\[switch\]\$ExerciseCaptureCancellation/);
  assert.match(script,/Get-CimInstance -ClassName Win32_Process/);
  assert.match(script,/ParentProcessId/);
  assert.match(script,/ExecutablePath/);
  assert.match(script,/captureWorkerOsReap='PASS_OS_CHILD_REAP_ONLY'/);
  assert.match(script,/wgcPickerAndCancellation='NOT_TESTED'/);
  assert.match(script,/sensitiveRegionRedaction='NOT_TESTED'/);
  assert.match(script,/visionOrGuiTestAcceptance='NOT_TESTED'/);
  assert.doesNotMatch(script,/Press-ObservedButton 'Approve'/);
});

test('P06 bounded region mask is not a shortcut around native OS source and privacy attestation',()=>{
  const worker=read('../../src-tauri/lu-observe-capture-worker/src/main.rs');
  assert.match(worker,/fn mask_sensitive_bgra_regions/);
  assert.match(worker,/regions\.is_empty\(\)\|\|regions\.len\(\)>MAX_MASKS/);
  assert.match(worker,/rect\.left\.checked_add\(rect\.width\)/);
  assert.match(worker,/invalid_or_untrusted_mask_never_partially_writes_raw_frame/);
  assert.match(worker,/future_sensitive_region_mask_is_stride_correct_without_touching_padding/);
  // Only fully blanked, nonce-bound receipts remain connected to WGC frame events.
  assert.match(worker,/fn frame_receipt\(/);
  assert.match(worker,/Lu-WGC-whole-frame-redacted-v2/);
  assert.doesNotMatch(worker,/mask_sensitive_bgra_regions\(width,height,row_pitch,buffer/);
});
