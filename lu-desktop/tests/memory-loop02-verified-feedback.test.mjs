import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=(path)=>readFileSync(new URL(path,import.meta.url),'utf8');
const ansf=read('../../src-tauri/lu-core/src/ansf_live_chat.rs');
const chats=read('../src-tauri/src/ansf_chat_store.rs');
const reader=read('../src-tauri/src/reviewed_task_continuity.rs');
const feedback=read('../src-tauri/src/agent_verified_feedback.rs');
const desktop=read('../src-tauri/src/lib.rs');

test('original ANSF index selects prior host-authenticated humans by source event order',()=>{
  assert.match(ansf,/pub fn recent_authenticated_turns_before\(/);
  assert.match(ansf,/seq<\?2 AND kind='user'/);
  assert.match(ansf,/origin='host' AND user_turn_id IS NOT NULL/);
  assert.match(ansf,/source_entry_id IS NOT NULL/);
  assert.match(ansf,/ORDER BY seq DESC LIMIT \?3/);
  assert.match(ansf,/limit>16/);
});
test('current human turn is independently rechecked after queueing',()=>{
  assert.match(chats,/pub fn prior_authenticated_turn_ids\(/);
  assert.match(chats,/latest_authenticated_user_turn_id\(chat_id\)/);
  assert.match(chats,/recent_authenticated_turns_before\(chat_id,current_turn,limit\)/);
  assert.match(feedback,/trusted_user_turn_text\(chat,current_turn\)/);
  assert.match(feedback,/latest_authenticated_user_turn_id\(chat\)/);
});
test('native reviewed effect reader retains independent digest without serializing internal call identifiers',()=>{
  assert.match(reader,/independently_observed_sha256:Option<String>/);
  assert.match(reader,/call_id:String/);
  assert.ok(reader.split('#[serde(skip_serializing)]').length>=3);
  assert.match(reader,/Sha256::digest\(receipt\)/);
  assert.match(reader,/expected_method\(tool,&method\)/);
  assert.match(reader,/Ok\(\("VERIFIED_AT_OBSERVATION",observed\)\)/);
});
test('next model reasoning turn observes prior independent evidence without scanning arbitrary folders',()=>{
  assert.match(desktop,/mod agent_verified_feedback;/);
  assert.match(desktop,/agent_verified_feedback::for_next_human_turn\(/);
  assert.match(desktop,/origin==GenerationOrigin::Human&&images\.is_empty\(\)/);
  assert.match(feedback,/prior_authenticated_turn_ids\(/);
  assert.match(feedback,/reviewed_turn_continuity\(profile,chat,&turn\)/);
  // The genuine-Git test-only fixture is intentionally allowed to start Git.
  // Production feedback generation is still forbidden to enumerate directories
  // or execute subprocesses, including when tests are compiled alongside it.
  const production=feedback.split('#[cfg(test)]')[0];
  assert.doesNotMatch(production,/read_dir\(|glob\(|\.walk\(|Command::new\(|\.spawn\(/);
});
test('source VERIFIED alone is not claimed as descendant termination',()=>{
  assert.match(feedback,/HostOriginalJobCustody::from_host_profile\(profile\)/);
  assert.match(feedback,/custody\.worker_and_descendants_ended\(/);
  assert.match(feedback,/SOURCE_VERIFIED_OS_CUSTODY_UNPROVEN/);
  assert.match(feedback,/HISTORICALLY_VERIFIED_WITH_OS_EMPTY_WITNESS/);
  assert.match(feedback,/NOT_INDEPENDENTLY_PROVEN/);
  assert.match(feedback,/currentSourceFreshness/);
});
test('conflict unsupported and uncertain worker effects are never promoted',()=>{
  assert.match(feedback,/INDEPENDENT_SOURCE_CONFLICT/);
  assert.match(feedback,/INDEPENDENT_SOURCE_UNSUPPORTED/);
  assert.match(feedback,/UNCERTAIN_OR_UNVERIFIED/);
  assert.match(feedback,/PARTIAL_TASK_REQUIRES_RECONCILIATION/);
  assert.match(feedback,/!valid_origin\|\|!fully_checked/);
  assert.match(reader,/origin\.as_ref\(\)\.is_none_or/);
});
test('only independently sourced nonexecuting facts enter bounded model feedback',()=>{
  assert.match(feedback,/MAX_PRIOR_HUMAN_TURNS:usize=12/);
  assert.match(feedback,/MAX_FEEDBACK_TURNS:usize=2/);
  assert.match(feedback,/MAX_FEEDBACK_BYTES:usize=3400/);
  assert.match(feedback,/originalHumanTurnId/);
  assert.match(feedback,/independentObservedSha256/);
  assert.match(feedback,/HISTORICAL_INDEPENDENT_FEEDBACK_DATA_ONLY/);
  assert.match(desktop,/model_prompt\.len\(\)\+envelope\.len\(\)<=16\*1024/);
});
test('old receipts cannot issue new permissions or cause automatic replay',()=>{
  for(const phrase of [
    '"historicalEffectMayBeReplayed":false',
    '"priorGrantReusable":false',
    '"executionAuthority":"NOT_AUTHORIZED_FOR_EXECUTION"',
    '"automaticRetryAllowed":false'
  ])assert.ok(feedback.includes(phrase));
  assert.doesNotMatch(feedback,/TaskAuthority\{|ConversationAuthority::|TaskStore::resume\(|claim_host\(|run_journaled_observed\(/);
});
test('Linux or Windows missing OS custody does not change independent oracle classification to PASS',()=>{
  assert.match(feedback,/source_oracle_verdict_and_kernel_custody_are_separate/);
  assert.match(feedback,/assert_eq!\(source_only\["feedbackStatus"\]/);
  assert.match(feedback,/currentSourceFreshness"\],"NOT_RECHECKED"/);
  assert.match(feedback,/mayExecuteOrReplay"\],false/);
});
test('real native ANSF task/effect SQLite restart and changed independent digest are tested',()=>{
  assert.match(feedback,/authenticated_original_feedback_persists_after_restart_and_fails_on_tamper/);
  assert.match(feedback,/TaskStore::open\(&taskfile\)/);
  assert.match(feedback,/EffectJournal::open\(&effectfile\)/);
  assert.match(feedback,/record_independent_outcome\("review-gh"/);
  assert.match(feedback,/drop\(effects\);drop\(tasks\);drop\(chats\)/);
  assert.match(feedback,/UPDATE lu_independent_outcomes SET receipt_sha256=/);
});
test('cross-chat and missing original journals cannot fabricate an outcome',()=>{
  assert.match(feedback,/no_cross_chat_feedback_and_no_original_journal_creation/);
  assert.match(feedback,/for_next_human_turn\(/);
  assert.match(feedback,/\.unwrap\(\)\.is_none\(\)/);
  assert.match(feedback,/lu-reviewed-task-\{past\}\.sqlite/);
  assert.match(desktop,/FEEDBACK_UNAVAILABLE_RECONCILE/);
});
