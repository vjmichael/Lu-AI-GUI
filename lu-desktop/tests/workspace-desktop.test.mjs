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

test('PLAN-04 uses actual loaded GGUF reservation and strict citation-validated drafts, never model tool grants',()=>{
  const settings=readFileSync(new URL('../web/native_windows.jsx',import.meta.url),'utf8');
  const synthesis=readFileSync(new URL('../../src-tauri/lu-core/src/project_task_synthesis.rs',import.meta.url),'utf8');
  const bridge=readFileSync(new URL('../src-tauri/src/project_task_desktop.rs',import.meta.url),'utf8');
  assert.match(host,/mod project_task_desktop;/);
  assert.match(host,/project_task_desktop::lu_project_generate_tasks,/);
  assert.match(bridge,/caller\.label\(\)!="settings"/);
  assert.match(bridge,/blocking_pick_folder\(\)/);
  assert.match(bridge,/blocking_show\(\)/);
  assert.match(bridge,/supervisor\.reserve_generation_with_history/);
  assert.match(bridge,/supervisor\.run_reserved_with_history/);
  assert.match(bridge,/generation_queue\.enqueue/);
  assert.match(bridge,/same_observed_sources/);
  assert.match(bridge,/ansf_project_task_proposals/);
  assert.match(bridge,/INSERT OR IGNORE/);
  assert.match(synthesis,/PLAN04_MODEL_INVENTED_REQUIREMENT/);
  assert.match(synthesis,/PLAN04_MODEL_STATUS_FABRICATION/);
  assert.match(synthesis,/PLAN04_MODEL_EVIDENCE_INVENTED/);
  assert.match(synthesis,/PLAN04_DEPENDENCY_CYCLE_OR_FORWARD_REFERENCE/);
  assert.match(synthesis,/no_execution_authority:true/);
  assert.match(settings,/invoke\('lu_project_generate_tasks'\)/);
  assert.match(settings,/human review required/);
  assert.doesNotMatch(bridge,/Command::new|reqwest|git fetch|run_plan|human.*autoApprove/);
  assert.doesNotMatch(settings,/invoke\('lu_project_generate_tasks',\{.*root/);
});


test('PLAN-05 keeps operator corrections, source reconciliation and acceptance history outside model authority',()=>{
  const screen=readFileSync(new URL('../web/native_windows.jsx',import.meta.url),'utf8');
  const core=readFileSync(new URL('../../src-tauri/lu-core/src/project_task_register.rs',import.meta.url),'utf8');
  const bridge=readFileSync(new URL('../src-tauri/src/project_register_desktop.rs',import.meta.url),'utf8');
  assert.match(host,/mod project_register_desktop;/);
  for(const command of ['lu_project_register_proposal','lu_project_list_registers',
      'lu_project_revise_task','lu_project_reconcile_register','lu_project_revalidate_task']){
    assert.match(host,new RegExp('project_register_desktop::'+command+','));
    assert.match(screen,new RegExp("invoke\\('"+command+"'"));
  }
  assert.match(bridge,/settings_only\(&caller\)/);
  assert.match(bridge,/blocking_pick_folder\(\)/);
  assert.match(bridge,/blocking_show\(\)/);
  assert.match(bridge,/TransactionBehavior::Immediate/);
  assert.match(bridge,/ansf_project_task_register_revisions/);
  assert.match(bridge,/PLAN05_HISTORICAL_REVISIONS_OVERWRITTEN/);
  assert.match(core,/PLAN05_REVISIONS_CANNOT_REWRITE_PROVENANCE/);
  assert.match(core,/PLAN05_CORRECTION_CANNOT_CHANGE_EVIDENCE/);
  assert.match(core,/PLAN05_STALE_EDIT_EXPECTED_VERSION/);
  assert.match(core,/independently_verified_requirement/);
  assert.match(core,/execution_authorized:false/);
  assert.match(screen,/Durable acceptance history/);
  assert.doesNotMatch(bridge,/supervisor\.run_reserved|grant_execution|Command::new|reqwest/);
  assert.doesNotMatch(screen,/invoke\('lu_project_reconcile_register',\{.*root/);
});


test('PLAN-05 installed Windows acceptance requires true OS dialog controls and original SQLite restart evidence',()=>{
  const fixture=readFileSync(new URL('../src-tauri/src/project_register_desktop.rs',import.meta.url),'utf8');
  const ui=readFileSync(new URL('../../scripts/plan05-windows-installed-acceptance.ps1',import.meta.url),'utf8');
  const readback=readFileSync(new URL('../../scripts/plan05-readonly-ansf-witness.py',import.meta.url),'utf8');
  const workflow=readFileSync(new URL('../../.github/workflows/lu-repo01-full-source.yml',import.meta.url),'utf8');
  assert.match(fixture,/export_original_installed_ui_test_fixture/);
  assert.match(fixture,/TEST_ONLY_SYNTHETIC_MODEL_RAW_JSON_NOT_GGUF/);
  assert.match(ui,/Lu — Confirm human task correction/);
  assert.match(ui,/Confirm review only/);
  assert.match(ui,/Cancel/);
  assert.match(ui,/UIAutomationClient/);
  assert.match(ui,/PASS_BYTE_IDENTICAL_REGISTER_HEAD/);
  assert.match(ui,/after-restart-independent-readback/);
  assert.match(readback,/PRAGMA query_only=ON/);
  assert.match(readback,/previous!=before/);
  assert.match(readback,/originalReplySha256/);
  assert.match(workflow,/build-agent01g-physical-windows-nsis\.ps1/);
  assert.match(workflow,/exact-head-nsis/);
  assert.match(workflow,/LU_PLAN05_INSTALLED_SHA/);
  assert.doesNotMatch(ui,/run_reserved|grant_execution|model\.infer|allowAutoApprove/);
});


test('PLAN-06 isolated task-list surface never turns operator plan review into execution',()=>{
  const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  const native=readFileSync(new URL('../src-tauri/src/project_task_board_desktop.rs',import.meta.url),'utf8');
  const screen=readFileSync(new URL('../web/task_board.jsx',import.meta.url),'utf8');
  const shell=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const settings=readFileSync(new URL('../web/native_windows.jsx',import.meta.url),'utf8');
  assert.match(host,/label:"task-list",title:"Lu — Project Tasks"/);
  assert.match(host,/project_task_board_desktop::lu_project_board_list,/);
  for(const name of ['lu_project_board_evidence','lu_project_board_reorder',
    'lu_project_board_correct','lu_project_board_approve','lu_project_board_export']){
    assert.match(host,new RegExp('project_task_board_desktop::'+name+','));
    assert.match(screen,new RegExp("invoke\\('"+name+"'"));
  }
  assert.match(shell,/luWindow==='task-list'\?<TaskBoardWindow\/>/);
  assert.match(settings,/kind:'task-list'/);
  assert.match(native,/ansf_project_task_board_revisions/);
  assert.match(native,/TransactionBehavior::Immediate/);
  assert.match(native,/PLAN06_STALE_BOARD_HEAD/);
  assert.match(native,/PLAN06_STALE_REGISTER_HEAD/);
  assert.match(native,/PLAN06_BOARD_CHAIN_BROKEN/);
  assert.match(native,/PLAN06_EXPORT_REFUSES_OVERWRITE/);
  assert.match(native,/blocking_save_file\(\)/);
  assert.match(native,/human_approved/);
  assert.match(native,/fn preview_order/);
  assert.match(native,/Review and save this EXACT sequence/);
  assert.match(native,/Description: \{\}/);
  assert.match(native,/PLAN06_REVIEWED_ORDER_NOT_EXACT_PERMUTATION/);
  assert.match(native,/no_execution_authority:true/);
  assert.match(native,/refuse|refusal|refuses|NO_ORDER_CHANGE/i);
  assert.match(screen,/Task status filters/);
  assert.match(screen,/Evidence details/);
  assert.match(screen,/Review correction in OS dialog/);
  assert.match(screen,/Approve plan wording and order only/);
  assert.match(screen,/Export evidence-bound JSON plan/);
  assert.doesNotMatch(native,/supervisor\.run_reserved|grant_execution|Command::new|reqwest|create_task_worker/);
  assert.doesNotMatch(screen,/invoke\('lu_agent_run_reviewed_step'/);
});


test('PLAN-06 installed native task list requires independent real Windows approval/refusal, dual-project switch and restart receipts',()=>{
  const host=readFileSync(new URL('../src-tauri/src/project_task_board_desktop.rs',import.meta.url),'utf8');
  const workflow=readFileSync(new URL('../../.github/workflows/lu-repo01-full-source.yml',import.meta.url),'utf8');
  const uia=readFileSync(new URL('../../scripts/plan06-windows-installed-acceptance.ps1',import.meta.url),'utf8');
  const witness=readFileSync(new URL('../../scripts/plan06-readonly-ansf-witness.py',import.meta.url),'utf8');
  assert.match(host,/export_dual_project_installed_ui_fixture/);
  assert.match(host,/TEST_ONLY_SYNTHETIC_ORIGINAL_MODEL_REPLY_NOT_GGUF/);
  assert.match(host,/preserved_user_data/);
  assert.match(uia,/PASS_DISTINCT_AUTHENTIC_TASK_LIST_HWND/);
  assert.match(uia,/Lu — Reorder human plan/);
  assert.match(uia,/Lu — Approve task plan only/);
  assert.match(uia,/Confirm review only/);
  assert.match(uia,/PASS_REAL_NATIVE_REFUSAL_NO_APPROVAL/);
  assert.match(uia,/UIAutomationClient/);
  assert.match(uia,/project-switch-not-observed/);
  assert.match(uia,/after-installed-app-restart/);
  assert.match(uia,/PASS_ORIGINAL_SQLITE_SHA_AND_APPROVAL_CHAIN/);
  assert.match(uia,/PASS_NATIVE_OS_EXPORT_CANCEL_NO_WRITE/);
  assert.match(witness,/mode=ro/);
  assert.match(witness,/PRAGMA query_only=ON/);
  assert.match(witness,/PLAN06 immutable task revision history overwritten/);
  assert.match(witness,/PLAN06 board row invalid/);
  assert.match(workflow,/installed-plan06-windows:/);
  assert.match(workflow,/LU_PLAN06_INSTALLED_SHA/);
  assert.match(workflow,/build-agent01g-physical-windows-nsis\.ps1/);
  assert.match(workflow,/export_dual_project_installed_ui_fixture/);
  assert.match(workflow,/plan06-windows-installed-acceptance\.ps1/);
  assert.doesNotMatch(uia,/grant_execution|model\.infer|allowAutoApprove|run_reserved/);
});


test('PLAN-07A evidence delta is host-verified and never self-authorizes completion',()=>{
  const core=readFileSync(new URL('../../src-tauri/lu-core/src/project_task_updates.rs',import.meta.url),'utf8');
  const host=readFileSync(new URL('../src-tauri/src/project_updates_desktop.rs',import.meta.url),'utf8');
  const ui=readFileSync(new URL('../web/task_board.jsx',import.meta.url),'utf8');
  const app=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  assert.match(app,/project_updates_desktop::lu_project_update_inspect,/);
  assert.match(app,/project_updates_desktop::lu_project_update_apply,/);
  assert.match(ui,/invoke\('lu_project_update_inspect'/);
  assert.match(ui,/invoke\('lu_project_update_apply'/);
  assert.match(ui,/Exact criterion review candidates/);
  assert.match(core,/project_task_register as register/);
  assert.match(core,/same_observed_sources/);
  assert.match(core,/PLAN07_WRONG_SELECTED_PROJECT_SCOPE/);
  assert.match(core,/Disposition::RequirementMissing/);
  assert.match(core,/Disposition::ExactCriterionReviewCandidate/);
  assert.match(core,/task_complete:false/);
  assert.match(core,/execution_authorized:false/);
  assert.match(core,/human_review_required:true/);
  assert.match(host,/PLAN07_REQUIRES_INDEPENDENT_HUMAN_TASK_WINDOW/);
  assert.match(host,/blocking_pick_folder\(\)/);
  assert.match(host,/project_status_desktop::classify_selected\(root\)/);
  assert.match(host,/ansf_project_task_refresh_observations/);
  assert.match(host,/TransactionBehavior::Immediate/);
  assert.match(host,/PLAN07_REGISTER_CHANGED_AFTER_PREVIEW/);
  assert.match(host,/PLAN07_SOURCE_CHANGED_AFTER_OPERATOR_REVIEW/);
  assert.match(host,/register::source_changed\(/);
  assert.match(host,/project_register_desktop::persist\(/);
  assert.match(host,/Lu — Review changed native task evidence/);
  assert.doesNotMatch(host,/supervisor\.run_reserved|grant_execution|Command::new|reqwest|auto_accept|git commit/);
  assert.doesNotMatch(ui,/invoke\('lu_agent_run_reviewed_step'/);
});
