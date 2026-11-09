import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const host=read('../src-tauri/src/agent_recovery_provenance.rs');
const lib=read('../src-tauri/src/lib.rs');
const original=read('../src-tauri/src/reviewed_task_continuity.rs');
const policy=read('../src-tauri/src/host_safe_continuation.rs');
const web=read('../web/main.jsx');

test('native Tauri API accepts only authenticated chat, never model-specified original turn or journal path',()=>{
  assert.match(host,/fn lu_agent_recovery_provenance\([\s\S]*?chat_id:String/);
  assert.match(host,/read_original_recovery\(profile,&chat_id,&host\.chats\)/);
  assert.match(host,/host\.catalog_path\.parent\(\)/);
  assert.match(host,/chats\.history_window\(chat,None,"older",1\)/);
  assert.match(host,/chats\.latest_authenticated_user_turn_id\(chat\)/);
  assert.match(host,/chats\.prior_authenticated_turn_ids\(/);
  assert.match(lib,/mod agent_recovery_provenance;/);
  assert.match(lib,/agent_recovery_provenance::lu_agent_recovery_provenance,/);
});

test('projector uses original SQLite and independently witnessed OS descendant custody',()=>{
  assert.match(host,/reviewed_turn_continuity\(\s*profile,chat,&original_turn/);
  assert.match(host,/HostOriginalJobCustody::from_host_profile\(profile\)/);
  assert.match(host,/custody\.worker_and_descendants_ended\(/);
  assert.match(host,/HostSafeNext::for_evidence\(HostReviewedEvidence\{/);
  assert.match(host,/turn_next=turn_next\.restrict\(decision\)/);
  assert.match(host,/worst=worst\.restrict\(turn_next\)/);
  assert.match(original,/SQLITE_OPEN_READ_ONLY/);
  assert.match(original,/Sha256::digest\(receipt\)/);
});

test('historical semantic source and OS custody are two distinct, non-live facts',()=>{
  for(const token of [
    'independent_oracle_at_observation',
    'independently_observed_sha256',
    'original_model_call_linked',
    'completed_worker_task_linked',
    'descendant_custody',
    'current_source_freshness',
    'KERNEL_EMPTY_AT_OBSERVATION',
    'NOT_INDEPENDENTLY_PROVEN',
    'NOT_RECHECKED'
  ])assert.ok(host.includes(token),token);
  assert.match(host,/full_journal&&op\.outcome=="VERIFIED_AT_OBSERVATION"/);
  assert.match(policy,/ReconcileOsCustodyThenFreshSourceAndHumanAuthorization/);
});

test('closed host decisions remain order-independent and cannot be replaced by model or WebView policy',()=>{
  assert.match(host,/worst=worst\.restrict\(/);
  assert.match(policy,/std::cmp::max\(self,other\)/);
  for(const token of [
    'execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION"',
    'prior_grant_reusable:false',
    'automatic_retry_allowed:false',
    'historical_effect_may_be_replayed:false',
    'model_may_override_host_safe_decision:false',
    'may_execute_or_replay:false'
  ]) assert.ok(host.includes(token),token);
  assert.doesNotMatch(host,/std::process::Command|\.spawn\(|TaskStore::resume|\.schedule\(|\.claim\(|\.authorize\(/);
});

test('missing original or omitted operations never become a positive clearance',()=>{
  assert.match(host,/reviewed\.is_empty\(\)\|\|window_incomplete/);
  assert.match(host,/view\.operations\.len\(\)>MAX_DISPLAYED_OPERATIONS_PER_TURN/);
  assert.match(host,/for_turn_without_complete_effects\(/);
  assert.match(host,/older_host_turns_not_inspected/);
  assert.match(host,/additional_operations_not_read:omitted/);
  assert.match(host,/ReconcileOriginalHistoryBeforeNewAction/);
});

test('native tests reopen actual task and effect SQLite after host restart and detect independently tampered receipt',()=>{
  assert.match(host,/original_sqlite_provenance_survives_restart_and_digest_tamper_stops/);
  assert.match(host,/drop\(chats\)/);
  assert.match(host,/ChatStore::open\(&root\)/);
  assert.match(host,/UPDATE lu_independent_outcomes SET receipt_sha256=/);
  assert.match(host,/RECEIPT_MISMATCH/);
});

test('cross-chat originals and missing paired journals cannot become successful outcomes',()=>{
  assert.match(host,/missing_original_effects_and_cross_chat_sources_never_claim_success/);
  assert.match(host,/NO_REVIEWED_EFFECTS_IN_HOST_WINDOW/);
  assert.match(host,/PARTIAL_JOURNALS|INCOMPLETE_ORIGINAL_HISTORY/);
  assert.match(host,/capped_original_human_history_is_marked_unread_not_globally_cleared/);
});

test('chat tool activity provides explicit human-triggered read-only source view',()=>{
  assert.match(web,/Original-source recovery provenance/);
  assert.match(web,/invoke\('lu_agent_recovery_provenance',\{chatId:targetChat\}\)/);
  assert.match(web,/inspectHistoricalRecoveryProvenance\(chatId\)/);
  assert.match(web,/Original human turn: \{turn\.originalHumanTurnId\}/);
  assert.match(web,/operation\.independentOracleAtObservation/);
  assert.match(web,/operation\.descendantCustody/);
  assert.match(web,/operation\.independentlyObservedSha256/);
});

test('UI clears potentially stale success before reading and refuses a malformed or granting response',()=>{
  const begin=web.indexOf('async function inspectHistoricalRecoveryProvenance(');
  const end=web.indexOf('async function prepareFreshAgentContinuation(',begin);
  assert.ok(begin>0&&end>begin);
  const body=web.slice(begin,end);
  assert.ok(body.indexOf('setRecoveryProvenanceByChat(old=>({...old,[targetChat]:null}))')<
    body.indexOf("invoke('lu_agent_recovery_provenance'"));
  for(const denied of [
    "view.executionAuthority!=='NOT_AUTHORIZED_FOR_EXECUTION'",
    "view.currentSourceFreshness!=='NOT_RECHECKED'",
    'view.priorGrantReusable!==false',
    'view.automaticRetryAllowed!==false',
    'view.historicalEffectMayBeReplayed!==false',
    'view.modelMayOverrideHostSafeDecision!==false'
  ]) assert.ok(body.includes(denied),denied);
  assert.match(body,/catch\(error\)/);
});

test('screen states limits and says historical evidence is not live or an authorization',()=>{
  for(const phrase of [
    'Original source bound:',
    'Older authenticated',
    'NOT RECHECKED',
    'Historical grants cannot be reused',
    'No native operation or model continuation was authorized'
  ]) assert.ok(web.includes(phrase),phrase);
  assert.match(web,/Additional\s+original effect operations were not read/);
});
