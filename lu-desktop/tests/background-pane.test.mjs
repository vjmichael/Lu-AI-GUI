import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const css=readFileSync(new URL('../web/styles.css',import.meta.url),'utf8');

test('background pane uses local persistent host task query, not model monitoring',()=>{
  assert.ok(host.includes('lu_core::tasks::TaskStore::open('));
  assert.ok(host.includes('lu-background-tasks.sqlite'));
  assert.ok(host.includes('background_tasks.reconcile_interrupted()'));
  assert.ok(host.includes('fn lu_background_tasks('));
  assert.ok(host.includes('state.background_tasks.dashboard(100)'));
  assert.ok(host.includes('lu_background_tasks,'));
  assert.ok(ui.includes("await invoke('lu_background_tasks')"));
  assert.ok(ui.includes('setInterval(refresh,2500)'));
  assert.ok(!ui.includes("invoke('lu_background_cancel'"));
});
test('pane provides operator state, history, and accessibility',()=>{
  assert.ok(ui.includes('Background Tasks'));
  assert.ok(ui.includes('aria-label="Background Tasks"'));
  assert.ok(ui.includes("backgroundFilter==='active'"));
  assert.ok(ui.includes("backgroundFilter==='all'"));
  assert.ok(ui.includes('role="progressbar"'));
  assert.ok(ui.includes('aria-valuenow={task.progress}'));
  assert.ok(ui.includes('INTERRUPTED'));
  assert.ok(ui.includes('Requires review before retrying.'));
  assert.ok(css.includes('.backgroundPaneBackdrop'));
  assert.ok(css.includes('.backgroundTaskProgress'));
});
test('host overview has no checkpoint or output path fields',()=>{
  const start=host.indexOf('struct BackgroundTaskView {');
  const end=host.indexOf('#[tauri::command]',start);
  assert.ok(start>=0);
  const section=host.slice(start,end);
  assert.ok(!section.includes('checkpoint:'));
  assert.ok(!section.includes('output_folder:'));
  assert.ok(section.includes('last_heartbeat'));
});
