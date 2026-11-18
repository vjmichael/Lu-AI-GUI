import test from 'node:test';
import assert from 'node:assert/strict';
import {explainToolOutcome} from '../web/reviewed_tool_feedback.mjs';

test('all reviewed-app statuses have an explanation and non-executing next step',()=>{
  for(const status of [
    'EFFECT_UNCERTAIN_OR_IN_FLIGHT','MODEL_FAILED','EFFECT_FAILED',
    'OUTCOME_CONFLICT','REJECTED','APPLIED_RECEIPT_MISSING',
    'INDEPENDENTLY_VERIFIED_AT_OBSERVATION','ANSWER_RECORDED_WITH_APPLIED_JOURNAL',
    'MODEL_ANSWER_ONLY','INCOMPLETE_OR_INTERRUPTED']){
    const guidance=explainToolOutcome('Reviewed app',status);
    assert.ok(guidance.title.length>5,status);
    assert.ok(guidance.meaning.length>30,status);
    assert.ok(guidance.nextStep.length>20,status);
    assert.doesNotMatch(guidance.nextStep,/(^|[.!]\s*)Retry automatically|(^|[.!]\s*)Grant permission now/i);
  }
});

test('uncertain effects and missing receipts never become success labels',()=>{
  for(const status of ['EFFECT_UNCERTAIN_OR_IN_FLIGHT','APPLIED_RECEIPT_MISSING','MODEL_ANSWER_ONLY','ANSWER_RECORDED_WITH_APPLIED_JOURNAL']){
    const guidance=explainToolOutcome('Reviewed app',status);
    assert.doesNotMatch(guidance.title,/^(verified|success|completed)/i);
  }
  assert.match(explainToolOutcome('Reviewed app','EFFECT_UNCERTAIN_OR_IN_FLIGHT').nextStep,/Do not retry automatically/i);
});

test('independent source check refers only to the observation, never ongoing custody',()=>{
  const guidance=explainToolOutcome('Reviewed app','INDEPENDENTLY_VERIFIED_AT_OBSERVATION');
  assert.match(guidance.title,/at observation/i);
  assert.match(guidance.meaning,/does not certify current file state or ongoing process termination/i);
});

test('ordinary model completion does not become independently verified OS work',()=>{
  const guidance=explainToolOutcome('Git','COMPLETED');
  assert.match(guidance.title,/Model turn completed/);
  assert.match(guidance.meaning,/does not certify an operating-system effect/i);
});

test('unknown keys and prototype property names fail closed',()=>{
  const bad='INDEPENDENTLY_VERIFIED_AT_OBSERVATION_v2_untrusted';
  const feedback=explainToolOutcome('Reviewed app',bad);
  assert.equal(feedback.title,'Execution status unavailable');
  assert.doesNotMatch(JSON.stringify(feedback),/v2_untrusted/);
  assert.equal(explainToolOutcome('Reader','INDEPENDENTLY_VERIFIED_AT_OBSERVATION').title,'Execution status unavailable');
  assert.equal(explainToolOutcome('Reviewed app',null).title,'Execution status unavailable');
  for(const key of ['__proto__','constructor','toString']){
    assert.equal(explainToolOutcome('Reviewed app',key).title,'Execution status unavailable');
    assert.equal(explainToolOutcome('Git',key).title,'Execution status unavailable');
  }
});
