import test from 'node:test';
import assert from 'node:assert/strict';
import {DEFAULT_PROFILE, normalizeProfile, profileField} from '../web/inference_profile.mjs';
test('default is valid and provides longer answer budget than R22',()=>{
 const p=normalizeProfile(DEFAULT_PROFILE);
 assert.equal(p.contextTokens,8192);assert.equal(p.maxNewTokens,2048);
});
test('per-model context cannot exceed allowed range',()=>{
 assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,contextTokens:524288}));
 assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,contextTokens:4001}));
});
test('model output tokens must fit within context',()=>{
 assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,contextTokens:8192,maxNewTokens:8192}));
});
test('sampling rejects nonfinite and invalid values',()=>{
 for(const v of [NaN,Infinity,-1,3])assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,temperature:v}));
});
test('field editing retains other controls',()=>{
 const result=profileField(DEFAULT_PROFILE,'contextTokens','16384');
 assert.equal(normalizeProfile(result).contextTokens,16384);
 assert.equal(result.topP,DEFAULT_PROFILE.topP);
});
