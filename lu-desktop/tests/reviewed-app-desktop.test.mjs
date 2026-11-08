import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const desktop=readFileSync(new URL('../src-tauri/src/reviewed_app_desktop.rs',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const tool=readFileSync(new URL('../../src-tauri/lu-core/src/native_reviewed_app_tool.rs',import.meta.url),'utf8');

test('next-Send reviewed Git and media require explicit human selection and native OS picker',()=>{
  assert.match(ui,/reviewedAppNextSend/);
  assert.match(ui,/Use reviewed Git on Send/);
  assert.match(ui,/Use reviewed media on Send/);
  assert.match(ui,/invoke\('lu_reviewed_app_ask',\s*\{/);
  assert.match(desktop,/blocking_pick_file\(\)/);
  assert.match(desktop,/blocking_pick_folder\(\)/);
  assert.match(desktop,/matches!\(mode.as_str\(\),"git"\|"media"\)/);
  assert.match(desktop,/PersistedReviewedHumanTurn/);
  assert.match(desktop,/trusted_user_turn_text\(chat,turn\)/);
  assert.match(host,/lu_reviewed_app_ask,/);
  assert.doesNotMatch(ui,/invoke\('lu_reviewed_app_ask',\s*\{[^}]*\b(?:path|executable|argv|permission)\s*:/s);
});

test('host-provided exact operation binding never infers model permission from discovery',()=>{
  assert.match(tool,/pub fn register_reviewed_application_tools\(/);
  assert.match(tool,/pub struct ReviewedApplicationTurnSession/);
  assert.match(tool,/ModelToolCatalog/);
  assert.match(tool,/NativeToolRegistry/);
  assert.match(tool,/NativeToolTurn::new\(/);
  assert.match(tool,/expect_fields\(args,&\[\],&\[\]\)/);
  assert.match(tool,/allowed.contains\(&record.read_tool_id\)/);
  assert.match(tool,/allowed.contains\(&record.execute_tool_id\)/);
  assert.match(tool,/run_supervised_native_turn_journaled_observed_with_host_routing/);
  assert.match(desktop,/open_user_task\(TaskAuthority/);
  assert.match(desktop,/ReviewedApplicationTurnSession::from_verified_host_task/);
  assert.doesNotMatch(tool,/Command::new\(/);
});

test('reviewed child process uses separate durable receipt and cancellation fencing',()=>{
  assert.match(tool,/format!\("\{call_id\}-\{\}",operation.suffix\(\)\)/);
  assert.match(tool,/journal.get\(&worker_id\)/);
  assert.match(tool,/EffectState::Applied/);
  assert.match(tool,/ReviewedCancelWatch::start/);
  assert.match(tool,/authority.revoke_task\(&task_id\)/);
  assert.match(desktop,/reader_generations\.lock\(\)/);
  assert.match(desktop,/run_journaled_observed\(/);
  assert.match(desktop,/answer.tool_calls==0/);
  assert.match(desktop,/host.chats.complete\(&chat_id,&gid_run\)/);
});

test('desktop only permits reviewed Git reads or FFprobe WAV/MP4 facts, never arbitrary shell',()=>{
  assert.match(desktop,/GitReadOperation::HeadCommit/);
  assert.match(desktop,/GitReadOperation::HistoryCount/);
  assert.match(desktop,/ReviewedContainer::Wave/);
  assert.match(desktop,/ReviewedContainer::Mp4/);
  assert.match(desktop,/if extension.eq_ignore_ascii_case\("wav"\)/);
  assert.match(desktop,/file must be WAV or MP4/i);
  assert.doesNotMatch(desktop,/fn execute_arbitrary_command\(/);
});
