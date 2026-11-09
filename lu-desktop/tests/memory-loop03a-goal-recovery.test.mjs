import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const host=read('../src-tauri/src/memory_loop03_goal_gate.rs');
const lib=read('../src-tauri/src/lib.rs');
const original=read('../src-tauri/src/agent_recovery_provenance.rs');
const goals=read('../src-tauri/src/memory_desktop.rs');
const auth=read('../src-tauri/src/chat_store.rs');
const reviewed=read('../src-tauri/src/host_safe_continuation.rs');

test('03A host-only deterministic goal and recovery classifier has no runtime authority',()=>{
  assert.match(host,/pub\(crate\) fn classify_current_human_turn\(/);
  assert.match(host,/GoalRead::Unavailable/);
  assert.match(host,/RecoveryRead::Unavailable/);
  assert.match(host,/execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION"/);
  assert.match(host,/prior_grant_reusable:false/);
  assert.match(host,/automatic_retry_allowed:false/);
  assert.match(host,/model_may_override_host_decision:false/);
  assert.doesNotMatch(host,/std::process::Command|\.spawn\(|\.claim_host\(|\.mark_applied\(|\.authorize\(|\.resume\(/);
});
test('actual normal GGUF turn derives authenticated current original human turn',()=>{
  assert.match(lib,/mod memory_loop03_goal_gate;/);
  assert.match(lib,/latest_authenticated_user_turn_id\(&chat_id\)/);
  assert.match(lib,/trusted_user_turn_text\(&chat_id,&gid\)/);
  assert.match(lib,/Some\(prompt\.as_str\(\)\)/);
  assert.match(lib,/origin==GenerationOrigin::Human&&images\.is_empty\(\)/);
  assert.match(auth,/pub fn latest_authenticated_user_turn_id\(/);
});
test('current ANSF goal status is projected by native host, not model prose',()=>{
  assert.match(lib,/host_memory\.snapshot\(/);
  assert.match(lib,/goal_id_present:snapshot\.goal_id\.is_some\(\)/);
  assert.match(lib,/goal_revision_present:/);
  assert.match(lib,/q\.status=="pending"/);
  assert.match(lib,/q\.status=="deferred"/);
  assert.match(lib,/answered"\|"pending"\|"deferred"\|"superseded"/);
  assert.match(goals,/project_goal_events\(&records\)/);
});
test('historical recovery is original independent source and OS custody status',()=>{
  assert.match(lib,/agent_recovery_provenance::read_original_recovery\(/);
  assert.match(lib,/status:history\.status/);
  assert.match(lib,/host_safe_next_action:/);
  assert.match(lib,/history\.older_host_turns_not_inspected/);
  assert.match(original,/reviewed_turn_continuity\(/);
  assert.match(original,/HostOriginalJobCustody::from_host_profile\(profile\)/);
  assert.match(reviewed,/std::cmp::max\(self,other\)/);
});
test('unknown history or goal fails closed and old permission is never reused',()=>{
  assert.match(host,/HISTORICAL_RECOVERY_UNAVAILABLE/);
  assert.match(host,/ORIGINAL_GOAL_MEMORY_UNAVAILABLE/);
  assert.match(host,/OLDER_ORIGINAL_HUMAN_TURNS_UNREAD/);
  assert.match(host,/CURRENT_HUMAN_TURN_UNAUTHENTICATED/);
  assert.match(host,/draft_review_context_ready:context_ready/);
  assert.match(host,/previous_effects_need_reconciliation:/);
  assert.match(host,/NO_REVIEWED_EFFECTS_IN_BOUNDED_WINDOW/);
});
test('deferred clarification is not consent; historical conflict still blocks review',()=>{
  assert.match(host,/DO_NOT_ASSUME_DEFERRED_ANSWER_OR_REPEAT_WITHOUT_REOPENING/);
  assert.match(host,/PENDING_ORIGINAL_HUMAN_CLARIFICATION/);
  assert.match(host,/RECONCILE_INDEPENDENT_SOURCE_BEFORE_NEW_EFFECT/);
  assert.match(host,/RECONCILE_OS_CUSTODY_BEFORE_NEW_EFFECT/);
  assert.match(host,/pending_and_not_now_are_distinct_and_never_count_as_yes/);
});
test('status capsule is bounded, nonexecuting and supplied to real current-model context',()=>{
  assert.match(lib,/lu-memory-loop03a-current-goal-recovery-v1/);
  assert.match(lib,/CURRENT_HUMAN_GOAL_AND_HISTORICAL_EFFECT_DATA_ONLY/);
  assert.match(lib,/modelMayOverrideHostDecision/);
  assert.match(lib,/encoded=serde_json::to_string\(&data\)/);
  assert.match(lib,/envelope\.len\(\)<=2300/);
  assert.match(lib,/model_prompt\.len\(\)\+envelope\.len\(\)<=16\*1024/);
  assert.match(lib,/Lu goal\/recovery preflight could not fit the bounded model context/);
  const section=lib.split('            // MEMORY-LOOP-03A:')[1]
    .split('            // M3: lexical cross-chat recall')[0];
  assert.doesNotMatch(section,/run_journaled_observed|claim_host|ToolInvocation|TaskStore::resume/);
  assert.match(section,/RecoveryRead::Unavailable/);
});
test('nine native Rust policy regressions are submitted including negative cases',()=>{
  const names=[
    'fully_historical_witnessed_goal_is_review_only_not_execution',
    'current_turn_must_be_authenticated_even_if_old_evidence_good',
    'no_goal_is_general_chat_not_a_fabricated_plan_authority',
    'pending_and_not_now_are_distinct_and_never_count_as_yes',
    'incomplete_goal_or_unknown_question_status_fails_closed',
    'missing_os_conflict_or_old_original_prevents_recovered_plan_clearance',
    'bounded_no_history_is_not_historical_proof_or_old_permission',
    'empty_unread_or_unknown_host_status_never_implies_clearance',
    'historical_conflict_cannot_be_hidden_by_answered_questions'
  ];
  for(const name of names)assert.match(host,new RegExp('fn '+name+'\\('));
  assert.equal((host.match(/#\[test\]/g)||[]).length,9);
});
