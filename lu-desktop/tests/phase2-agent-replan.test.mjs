import test from 'node:test';
import assert from 'node:assert/strict';
import {parseHumanReplanOutline,humanReplanDefaultOutline} from '../web/agent_replan.mjs';

test('phase2 human replan is typed, bounded and dependency ordered',()=>{
  const prior=[
    {purpose:'INDEPENDENT_CHECK'},
    {purpose:'PRESENT'}
  ];
  const steps=parseHumanReplanOutline(
    'COLLECT_SOURCE, INDEPENDENT_CHECK, PRESENT',prior);
  assert.deepEqual(steps,[
    {kind:'COLLECT_SOURCE',dependsOn:null,expected:'SOURCE_REFERENCES'},
    {kind:'INDEPENDENT_CHECK',dependsOn:0,expected:'INDEPENDENT_WITNESS'},
    {kind:'PRESENT',dependsOn:1,expected:'EXPLANATION'}
  ]);
  assert.equal(humanReplanDefaultOutline(prior),'INDEPENDENT_CHECK, PRESENT');
});
test('rejects stale, unsupported or effectful replacement requests',()=>{
  const prior=[{purpose:'INDEPENDENT_CHECK'},{purpose:'PRESENT'}];
  for(const input of [
    '', 'INDEPENDENT_CHECK, PRESENT',
    'EXECUTE_SHELL','sudo git push','TOOLS_ALLOWED',
    'INDEPENDENT_CHECK,'.repeat(9),
    '\u0000', 'A'.repeat(513)
  ]) assert.throws(()=>parseHumanReplanOutline(input,prior),input);
});
