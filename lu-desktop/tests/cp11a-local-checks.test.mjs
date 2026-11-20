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
