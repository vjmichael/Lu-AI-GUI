import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const ui=read('../web/native_windows.jsx');
const native=read('../src-tauri/src/lib.rs');
const controller=read('../src-tauri/src/active_checks.rs');
const archive=read('../src-tauri/src/diagnostic_archive.rs');
test('local checks are an explicit Settings action, not a Chat or mount action',()=>{
  assert.match(ui, /section==='privacy'[\s\S]*?<LocalChecks active=\{active\}\/>/);
  const panel=ui.slice(ui.indexOf('function LocalChecks'),ui.indexOf('export function SettingsWindow'));
  const effect=panel.slice(panel.indexOf('useEffect('),panel.indexOf('async function start()'));
  assert.match(effect,/lu_local_check_status/);assert.doesNotMatch(effect,/lu_run_local_checks/);
  assert.match(panel,/onClick=\{\(\)=>void start\(\)\}/);
  assert.doesNotMatch(read('../web/main.jsx'),/lu_run_local_checks|lu_stop_local_checks/);
});
test('real native one-use approval and stop gate have no model or user payload',()=>{
  const cmd=native.slice(native.indexOf('async fn lu_run_local_checks'),native.indexOf('/// Restore exact saved'));
  assert.match(cmd,/caller.label\(\)/);assert.match(cmd,/blocking_show\(\)/);assert.match(cmd,/owner.active_checks.run/);
  assert.match(cmd,/has_foreground_jobs\(\)/);assert.match(cmd,/available_ram\(\)/);
  assert.match(controller,/if !consent\(\)\|\|reservation.cancel.load/);
  assert.match(native,/state.active_checks.shutdown\(\)/);
  assert.doesNotMatch(read('../../src-tauri/lu-core/src/native_capability_inventory.rs'),/lu_run_local_checks/);
});
test('results distinguish interruptions and bounded sample performance; no automatic repair',()=>{
  assert.match(ui,/Unfinished checks were not verified/);assert.match(ui,/not a model benchmark/);
  assert.match(ui,/Nothing|No personal files/);assert.match(archive,/UPDATE active_check SET state='interrupted' WHERE state='running'/);
  assert.match(controller,/store.search_current_messages/);assert.match(controller,/rows.len\(\)!=1/);
  assert.match(controller,/active_check_report\(\)/);assert.doesNotMatch(ui.slice(ui.indexOf('function LocalChecks'),ui.indexOf('export function SettingsWindow')),/run_id|SHA-256|repair now|tool registry/i);
});
test('expanded suite preserves legacy verdict and only uses newly reserved scratch slots',()=>{
  assert.match(controller,/LEGACY_CHECKS/);
  assert.match(controller,/SCRATCH_SLOTS: \[&str; 4\]/);
  assert.match(controller,/AlreadyExists=>continue/);
  assert.match(controller,/const SCALE_ROWS: u32 = 1024/);
  assert.match(controller,/const SCALE_QUERIES: u32 = 64/);
  assert.match(controller,/document_windows_check\(stop\)/);
  assert.match(archive,/old.body.expanded!=body.expanded/);
  assert.match(ui,/This report used the earlier three-check suite/);
  assert.match(ui,/Prepared and reopened 1,024 sample messages/);
  assert.match(ui,/if\(!await invoke\('lu_stop_local_checks'\)\)/);
});
test('installed acceptance requires native accessibility actions and independent read-only journal',()=>{
  const gate=read('../../scripts/cp11a-native-checks-gui.py');
  assert.match(gate,/mode=ro/); assert.match(gate,/action\(app.pid, 'Stop'/);
  assert.match(gate,/app.kill\(\); app.wait/);assert.match(gate,/automatic replay/);
  assert.match(gate,/fresh check changed retained interrupted workspace/);
  assert.doesNotMatch(gate,/invoke\(|UPDATE active_check|INSERT INTO active_check/);
});
