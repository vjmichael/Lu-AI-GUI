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
  // Chat-first UX removed the old specialist button. Host intent and picker remain.
  assert.match(host,/workspace_desktop::explicit_read_workspace_request\(&prompt\)/);
  assert.doesNotMatch(ui,/Ask Workspace<\/button>/);
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
  assert.match(host,/workspace_desktop::ask_workspace_for_chat\(/);
  assert.match(route,/append_user_with_turn_id\(&chat_id,&generation_id,&prompt\)/);
  assert.match(route,/blocking_pick_folder\(\)/);
  assert.match(host,/workspace_desktop::explicit_read_workspace_request\(&prompt\)/);
  assert.match(route,/read_workspace/);
});

test('P01 native filename/metadata discovery is advertised only in a scoped workspace turn',()=>{
  assert.match(route,/indexed filename\/metadata/);
  assert.match(route,/indexed discovery tool before repeatedly traversing folders/);
  assert.match(route,/WorkspaceReadTurnSession::new\(/);
  assert.doesNotMatch(route,/register_discovery_tool\(/);
  assert.match(route,/blocking_pick_folder\(\)/);
  assert.match(host,/workspace_desktop::explicit_read_workspace_request\(&prompt\)/);
  assert.match(route,/read_workspace/);
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

test('PLAN-02 schema and native folder boundary remain host owned',()=>{
  const screen=readFileSync(new URL('../web/native_windows.jsx',import.meta.url),'utf8');
  const graph=readFileSync(new URL('../../src-tauri/lu-core/src/project_evidence_graph.rs',import.meta.url),'utf8');
  const hostGraph=readFileSync(new URL('../src-tauri/src/project_evidence_desktop.rs',import.meta.url),'utf8');
  assert.match(host,/mod project_evidence_desktop;/);
  assert.match(host,/project_evidence_desktop::lu_project_evidence_graph,/);
  assert.match(hostGraph,/caller\.label\(\)!="settings"/);
  assert.match(hostGraph,/blocking_pick_folder\(\)/);
  assert.match(hostGraph,/project_discovery_desktop::scan_selected/);
  assert.match(hostGraph,/OpenFlags::SQLITE_OPEN_READ_WRITE/);
  assert.match(hostGraph,/ansf_project_evidence_snapshots/);
  assert.match(hostGraph,/INSERT OR IGNORE/);
  assert.match(graph,/pub enum EvidenceState\{Reported,Inspected,IndependentlyVerified,Stale,Unknown\}/);
  assert.match(graph,/PLAN02_FALSE_VERIFICATION/);
  assert.match(graph,/PLAN02_SNAPSHOT_CHANGED/);
  assert.match(screen,/invoke\('lu_project_evidence_graph'\)/);
  assert.match(screen,/PLAN-03 classifier, PLAN-04 GGUF task synthesis/);
  assert.doesNotMatch(hostGraph,/Command::new|reqwest|git fetch|model\.infer|run_plan/);
  assert.doesNotMatch(screen,/invoke\('lu_project_evidence_graph',\{.*root/);
});

test('PLAN-03 classification is native-only, graph-audited, ANSF-persisted and never a model grant',()=>{
  const settings=readFileSync(new URL('../web/native_windows.jsx',import.meta.url),'utf8');
  const classifier=readFileSync(new URL('../../src-tauri/lu-core/src/project_status_classifier.rs',import.meta.url),'utf8');
  const bridge=readFileSync(new URL('../src-tauri/src/project_status_desktop.rs',import.meta.url),'utf8');
  assert.match(host,/mod project_status_desktop;/);
  assert.match(host,/project_status_desktop::lu_project_classify,/);
  assert.match(bridge,/caller\.label\(\)!="settings"/);
  assert.match(bridge,/blocking_pick_folder\(\)/);
  assert.match(bridge,/project_evidence_desktop::assemble_from_selected/);
  assert.match(bridge,/classifier::observe_project_paths/);
  assert.match(bridge,/inspect_repository_baseline/);
  assert.match(bridge,/ansf_project_status_snapshots/);
  assert.match(bridge,/INSERT OR IGNORE/);
  assert.match(classifier,/pub enum Status\{Missing,Partial,NotIntegrated,Broken,Unverified,Accepted,Blocked,Unknown\}/);
  assert.match(classifier,/eg::audit\(g\)/);
  assert.match(classifier,/PLAN03_FOREIGN_OR_DUPLICATE_PATH_PROOF/);
  assert.match(settings,/invoke\('lu_project_classify'\)/);
  assert.match(settings,/PLAN-04 task synthesis is separate/);
  assert.doesNotMatch(bridge,/Command::new|reqwest|git fetch|run_plan|model\.infer/);
  assert.doesNotMatch(settings,/invoke\('lu_project_classify',\{.*root/);
});
