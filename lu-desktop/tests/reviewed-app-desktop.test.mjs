import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const desktop=readFileSync(new URL('../src-tauri/src/reviewed_app_desktop.rs',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const tool=readFileSync(new URL('../../src-tauri/lu-core/src/native_reviewed_app_tool.rs',import.meta.url),'utf8');

test('single Send enters native host routing; reviewed application still needs OS picker',()=>{
  assert.match(ui,/reviewedAppNextSend/);
  // P0-WIN-CHAT intentionally unmounted the legacy specialist buttons.
  // The ordinary Send routes through lu_generate and the native host
  // classifies the original human request. No JS text can be a tool grant.
  assert.match(ui,/Native tools enter through the original human chat and per-turn OS approvals/);
  assert.match(ui,/invoke\('lu_generate',\s*\{/);
  assert.match(ui,/invoke\('lu_reviewed_app_ask',\s*\{/);
  assert.match(desktop,/blocking_pick_file\(\)/);
  assert.match(desktop,/blocking_pick_folder\(\)/);
  assert.match(desktop,/matches!\(mode.as_str\(\),"git"\|"media"\|"pdf"\|"image"\)/);
  assert.match(desktop,/PersistedReviewedHumanTurn/);
  assert.match(desktop,/trusted_user_turn_text\(chat,turn\)/);
  assert.match(host,/lu_reviewed_app_ask,/);
  assert.doesNotMatch(ui,/invoke\('lu_reviewed_app_ask',\s*\{[^}]*\b(?:path|executable|argv|permission)\s*:/s);
});

test('host-provided exact operation binding never infers model permission from discovery',()=>{
  assert.match(tool,/pub fn register_reviewed_application_tools\(/);
  assert.match(tool,/pub struct ReviewedApplicationTurnSession/);
  assert.match(tool,/ModelToolCatalog/);
  assert.match(tool,/NativeToolRegistry/);
  assert.match(tool,/NativeToolTurn::new\(/);
  assert.match(tool,/expect_fields\(args,&\[\],&\[\]\)/);
  assert.match(tool,/allowed.contains\(&record.read_tool_id\)/);
  assert.match(tool,/allowed.contains\(&record.execute_tool_id\)/);
  assert.match(tool,/run_supervised_native_turn_journaled_observed_with_host_routing/);
  assert.match(desktop,/open_user_task\(TaskAuthority/);
  assert.match(desktop,/ReviewedApplicationTurnSession::from_verified_host_task/);
  assert.doesNotMatch(tool,/Command::new\(/);
});

test('reviewed child process uses separate durable receipt and cancellation fencing',()=>{
  assert.match(tool,/format!\("\{call_id\}-\{\}",operation.suffix\(\)\)/);
  assert.match(tool,/journal.get\(&worker_id\)/);
  assert.match(tool,/EffectState::Applied/);
  assert.match(tool,/ReviewedCancelWatch::start/);
  assert.match(tool,/authority.revoke_task\(&task_id\)/);
  assert.match(desktop,/reader_generations\.lock\(\)/);
  assert.match(desktop,/run_journaled_observed\(/);
  assert.match(desktop,/answer.tool_calls==0/);
  assert.match(desktop,/host.chats.complete_reviewed_if_not_revoked\(/);
});

test('desktop only permits reviewed Git reads or FFprobe WAV/MP4 facts, never arbitrary shell',()=>{
  assert.match(desktop,/GitReadOperation::HeadCommit/);
  assert.match(desktop,/GitReadOperation::HistoryCount/);
  assert.match(desktop,/ReviewedContainer::Wave/);
  assert.match(desktop,/ReviewedContainer::Mp4/);
  assert.match(desktop,/if extension.eq_ignore_ascii_case\("wav"\)/);
  assert.match(desktop,/file must be WAV or MP4/i);
  assert.doesNotMatch(desktop,/fn execute_arbitrary_command\(/);
});

test('read-only reviewed-app tool activity shows durable effects, never model claims as OS proof',()=>{
  const audit=readFileSync(new URL('../src-tauri/src/reviewed_app_audit.rs',import.meta.url),'utf8');
  assert.match(host,/\("Reviewed app","lu-reviewed-app-model-audit.sqlite"\)/);
  assert.match(host,/trusted_user_turn_text\(/);
  assert.match(host,/read_reviewed_effect_counts\(/);
  assert.match(host,/classify_reviewed_evidence\(/);
  assert.match(ui,/Reviewed app effect journal:/);
  assert.match(ui,/item.reviewedEffects.appliedWithReceipt/);
  assert.match(ui,/item.reviewedEffects.uncertain/);
  assert.match(ui,/Model answers are not OS receipts/);
  assert.match(audit,/SQLITE_OPEN_READ_ONLY/);
  assert.match(audit,/lu-reviewed-effects-\{host_turn_id\}\.sqlite/);
  assert.match(audit,/Invalid reviewed evidence turn identity/);
  assert.match(audit,/APPLIED_RECEIPT_MISSING/);
  assert.match(audit,/EFFECT_UNCERTAIN_OR_IN_FLIGHT/);
  assert.doesNotMatch(audit,/pub fn (?:execute|launch|grant|replay)_/);
  assert.doesNotMatch(ui,/invoke\('lu_reviewed_app_ask',\s*\{[^}]*\b(?:path|executable|argv|permission)\s*:/s);
});

test('E1-P10 shows independent at-observation proof, not worker or model authority',()=>{
  const oracle=readFileSync(new URL('../../src-tauri/lu-core/src/reviewed_outcome_oracle.rs',import.meta.url),'utf8');
  const worker=readFileSync(new URL('../../src-tauri/lu-core/src/native_reviewed_app_tool.rs',import.meta.url),'utf8');
  const journal=readFileSync(new URL('../../src-tauri/lu-core/src/journal.rs',import.meta.url),'utf8');
  const audit=readFileSync(new URL('../src-tauri/src/reviewed_app_audit.rs',import.meta.url),'utf8');
  assert.match(worker,/verify_git\(repository_root,git_op,&value\)/);
  assert.match(worker,/verify_media\(/);
  assert.match(worker,/journal\.record_independent_outcome\(/);
  assert.match(worker,/IndependentOutcomeVerdict::Conflict/);
  assert.match(worker,/independentOutcome/);
  assert.match(journal,/CREATE TABLE IF NOT EXISTS lu_independent_outcomes/);
  assert.match(journal,/state!="APPLIED"/);
  assert.match(oracle,/WAV_PCM_DIRECT_V1/);
  assert.match(oracle,/GIT_HEAD_DIRECT_V1/);
  assert.match(oracle,/GIT_STATUS_INDEX_TREE_V1/);
  assert.match(oracle,/tracked_worktree_snapshot\(/);
  assert.match(oracle,/MP4_BOXES_DIRECT_V1/);
  assert.match(oracle,/parse_mp4\(&bytes\)/);
  assert.doesNotMatch(oracle,/Command::new|\.spawn\(/);
  assert.match(audit,/INDEPENDENTLY_VERIFIED_AT_OBSERVATION/);
  assert.match(audit,/OUTCOME_CONFLICT/);
  assert.match(ui,/item.reviewedEffects.independentlyVerified/);
  assert.match(ui,/item.reviewedEffects.outcomeConflict/);
  assert.match(ui,/Current source independently reverified: no/);
  assert.doesNotMatch(ui,/invoke\('lu_reviewed_app_ask',\s*\{[^}]*\b(?:path|executable|argv|permission)\s*:/s);
});

test('E1-P14 explains host evidence to humans without inventing permission or replay',()=>{
  assert.match(ui,/import \{explainToolOutcome,explainReviewedContinuation\} from '\.\/reviewed_tool_feedback\.mjs'/);
  assert.match(ui,/explainToolOutcome\(item\.source,item\.state\)\.title/);
  assert.match(ui,/explainToolOutcome\(item\.source,item\.state\)\.meaning/);
  assert.match(ui,/Next safe step:/);
  assert.doesNotMatch(ui,/<strong>\{item\.source\} — \{item\.state\.replaceAll\('_',' '\)\}<\/strong>/);
  assert.match(ui,/Viewing never restores/);
  assert.match(ui,/Model answers are not OS receipts/);
  assert.doesNotMatch(ui,/invoke\('lu_reviewed_app_ask',\s*\{[^}]*\b(?:path|executable|argv|permission)\s*:/s);
});
