// AGENT-01L source/authorization contracts, NOT a substitute for native OS CI.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const windows=readFileSync(new URL('../../src-tauri/lu-win-worker-job/src/lib.rs',import.meta.url),'utf8');
const processHost=readFileSync(new URL('../../src-tauri/lu-core/src/native_background_process.rs',import.meta.url),'utf8');
const tasks=readFileSync(new URL('../../src-tauri/lu-core/src/tasks.rs',import.meta.url),'utf8');
const custody=readFileSync(new URL('../../src-tauri/lu-core/src/agent_os_custody.rs',import.meta.url),'utf8');
const core=readFileSync(new URL('../../src-tauri/lu-core/src/agent_recovery_decision.rs',import.meta.url),'utf8');
const desktop=readFileSync(new URL('../src-tauri/src/agent_recovery_desktop.rs',import.meta.url),'utf8');

test('kernel witness is emitted only by an assigned live Job handle',()=>{
  assert.match(windows,/pub struct WindowsJobEmptyProof\{_private:\(\)\}/);
  assert.match(windows,/self\.1\.load\(Ordering::Acquire\)/);
  assert.match(windows,/QueryInformationJobObject\(/);
  assert.match(windows,/accounting\.active_processes==0/);
  assert.match(windows,/TerminateJobObject\(self\.0,1\)/);
  assert.match(windows,/JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE/);
});
test('worker admitted before first code and OS proof gathered before closing Job',()=>{
  assert.match(processHost,/job\.assign_and_resume_suspended\(&child\)/);
  assert.match(processHost,/job\.terminate_and_verify_empty\(\)/);
  assert.match(processHost,/job_empty:Option<lu_win_worker_job::WindowsJobEmptyProof>/);
  assert.match(processHost,/drop\(owner\.job\.take\(\)\)/);
});
test('receipt is published only for a completed typed operation',()=>{
  assert.match(processHost,/matches!\(outcome,Some\(true\)\)&&had_version_reader&&version\.is_some\(\)/);
  assert.match(processHost,/record_verified_windows_job_empty\(/);
  assert.match(tasks,/t\.state='COMPLETED'/);
  assert.match(tasks,/o\.user_turn_id=\?2/);
  assert.match(tasks,/proof\.method\(\)/);
});
test('exact immutable original-turn Windows custody row is not a grant',()=>{
  assert.match(tasks,/CREATE TABLE IF NOT EXISTS lu_task_windows_job_custody/);
  assert.match(tasks,/WINDOWS_JOB_ACTIVE_ZERO_V1/);
  assert.match(tasks,/lu_task_os_custody_no_update/);
  assert.match(tasks,/lu_task_os_custody_no_delete/);
  assert.doesNotMatch(custody,/ConversationAuthority::new|TaskAuthority\{|Command::new|\.spawn\(/);
});
test('persisted witness reopened only from host profile and bound chat, turn, call',()=>{
  assert.match(custody,/lu-reviewed-task-\{turn\}\.sqlite/);
  assert.match(custody,/SQLITE_OPEN_READ_ONLY/);
  assert.match(custody,/o\.conversation_id/);
  assert.match(custody,/j\.workspace_id=\?2/);
  assert.match(custody,/saved_chat==chat/);
  assert.match(custody,/saved_turn==turn/);
  assert.match(custody,/bounded_journal\(&file,false\)/);
});
test('Linux proof requires original scoped cgroup populated-zero receipt; PGID alone is insufficient',()=>{
  assert.match(custody,/#\[cfg\(target_os="linux"\)\]/);
  assert.match(custody,/linux_original_cgroup_empty\(/);
  for(const kind of ['GIT','PROBE','NATIVE']){
    assert.match(custody,new RegExp('LU_SEC06C_OWNED_'+kind+'_V1'));
  }
  assert.doesNotMatch(custody,/LU_CGROUP_V2_EMPTY_V1/);
  assert.match(custody,/POPULATED=0/);
  assert.match(custody,/linux_pgid_guardian_never_claims_escaped_descendant_custody/);
  assert.match(core,/STOP_ANCESTOR_CUSTODY_UNPROVEN/);
});
test('actual OS acceptance includes nested Job and reviewed Windows Git journal',()=>{
  assert.match(windows,/fn native_job_terminates_descendant_cmd_tree_and_queries_kernel_empty\(/);
  assert.match(windows,/fn unassigned_job_cannot_forge_an_empty_process_tree\(/);
  assert.match(processHost,/fn agent01l_real_git_job_attests_original_turn_and_reopens_without_replay\(/);
  assert.match(processHost,/start_host_approved_git_read\(/);
  assert.match(processHost,/HostOriginalJobCustody::from_host_profile/);
});
test('installed desktop recovers OS evidence, still returns no tool authorization',()=>{
  assert.match(desktop,/HostOriginalJobCustody::from_host_profile\(profile\)/);
  assert.match(core,/execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION"/);
  assert.match(core,/REVIEW_NEW_STEP_WITH_FRESH_AUTHORIZATION_ONLY/);
  assert.doesNotMatch(desktop,/NoIndependentCustodyProof/);
});

test('fresh child preparation checks parent OS custody before dialog and intent',()=>{
  const prepare=readFileSync(new URL('../src-tauri/src/agent_plan_continuation_desktop.rs',import.meta.url),'utf8');
  const n=(prepare.match(/require_parent_os_custody_for_fresh_plan\(/g)||[]).length;
  assert.equal(n,2);
  const first=prepare.indexOf('require_parent_os_custody_for_fresh_plan(');
  const dialog=prepare.indexOf('.blocking_show()');
  const second=prepare.lastIndexOf('require_parent_os_custody_for_fresh_plan(');
  const intent=prepare.indexOf('stage_fresh_continuation(');
  assert.ok(first<dialog && dialog<second && second<intent,
    'No prior kernel custody: do not create a new child plan');
});
test('reviewed execution rechecks real OS custody before native picker and before host claim',()=>{
  const broker=readFileSync(new URL('../src-tauri/src/reviewed_app_desktop.rs',import.meta.url),'utf8');
  const checks=broker.match(/require_fresh_child_os_custody_before_worker\(/g)||[];
  assert.equal(checks.length,2);
  assert.match(broker,/require_fresh_child_os_custody_before_worker\([\s\S]*?Ok\(active\.original_human_turn_id\)/);
  assert.match(broker,/require_fresh_child_os_custody_before_worker\([\s\S]*?let owner=next_id\("h-"\)/);
  const recovery=readFileSync(new URL('../src-tauri/src/agent_recovery_desktop.rs',import.meta.url),'utf8');
  assert.match(recovery,/status!="REVIEW_FIRST_STEP_ONLY"/);
  assert.match(recovery,/status!="REVIEW_NEW_STEP_WITH_FRESH_AUTHORIZATION_ONLY"/);
});
