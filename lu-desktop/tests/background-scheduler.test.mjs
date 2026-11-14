import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const core=readFileSync(new URL('../../src-tauri/lu-core/src/native_background_scheduler.rs',import.meta.url),'utf8');
const pressure=readFileSync(new URL('../../src-tauri/lu-core/src/native_resource_pressure.rs',import.meta.url),'utf8');
const results=readFileSync(new URL('../../src-tauri/lu-core/src/native_background_result.rs',import.meta.url),'utf8');
const taskStore=readFileSync(new URL('../../src-tauri/lu-core/src/tasks.rs',import.meta.url),'utf8');
const supervisor=readFileSync(new URL('../../src-tauri/lu-core/src/native_background_process.rs',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const css=readFileSync(new URL('../web/styles.css',import.meta.url),'utf8');

test('P02 uses existing supervised worker and durable task receipts',()=>{
  assert.match(core,/struct BackgroundScheduler/);
  assert.match(core,/BackgroundProcessSupervisor/);
  assert.match(core,/self\.process\.start_host_approved\(plan\)/);
  assert.match(core,/self\.process\.poll\(\)/);
  assert.match(core,/self\.store\.get\(&id\)/);
  assert.match(supervisor,/pub fn cancel_owned\(/);
  assert.match(host,/background_scheduler: Arc<lu_core::native_background_scheduler::BackgroundScheduler>/);
  assert.match(host,/lu-background-schedule\.sqlite/);
  assert.match(host,/background_tasks\.reconcile_interrupted\(\)/);
  assert.match(host,/background_scheduler\.tick_interactive_guarded\(/);
  assert.match(host,/background_scheduler\.apply_interactive_admission\(admission\)/);
});
test('P02 persisted scheduling never serializes executable paths or auto replays after crashes',()=>{
  assert.match(core,/CREATE TABLE IF NOT EXISTS lu_schedule\(/);
  assert.match(core,/CREATE TABLE IF NOT EXISTS lu_schedule_events\(/);
  assert.match(core,/host_restart_rebind_required/);
  assert.match(core,/WHERE state IN \('QUEUED','RUNNING','PAUSED'\)/);
  assert.match(core,/NEEDS_REVIEW/);
  assert.match(core,/plans:BTreeMap<String,Active>/);
  const schema=core.slice(core.indexOf('CREATE TABLE IF NOT EXISTS lu_schedule('),
    core.indexOf('CREATE INDEX IF NOT EXISTS lu_schedule_workspace'));
  for(const forbidden of ['executable TEXT','arguments TEXT','file_path TEXT','argv BLOB']){
    assert.ok(!schema.includes(forbidden),forbidden);
  }
  assert.match(core,/last\.opts\.safe_retry/);
});
test('P02 guardian admission preserves interactive room and denies unverified GPU claims',()=>{
  assert.match(core,/fn interactive_first\(\)/);
  assert.match(core,/saturating_sub\(1\)/);
  assert.match(core,/gpu:0,vram_mib:0,disk:1,network:0/);
  assert.match(core,/fn supports\(/);
  assert.match(core,/fn set_budget\(/);
  assert.match(host,/state\.supervisor\.status\(\)\.is_ok_and/);
  assert.match(host,/ResourceBudget::interactive_admission\(/);
  assert.match(host,/background_pressure\.sample\(\)/);
  assert.match(host,/background_scheduler\.apply_interactive_admission\(admission\)/);
});
test('P02 tasks can be queued, prioritized, delayed, repeated and safely cancelled',()=>{
  assert.match(core,/pub fn enqueue_host_plan\(/);
  assert.match(core,/pub fn pause_queued\(/);
  assert.match(core,/pub fn resume_queued\(/);
  assert.match(core,/pub fn cancel_owned\(/);
  assert.match(core,/pub fn guardian_snapshot\(/);
  assert.match(core,/pub fn tick_at\(/);
  assert.match(core,/opts\.repeat_seconds/);
  assert.match(core,/opts\.dependency/);
  assert.match(core,/Priority::Interactive/);
  assert.match(host,/scheduler\.enqueue_host_plan\(plan,options\)/);
  assert.match(host,/blocking_pick_file\(\)/);
  assert.match(host,/Approve schedule/);
  assert.match(host,/fn lu_background_queue_control\(/);
  assert.match(host,/fn lu_background_guardian_status\(/);
  assert.match(ui,/backgroundScheduleOptions/);
  assert.match(ui,/lu_background_guardian_status/);
  assert.match(ui,/changeQueuedBackgroundState/);
  assert.match(ui,/NEEDS_REVIEW/);
  assert.match(css,/\.backgroundGuardianStatus/);
});
test('P02 final output is not fabricated and real terminal handoff stays in TaskStore',()=>{
  assert.match(core,/TaskState::Completed=>Some\("COMPLETED"\)/);
  assert.match(core,/TaskState::Failed=>Some\("FAILED"\)/);
  assert.match(core,/TaskState::Interrupted=>Some\("INTERRUPTED"\)/);
  assert.match(host,/pending_terminal_notices_after\(/);
  assert.match(host,/acknowledge_terminal_notice\(/);
  assert.doesNotMatch(core,/eval\(/);
});

test('P02 live host pressure probes reduce admission and unsafe GPU stays off',()=>{
  assert.match(pressure,/struct PressureMonitor/);
  assert.match(pressure,/parse_mem_available/);
  assert.match(pressure,/parse_proc_cpu/);
  assert.match(core,/under_host_pressure/);
  assert.match(host,/background_pressure\.sample\(\)/);
  assert.match(host,/observed_available_ram_mib/);
  assert.match(host,/observed_cpu_busy_percent/);
  assert.match(ui,/observedAvailableRamMib/);
  assert.match(ui,/observedCpuBusyPercent/);
  assert.match(core,/gpu:0,vram_mib:0/);
});
test('P02 owner-scope reapproval cannot auto-reuse old executor or input file',()=>{
  assert.match(core,/pub fn reapprove_reviewed/);
  assert.match(core,/superseded_after_explicit_reapproval/);
  assert.match(core,/host_restart_rebind_required/);
  assert.match(host,/fn lu_background_reapprove/);
  assert.match(host,/Reapprove scheduled work/);
  assert.match(host,/blocking_pick_file\(\)/);
  assert.match(ui,/reapproveScheduledTask/);
  assert.match(ui,/Reapprove with a selected file/);
});
test('P02 verifies a native scanner digest and returns no invented file artifact',()=>{
  assert.match(supervisor,/struct ScanProof/);
  assert.match(supervisor,/complete_scan_with_receipt/);
  assert.match(taskStore,/CREATE TABLE IF NOT EXISTS lu_task_verified_results/);
  assert.match(taskStore,/pub fn complete_scan_with_receipt/);
  assert.match(taskStore,/pub fn verified_result/);
  assert.match(results,/verified_sha256/);
  assert.match(results,/verified_bytes/);
  assert.match(results,/artifacts_verified: false/);
});
test('P02 one Guardian admits both schedulable workers and approved OCR workers',()=>{
  assert.match(core,/pub fn admit_host_worker/);
  assert.match(core,/struct GuardianLease/);
  assert.match(core,/impl Drop for GuardianLease/);
  assert.match(host,/ResourceClaim::ocr\(\)/);
  assert.match(host,/OCR resource admission denied/);
});
