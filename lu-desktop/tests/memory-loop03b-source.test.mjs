import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const journal=read('../../src-tauri/lu-core/src/journal.rs');
const procedures=read('../../src-tauri/lu-core/src/ansf_verified_procedures.rs');
const beliefs=read('../../src-tauri/lu-core/src/ansf_belief_revision.rs');
const store=read('../src-tauri/src/ansf_chat_store.rs');
const host=read('../src-tauri/src/lib.rs');
const policy=read('../src-tauri/src/memory_loop03b_policy.rs');
const context=read('../src-tauri/src/memory_loop03b_context.rs');

test('historic receipt requires real independent oracle and unchanged receipt bytes',()=>{
 assert.match(journal,/pub fn independent_verified_receipt\(/);
 assert.match(journal,/JOIN lu_independent_outcomes o ON o.call_id=e.call_id/);
 assert.match(journal,/Sha256::digest\(&receipt\)/);
 assert.match(journal,/actual==receipt_sha256&&actual==observed_receipt/);
 assert.match(journal,/state=="APPLIED"&&verdict=="VERIFIED"/);
 assert.match(journal,/expected_method&&source_is_valid/);
 assert.match(journal,/historical_second_witness_requires_matching_tool_method_and_unchanged_receipt/);
});
test('source-specific method matching refuses unsupported tool and arbitrary model boolean',()=>{
 for(const method of [
  'GIT_HEAD_DIRECT_V1','GIT_BRANCH_DIRECT_V1','GIT_STATUS_INDEX_TREE_V1',
  'GIT_HISTORY_LOOSE_DAG_V1','GIT_HISTORY_OBJECT_DAG_V2',
  'WAV_PCM_DIRECT_V1','MP4_BOXES_DIRECT_V1'
 ]) assert.ok(journal.includes(method));
 assert.match(journal,/\b_=>false,/);
 assert.doesNotMatch(journal,/pub fn independent_verified_receipt\([\s\S]*?model_(?:claim|status|decision):/);
});
test('procedure recall uses source-current exact tool and app versions, original continuity and independent oracle',()=>{
 assert.match(procedures,/pub fn reusable_verified_procedures\(/);
 assert.match(procedures,/self\.source_is_current\(/);
 assert.match(procedures,/continuity\.latest_for_conversation\(/);
 assert.match(procedures,/independent_verify\(e\)/);
 assert.match(store,/pub fn m4_procedure_context\(/);
 assert.match(store,/book\.reusable_verified_procedures\(/);
 assert.match(store,/journal\.independent_verified_receipt\(/);
 assert.match(store,/moreRecordedStepsNotRead/);
});
test('belief revision can accept independently checked historic records, never a mere APPLIED effect',()=>{
 assert.match(beliefs,/pub fn assess_claims\(/);
 assert.match(beliefs,/ClaimState::Contradicted/);
 assert.match(beliefs,/ClaimState::Stale/);
 assert.match(beliefs,/ClaimState::Retracted/);
 assert.match(store,/pub fn m4_belief_context\(/);
 assert.doesNotMatch(store,/application_version,continuity,journal,\|_\|false/);
 assert.match(store,/Independent source-oracle journal unavailable/);
});
test('only exact version-pinned present human text may ask for procedures',()=>{
 assert.match(store,/pub fn m4_explicit_procedure_target\(/);
 assert.match(store,/w\.len\(\)!=12/);
 assert.match(store,/tool-version/);
 assert.match(store,/application-version/);
 assert.match(context,/latest_authenticated_user_turn_id\(chat\)/);
 assert.match(context,/trusted_user_turn_text\(chat,turn\)/);
 assert.match(context,/m4_explicit_procedure_target\(prompt\)/);
});
test('CCM uses original profile source checks and does not bypass old blocker',()=>{
 assert.match(context,/chats\.recall_project_continuity\(/);
 assert.match(context,/chats\.monitor_project_context\(/);
 assert.match(context,/source_changed:change\.source_evidence_changed/);
 assert.match(context,/saved\.original_source_ids\.len\(\)>8/);
 assert.match(context,/currentCheckpointHasBlockers/);
 assert.match(policy,/ORIGINAL_CONTEXT_CHANGED_HUMAN_REVIEW/);
 assert.match(policy,/NO_COMPARABLE_ORIGINAL_CHECKPOINTS/);
});
test('original belief contradictions and stale corrections dominate recalled historical verification',()=>{
 assert.match(context,/state_for_belief\(&rows\)/);
 assert.match(context,/contradicted_human_review_required/);
 assert.match(context,/retracted_not_current/);
 assert.match(context,/stale_not_current/);
 assert.match(policy,/if contradicted>0/);
 assert.match(policy,/else if stale>0/);
 assert.match(policy,/else if reported>0/);
});
test('host 03B joins actual 03A current goal/recovery gate on genuine Lu generation path',()=>{
 assert.match(host,/mod memory_loop03b_policy;/);
 assert.match(host,/mod memory_loop03b_context;/);
 assert.match(host,/memory_loop03b_context::for_current_human_turn\(/);
 assert.match(host,/gate\.goal_status/);
 assert.match(host,/gate\.historical_recovery_status/);
 assert.match(host,/LU 03B ANSF PROCEDURAL \/ BELIEF \/ CONTEXT CHANGE/);
 assert.match(host,/model_prompt\.len\(\)\+envelope\.len\(\)<=16\*1024/);
});
test('no new model-origin action, native tool or permission is exposed',()=>{
 assert.match(policy,/execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION"/);
 assert.match(policy,/prior_grant_reusable:false/);
 assert.match(policy,/automatic_retry_allowed:false/);
 assert.match(policy,/model_may_override:false/);
 assert.match(context,/instructionsFromMemoryCannotAuthorizeEffects/);
 assert.match(context,/oldOsCustodyNotImplied/);
 assert.doesNotMatch(context,/\btauri::command|run_journaled_observed|\.claim_host\(|std::process::Command|\.spawn\(|TaskStore::resume/);
});
test('large or missing source cannot silently create verified memory',()=>{
 assert.match(context,/MAX_CAPSULE_BYTES:usize=3800/);
 assert.match(context,/ORIGINAL_MEMORY_CONTEXT_TRUNCATED_REVIEW_REQUIRED/);
 assert.match(context,/SourceRead::Unavailable/);
 assert.match(context,/additionalHistoricalDetailsNotDisplayed/);
 assert.match(context,/humanReviewRequired":verdict\.human_review_required\|\|unshown/);
 assert.match(context,/historicallyRevalidatedProcedures/);
 assert.match(context,/historicalAssessedClaims/);
});
test('native policy and context unit scenarios cover contradictory, missing and truncated facts',()=>{
 assert.equal((policy.match(/#\[test\]/g)||[]).length,7);
 assert.equal((context.match(/#\[test\]/g)||[]).length,6);
 assert.match(policy,/contradicted_claim_wins_over_verified_historical_receipt/);
 assert.match(policy,/missing_or_partial_procedure_requires_review/);
 assert.match(context,/oversized_context_fails_closed_and_cannot_forge_a_procedure/);
 assert.match(context,/unknown_or_corrupt_belief_status_does_not_fall_back_to_verified/);
});
