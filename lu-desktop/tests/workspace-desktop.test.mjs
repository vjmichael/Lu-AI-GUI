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

test('PLAN-01 uses native picker and existing host-bound read-only Git',()=>{
  const settings=readFileSync(new URL('../web/native_windows.jsx',import.meta.url),'utf8');
  const scanner=readFileSync(new URL('../src-tauri/src/project_discovery_desktop.rs',import.meta.url),'utf8');
  const git=readFileSync(new URL('../../src-tauri/lu-core/src/native_git_inspect.rs',import.meta.url),'utf8');
  assert.match(host,/mod project_discovery_desktop;/);
  assert.match(host,/project_discovery_desktop::lu_project_discover,/);
  assert.match(scanner,/caller\.label\(\)!="settings"/);
  assert.match(scanner,/blocking_pick_folder\(\)/);
  assert.match(scanner,/Dir::open_ambient_dir/);
  assert.match(scanner,/MAX_DIRS:usize=256/);
  assert.match(scanner,/MAX_FILES:usize=1600/);
  assert.match(scanner,/MAX_DOC_BYTES:u64=96\*1024/);
  assert.match(scanner,/remote_issue_pr_state:"not_checked_no_network"/);
  assert.match(scanner,/reported_untrusted_document_only/);
  assert.match(scanner,/workspace_desktop::trusted_git_executable\(\)/);
  assert.match(scanner,/native_git_inspect::inspect_repository_baseline/);
  assert.match(git,/ApprovedGit::open\(approved_root,trusted_git_exe\)/);
  assert.match(git,/--no-optional-locks/);
  assert.match(git,/GIT_NO_LAZY_FETCH/);
  assert.match(settings,/invoke\('lu_project_discover'\)/);
  assert.match(settings,/Remote issues\/PRs\/CI: NOT INSPECTED/);
  assert.doesNotMatch(scanner,/Command::new|reqwest|http:|https:|spawn\("llama/);
  assert.doesNotMatch(settings,/invoke\('lu_project_discover',\{.*path/);
});
