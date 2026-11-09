import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const lib=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const handoff=readFileSync(new URL('../src-tauri/src/agent_memory_handoff.rs',import.meta.url),'utf8');
test('explicit current user syntax is narrow and cannot be triggered by old text',()=>{
  assert.match(handoff,/pub\(crate\) fn explicit_agent_handoff_request\(/);
  assert.match(handoff,/w\.len\(\)!=4/);
  assert.match(handoff,/digest\.len\(\)!=32/);
  assert.match(handoff,/trusted_user_turn_text\(chat,live_human_turn\)/);
});
test('native Send only includes handoff for human nonimage nonplan turns',()=>{
  assert.match(lib,/origin==GenerationOrigin::Human && images\.is_empty\(\) && !plan_only/);
  assert.match(lib,/agent_memory_handoff::explicit_agent_handoff_request\(&prompt\)/);
  assert.match(lib,/agent_memory_handoff::original_agent_handoff\(/);
  assert.match(lib,/&chat_id,&gid,&prompt,&plan/);
});
test('memory read only from exact authenticated plan and original turn',()=>{
  assert.match(handoff,/read_chat_progress\(/);
  assert.match(handoff,/find\(\|p\|p\.plan_id==plan_id\)/);
  assert.match(handoff,/trusted_user_turn_text\(chat,&progress\.original_human_turn_id\)/);
  assert.match(handoff,/open_existing_readonly\(&path\)/);
});
test('memory recovery includes independent OS custody and current ANSF source oracle',()=>{
  assert.match(handoff,/HostOriginalJobCustody::from_host_profile\(profile\)/);
  assert.match(handoff,/reconcile_recovery\(/);
  assert.match(handoff,/OriginalChatGoalSources\{chats\}/);
  assert.match(handoff,/independentRecoveryStatus/);
  assert.match(handoff,/independentlyObservedSteps/);
});
test('stored plans and old completions never become permission grants',()=>{
  assert.match(handoff,/NOT_AUTHORIZED_FOR_EXECUTION/);
  assert.match(handoff,/automaticRetryAllowed":false/);
  assert.match(handoff,/priorGrantReusable":false/);
  assert.match(handoff,/oldEffectMayBeReplayed":false/);
  assert.doesNotMatch(handoff,/\.spawn\(|TaskAuthority\{|ConversationAuthority::|claim_host\(|propose_step\(|stage_initial_draft\(/);
});
test('missing or cross-chat plan fails without source disclosure',()=>{
  assert.match(handoff,/No source-bound plan in the active conversation/);
  assert.match(handoff,/refuses_nonexistent_or_cross_chat_plan_without_creating_plan_ledger/);
  assert.match(lib,/UNAVAILABLE_OR_NOT_AUTHENTICATED/);
  assert.match(lib,/requestHumanReview":true/);
});
test('bounded memory context never emits raw historic instructions',()=>{
  assert.match(handoff,/MAX_MODEL_BYTES:usize=4096/);
  assert.match(handoff,/\.take\(8\)/);
  assert.match(lib,/envelope\.len\(\)<=4096/);
  assert.match(lib,/model_prompt\.len\(\)\+envelope\.len\(\)<=16\*1024/);
  assert.match(lib,/HISTORICAL DATA ONLY/);
});
