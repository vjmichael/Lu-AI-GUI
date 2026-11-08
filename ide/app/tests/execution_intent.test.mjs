import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

test('D1 native intent operations exist but no spawn, shell or execution grant', () => {
  const native = read('../../host/src/execution_intents.rs');
  const tauri = read('../src-tauri/src/lib.rs');
  assert.match(native, /pub fn prepare\(/);
  assert.match(native, /pub fn decide\(/);
  assert.match(native, /pub fn list\(/);
  assert.match(native, /INTENT_TTL_SECS: u64 = 120/);
  assert.match(native, /execution: "never_started"/);
  assert.match(native, /grant_token_issued: false/);
  assert.match(native, /safe_manifest\(/);
  assert.match(native, /check_source\(/);
  assert.match(native, /allowed_transition\(/);
  assert.doesNotMatch(native, /std::process|Command::new|\.spawn\(|\.exec\(|shell\s*=\s*true/);
  assert.match(tauri, /async fn ide_prepare_execution_intent/);
  assert.match(tauri, /async fn ide_decide_execution_intent/);
  assert.match(tauri, /async fn ide_list_execution_intents/);
  assert.match(tauri, /execution-intents\.jsonl/);
});
test('D1 typed schemas offer only allowlisted recipes and decisions', () => {
  const schemas = JSON.parse(read('../../host/src/capability_schemas.json'));
  assert.equal(Object.keys(schemas.operations).length,46);
  // B3 adds one read-only source-search operation and B2 adds four strictly
  // scoped private draft operations; original D1 intent policy is unchanged.
  for (const id of ['source.findProject','draft.checkpoint','draft.latest','draft.load','draft.discard']) {
    assert.ok(schemas.operations[id], id);
  }
  assert.equal(Object.keys(schemas.definitions).length,42);
  const prepare = schemas.operations['job.prepare'].request;
  assert.deepEqual(prepare.required,['projectId','recipeId']);
  assert.deepEqual(prepare.properties.recipeId.enum,[
    'rust_check','rust_tests','lu_ide_host_tests',
  ]);
  const review = schemas.operations['job.review'].request;
  assert.deepEqual(review.required,['projectId','intentId','decision']);
  assert.deepEqual(review.properties.decision.enum,['record_review','cancel']);
  assert.equal(schemas.definitions.IntentSummary.properties.grantTokenIssued.const,false);
  assert.equal(schemas.definitions.IntentList.properties.workerExecutionEnabled.const,false);
});
test('D1 UI requires human review but contains no Run action', () => {
  const ui = read('../web/main.jsx');
  assert.match(ui, /aria-label="Execution intent review"/);
  assert.match(ui, /Prepare for human review/);
  assert.match(ui, /Record human review \(no execution\)/);
  assert.match(ui, /invoke\('ide_prepare_execution_intent'/);
  assert.match(ui, /invoke\('ide_decide_execution_intent'/);
  assert.match(ui, /invoke\('ide_list_execution_intents'/);
  assert.match(ui, /reviewIntentId === entry\.intentId/);
  assert.doesNotMatch(ui, /invoke\('ide_run_job'/);
});
