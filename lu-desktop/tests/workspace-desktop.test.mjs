import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const route=readFileSync(new URL('../src-tauri/src/workspace_desktop.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');

test('Ask Workspace is native-folder-picked and never accepts WebView file paths',()=>{
  assert.match(host,/lu_workspace_ask_directory,/);
  assert.match(route,/blocking_pick_folder\(\)/);
  assert.match(route,/is_real_directory\(&root\)/);
  assert.match(route,/WorkspaceReadTurnSession::new\(/);
  assert.match(ui,/invoke\('lu_workspace_ask_directory',\{/);
  assert.match(ui,/Ask Workspace<\/button>/);
  assert.ok(!ui.includes("invoke('lu_workspace_ask_directory',{path:"));
  assert.ok(!ui.includes("invoke('lu_workspace_ask_directory',{folder:"));
});

test('authenticated human turn, queued cancellation and final answer remain host-owned',()=>{
  assert.match(route,/append_user_with_turn_id\(&chat_id,&generation_id,&prompt\)/);
  assert.match(route,/trusted_user_turn_text\(conversation_id,user_turn_id\)/);
  assert.match(route,/ticket.wait_for_slot\(\)\?/);
  assert.match(route,/ReaderRunControl\{/);
  assert.match(route,/reader_generations\.lock\(\)/);
  assert.match(route,/ModelTurnJournal::open\(/);
  assert.match(route,/run_journaled_observed\(/);
  assert.match(route,/result.tool_calls==0/);
  assert.match(route,/host.chats.complete\(&chat_id,&gid\)/);
});

test('workspace selection is opt-in and normal Send remains a separate path',()=>{
  assert.match(ui,/async function askWorkspaceDirectory\(\)/);
  assert.match(ui,/async function askReaderDocument\(\)/);
  assert.match(ui,/async function send\(e\)/);
  assert.match(ui,/onClick=\{\(\)=>void askWorkspaceDirectory\(\)\}/);
  assert.match(ui,/Choose a folder and let Lu discover files by name/);
});

test('P01 native filename/metadata discovery is advertised only in a scoped workspace turn',()=>{
  assert.match(route,/indexed filename\/metadata/);
  assert.match(route,/indexed discovery tool before repeatedly traversing folders/);
  assert.match(route,/WorkspaceReadTurnSession::new\(/);
  assert.doesNotMatch(route,/register_discovery_tool\(/);
  assert.match(ui,/Choose a folder and let Lu discover files by name/);
});
