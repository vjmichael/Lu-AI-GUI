import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const policy=read('../src-tauri/src/host_safe_continuation.rs');
const feedback=read('../src-tauri/src/agent_verified_feedback.rs');
const lib=read('../src-tauri/src/lib.rs');
const original=read('../src-tauri/src/reviewed_task_continuity.rs');

test('decision is host-controlled pure Rust and provides no execution capability',()=>{
  assert.match(policy,/pub\(crate\) enum HostSafeNext/);
  assert.match(policy,/pub\(crate\) fn for_evidence\(/);
  assert.doesNotMatch(policy,/Command::new|\.spawn\(|TaskStore::resume|claim_host|\bunsafe\s*\{/);
});
test('all four recovery classes are closed source-ranked labels',()=>{
  for(const label of [
    'RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION',
    'MANUAL_RECONCILIATION_BEFORE_NEW_ACTION',
    'RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION',
    'RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION'
  ]) assert.ok(policy.includes(label));
  assert.match(policy,/std::cmp::max\(self,other\)/);
});
test('source verified alone cannot bypass OS evidence and current source recheck',()=>{
  assert.match(policy,/full_descendant_custody_attested/);
  assert.match(policy,/if !source\.full_descendant_custody_attested/);
  assert.match(policy,/ReconcileOsCustodyThenFreshSourceAndHumanAuthorization/);
});
test('conflicts, interrupted effects, missing origins or partial reading are stronger STOPs',()=>{
  assert.match(policy,/source\.independent_oracle!="VERIFIED_AT_OBSERVATION"/);
  assert.match(policy,/source\.omitted_history/);
  assert.match(policy,/source\.original_model_call_linked/);
  assert.match(policy,/source\.reviewed_worker_completed/);
  assert.match(policy,/missing_origin_truncated_history_and_empty_operations_fail_closed/);
  assert.match(policy,/partial_history_precedence_is_order_independent_and_never_silent_pass/);
});
test('original ANSF and independent SQLite reconstructions control the decision',()=>{
  assert.match(feedback,/reviewed_turn_continuity\(profile,chat,&turn\)/);
  assert.match(feedback,/HostOriginalJobCustody::from_host_profile\(profile\)/);
  assert.match(feedback,/HostSafeNext::for_evidence\(HostReviewedEvidence\{/);
  assert.match(feedback,/turn_safe_next=turn_safe_next\.restrict\(decision\)/);
  assert.match(feedback,/safe_next=safe_next\.restrict\(turn_safe_next\)/);
  assert.match(original,/Sha256::digest\(receipt\)/);
});
test('host-authenticated human gets data-only non-grantable decision capsule',()=>{
  assert.match(lib,/mod host_safe_continuation;/);
  assert.match(feedback,/latest_authenticated_user_turn_id\(chat\)/);
  assert.match(feedback,/trusted_user_turn_text\(chat,current_turn\)/);
  assert.match(feedback,/"hostSafeNextAction":safe_next\.code\(\)/);
  assert.match(feedback,/"modelMayOverrideHostSafeDecision":false/);
  assert.match(feedback,/"priorGrantReusable":false/);
  assert.match(feedback,/"automaticRetryAllowed":false/);
  assert.match(lib,/RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION/);
});
test('native test reopens original SQLite and escalates a tampered effect',()=>{
  assert.match(feedback,/authenticated_original_feedback_persists_after_restart_and_fails_on_tamper/);
  assert.match(feedback,/drop\(effects\);drop\(tasks\);drop\(chats\)/);
  assert.match(feedback,/UPDATE lu_independent_outcomes SET receipt_sha256=/);
  assert.match(feedback,/damaged\["hostSafeNextAction"\]/);
});
test('bounded explicit history is not a clearance of omitted effects',()=>{
  assert.match(feedback,/MAX_OPERATIONS_PER_TURN:usize=2/);
  assert.match(feedback,/omitted_history=view\.operations\.len\(\)>MAX_OPERATIONS_PER_TURN/);
  assert.match(feedback,/additionalOperationsNotRead/);
  assert.match(policy,/ReconcileOriginalHistoryBeforeNewAction/);
  assert.match(feedback,/MAX_FEEDBACK_BYTES:usize=3400/);
});
