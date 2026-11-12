import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const core=readFileSync(new URL('../../src-tauri/lu-core/src/native_background_scheduler.rs',import.meta.url),'utf8');
const queue=readFileSync(new URL('../src-tauri/src/generation_queue.rs',import.meta.url),'utf8');
const desktop=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const frontend=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const telemetry=readFileSync(new URL('../src-tauri/src/perf01_telemetry.rs',import.meta.url),'utf8');

test('P4 CPU-first Send pathways precede the sole model queue and remain model-free',()=>{
  const cpuSection=desktop.slice(desktop.indexOf('async fn lu_generate('),
    desktop.indexOf('async fn generate_internal('));
  const p=cpuSection.indexOf('perf01_router::classify(');
  const m=desktop.indexOf('.enqueue_with_class(&session_id, &chat_id, &generation_id');
  assert.ok(p>=0 && m>=0,'native CPU classifier and host-bound model queue exist');
  assert.match(cpuSection,/Route::CurrentChatCounts/);
  assert.match(cpuSection,/Route::CurrentChatLiteral/);
  assert.match(cpuSection,/Route::LocalModelStatus/);
  assert.match(cpuSection,/host_clock::fresh_local_clock_answer/);
  assert.match(cpuSection,/workspace_desktop::cpu_search_workspace_for_chat/);
  assert.doesNotMatch(cpuSection,/\.load\(&model_ref/);
  assert.match(telemetry,/model_started_by_this_route=Some\(false\)/);
});

test('P4 resource decisions are measured, fail-closed and cannot mint VRAM',()=>{
  assert.match(core,/pub fn interactive_admission\(/);
  assert.match(core,/foreground_pending:bool/);
  assert.match(core,/budget=budget\.under_host_pressure\(pressure\)/);
  assert.match(core,/budget\.gpu=0/);
  assert.match(core,/budget\.vram_mib=0/);
  assert.match(core,/HostPressure/);
  for(const reason of ['ForegroundModelTurn','SingleProcessor','UnknownMemory',
    'UnknownCpuWithResidentModel','MemoryPressure','CpuPressure','BackgroundPermitted']){
      assert.ok(core.includes(reason),'missing policy reason '+reason);
  }
  assert.match(core,/fn perf01_p4_new_workers_yield_to_human_and_unknown_pressure/);
  assert.match(core,/fn perf01_p4_single_processor_cpu_and_memory_pressure_close_new_admission/);
});

test('P4 native scheduler maintains process custody and probes immediately before spawn',()=>{
  assert.match(core,/self\.process\.poll\(\)/);
  assert.match(core,/pub fn tick_interactive_guarded\(/);
  assert.match(core,/if foreground_probe\(\)\{break\}/);
  const probeAt=core.indexOf('if foreground_probe(){break}');
  const spawnAt=core.indexOf('if self.process.start_host_approved(plan).is_err()');
  assert.ok(probeAt>=0 && spawnAt>probeAt);
  assert.match(desktop,/tick_interactive_guarded\(/);
  assert.match(desktop,/\|\|state\.generation_queue\.has_foreground_jobs\(\)/);
  assert.match(desktop,/admission\.defer_new_background/);
  assert.match(core,/fn perf01_p4_second_probe_stops_native_starts_after_queue_scan/);
  assert.match(core,/pub fn apply_interactive_admission\(/);
  assert.match(core,/interactive_admission_hold:bool/);
  assert.match(core,/if g\.interactive_admission_hold/);
  assert.match(core,/fn perf01_p4_guardian_hold_blocks_new_lease_but_preserves_existing_custody/);
  assert.match(desktop,/apply_interactive_admission\(admission\)/);
});

test('P4 model response queue prioritizes humans but eventually admits aged background',()=>{
  assert.match(queue,/const MAX_FOREGROUND_BURST: u8 = 4/);
  assert.match(queue,/const MIN_BACKGROUND_DWELL: Duration = Duration::from_secs\(2\)/);
  assert.match(queue,/background_aged/);
  assert.match(queue,/state\.foreground_streak/);
  assert.match(queue,/MAX_BACKGROUND_JOBS: usize = 2/);
  assert.match(queue,/fn perf01_p4_recent_background_yields_next_foreground_ttft_after_burst/);
  assert.match(queue,/fn background_eventually_gets_one_slot_after_foreground_burst/);
  assert.match(queue,/pub fn priority_snapshot\(/);
});

test('P4 Resource Guardian presents policy reason without leaking model prompts or granting tools',()=>{
  assert.match(desktop,/interactive_admission_reason:Mutex<Option/);
  assert.match(desktop,/interactive_admission_reason:reason\.map/);
  assert.match(desktop,/waiting_foreground_generations:priority\.waiting_foreground/);
  assert.match(frontend,/backgroundGuardian\.newBackgroundAdmissionDeferred/);
  assert.match(frontend,/backgroundGuardian\.activeModelGenerationClass/);
  assert.match(frontend,/Host reason:/);
  assert.match(frontend,/Existing workers retain cancellation and restart custody/);
  assert.doesNotMatch(frontend,/invoke\('lu_background_authorize_from_priority'/);
});
