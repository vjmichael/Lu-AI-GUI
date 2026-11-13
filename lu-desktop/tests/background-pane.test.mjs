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
  assert.ok(host.includes('state.background_tasks.dashboard_for_workspace(&chat_id,100)'));
  assert.ok(host.includes('lu_background_tasks,'));
  assert.ok(ui.includes("invoke('lu_background_tasks',{chatId})"));
  assert.ok(ui.includes('setInterval(refresh,2500)'));
  assert.ok(ui.includes("await invoke('lu_background_cancel', {taskId,chatId})"));
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
  assert.ok(ui.includes('task.canCancel &&'));
  assert.ok(host.includes('fn lu_background_cancel('));
  assert.ok(host.includes('scheduler.cancel_owned(&task_id,&chat_id)'));
  assert.ok(host.includes('Terminate task'));
  assert.ok(ui.includes('Queued tasks can be paused, resumed or cancelled'));

  assert.ok(css.includes('.backgroundPaneBackdrop'));
  assert.ok(css.includes('.backgroundTaskProgress'));
});
test('real native file-scan worker connects the system picker to the pane',()=>{
  assert.ok(host.includes('async fn lu_background_scan_file('));
  assert.ok(host.includes('lu_background_scan_file,'));
  assert.ok(host.includes('.blocking_pick_file()'));
  assert.ok(host.includes('std::env::current_exe()'));
  assert.ok(host.includes('progress_protocol:true'));
  assert.ok(host.includes('scheduler.enqueue_host_plan(plan,options)'));
  assert.ok(ui.includes("await invoke('lu_background_scan_file',{"));
  assert.ok(ui.includes('Scan media file…'));
  assert.ok(ui.includes('Native SHA-256 integrity scan'));
  assert.ok(css.includes('.backgroundScanLaunch'));
});
test('Lu host pushes end notifications without model polling, and routes them into the next chat turn',()=>{
  assert.ok(host.includes('pending_terminal_notices_after('));
  assert.ok(host.includes('app.emit("lu-background-terminal"'));
  assert.ok(host.includes('pending_notices_for_workspace(&chat_id,8)'));
  assert.ok(host.includes('acknowledge_terminal_notice('));
  assert.ok(host.includes('state.chats.complete(&chat_id, &gid)?'));
  assert.ok(ui.includes("listen('lu-background-terminal'"));
  assert.ok(ui.includes('backgroundGlobalNotice'));
  assert.ok(ui.includes('Background task ended:'));
  assert.ok(ui.includes("await invoke('lu_background_scan_file',{"));
  assert.ok(!ui.includes("invoke('lu_generate', {taskId"));
});
test('explicit native opt-in is required before Lu can proactively ask the model to respond',()=>{
  assert.ok(host.includes('fn lu_background_auto_continue('));
  assert.ok(host.includes('state.chats.history(&chat_id)?'));
  assert.ok(host.includes('set_auto_continue(&chat_id,approved)'));
  assert.ok(host.includes('auto_continue_enabled(&chat_id)'));
  assert.ok(host.includes('fn start_background_watchdog('));
  assert.match(host,/if admission\.defer_new_background \|\|\s*state\.generation_queue\.has_jobs\(\)\{continue;\}/);
  assert.ok(host.includes('let Some(session_id)=model.session_id else{continue};'));
  assert.ok(host.includes('GenerationOrigin::BackgroundTerminal'));
  assert.ok(host.includes('generate_internal('));
  assert.ok(host.includes('origin==GenerationOrigin::Human'));
  assert.ok(host.includes('state.background_tasks.pending_notices_for_workspace(&chat_id,1)'));
  assert.ok(ui.includes("invoke('lu_background_auto_continue'"));
  assert.ok(ui.includes('checked={Boolean(chatId&&autoBackgroundContinuation)}'));
  assert.ok(ui.includes('Automatically notify this conversation'));
  assert.ok(ui.includes("listen('lu-generation-finish'"));
  assert.ok(ui.includes('loadLatestUiHistory(finishedChat)'));
  assert.ok(ui.includes("listen('lu-generation-finish'"));
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

test('P02 recurring schedule review requires real OS picker and independent confirmation',()=>{
  assert.ok(host.includes('lu_background_reapprove,'));
  assert.ok(host.includes('Reapprove scheduled work'));
  assert.ok(host.includes('scheduler.reapprove_reviewed(&review_task_id,'));
  assert.ok(ui.includes("invoke('lu_background_reapprove'"));
  assert.ok(ui.includes('task.needsReview'));
  assert.ok(ui.includes('Reapprove with a selected file'));
});
