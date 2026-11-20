import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const ui=read('../web/native_windows.jsx'),native=read('../src-tauri/src/lib.rs'),model=read('../src-tauri/src/model_benchmark.rs'),queue=read('../src-tauri/src/generation_queue.rs');
test('model benchmark is explicitly initiated in settings and separately reviewed',()=>{
  const panel=ui.slice(ui.indexOf('function LocalChecks'),ui.indexOf('export function SettingsWindow'));
  assert.match(panel,/onClick=\{\(\)=>void startModel\(\)\}/);
  assert.doesNotMatch(panel.slice(panel.indexOf('useEffect('),panel.indexOf('async function start()')),/lu_run_model_checks/);
  const cmd=native.slice(native.indexOf('async fn lu_run_model_checks'),native.indexOf('fn lu_local_check_status'));
  assert.match(cmd,/blocking_show\(\)/);assert.match(cmd,/snapshot.phase/);assert.match(cmd,/run_suite/);assert.match(cmd,/model_profile_admission.try_lock/);
  assert.doesNotMatch(read('../web/main.jsx'),/lu_run_model_checks/);
  assert.doesNotMatch(read('../../src-tauri/lu-core/src/native_capability_inventory.rs'),/lu_run_model_checks/);
});
test('fixed synthetic samples use the real supervisor with host scoring and observed metrics',()=>{
  assert.match(model,/run_reserved_with_history/);assert.match(model,/reserve_generation_with_history/);assert.match(model,/score\(&answer,&expected\)/);
  assert.match(model,/end.generated_bytes!=answer.len/);assert.match(model,/end.output_pieces!=pieces/);assert.match(model,/sample_host_engine/);
  assert.match(model,/supervisor.cancel\(sid,&gid/);assert.match(model,/CASE_LIMIT/);assert.match(model,/lease.yield_requested/);
  assert.match(queue,/!state.benchmark && state.active.is_none\(\)/);assert.match(ui,/not a general intelligence/);assert.match(ui,/Not measured/);
});
