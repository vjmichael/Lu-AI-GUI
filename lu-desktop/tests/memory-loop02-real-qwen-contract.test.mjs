import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=(path)=>readFileSync(new URL(path,import.meta.url),'utf8');
const source=read('../../src-tauri/lu-core/src/reviewed_outcome_oracle.rs');
const native=read('../src-tauri/src/agent_verified_feedback.rs');
const verifier=read('../../benchmarks/e1/memory_loop02_genuine_qwen_feedback.py');
const grading=read('../../benchmarks/e1/test_memory_loop02_qwen_feedback.py');
const workflow=read('../../.github/workflows/lu-memory-loop02-real-qwen.yml');
test('native model fixtures invoke the actual separate on-disk Git HEAD oracle',()=>{
  assert.match(native,/memory_loop02_real_git_oracle_feedback_qwen_fixture_export/);
  assert.match(native,/Command::new\("git"\)/);
  assert.match(native,/git\(&\["rev-parse","HEAD"\]\)/);
  assert.match(native,/verify_git\(&repository,GitReadOperation::HeadCommit,&worker\)/);
  assert.match(source,/pub fn verify_git\(/);
});
test('native original SQLite closes/reopens and records conflict/uncertainty',()=>{
  assert.match(native,/EffectJournal::open\(&effectfile\)/);
  assert.match(native,/bind_reviewed_human_turn\(&chat,&original\)/);
  assert.match(native,/effects\.mark_uncertain\("review-gh"\)/);
  assert.match(native,/IndependentOutcomeVerdict::Conflict/);
  assert.match(native,/drop\(chats\)/);
  assert.match(native,/let reopened=ChatStore::open\(&root\)/);
  assert.match(native,/for_next_human_turn\(/);
});
test('model bytes, engine source and real health are mandatory',()=>{
  assert.match(verifier,/7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d/);
  assert.match(verifier,/2bc46baa9207d098d03741ac394e7faaf1a955b6/);
  assert.match(verifier,/if fh\.read\(4\) != b"GGUF"/);
  assert.match(verifier,/model_sha != MODEL_SHA256/);
  assert.match(verifier,/subprocess\.Popen\(/);
  assert.match(verifier,/health\.get\("status"\) == "ok"/);
  assert.match(workflow,/sha256sum -c -/);
});
test('three native semantic outcomes become nine actual Qwen completions',()=>{
  assert.match(verifier,/"source_verified"/);
  assert.match(verifier,/"independent_conflict"/);
  assert.match(verifier,/"interrupted_effect"/);
  assert.match(workflow,/--repetitions 3/);
  assert.match(workflow,/observed_actual_model_generations"\]==9/);
});
test('source verification without OS custody first requires kernel recovery reconciliation',()=>{
  assert.match(verifier,/RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION/);
  assert.match(verifier,/MANUAL_RECONCILIATION_BEFORE_NEW_ACTION/);
  assert.match(verifier,/does_not_infer_os_process_tree_termination/);
  assert.match(grading,/test_verified_source_without_os_custody_requires_fresh_authorization/);
});
test('previous permission, auto-retry and current freshness claims fail strict grade',()=>{
  assert.match(verifier,/refuses_previous_tool_grant/);
  assert.match(verifier,/refuses_unreviewed_automatic_retry/);
  assert.match(verifier,/does_not_claim_fresh_source/);
  assert.match(verifier,/authority_violations/);
  assert.match(verifier,/overall_pass/);
});
test('never claim actual model pass without live completions',()=>{
  assert.match(verifier,/observed_actual_model_generations/);
  assert.match(verifier,/model_started_and_healthy/);
  assert.match(verifier,/overall_pass"\] = False/);
  assert.match(workflow,/NOT the installed Lu WebView/);
});
