import test from 'node:test';
import assert from 'node:assert/strict';
import {DEFAULT_PROFILE, normalizeProfile, profileField} from '../web/inference_profile.mjs';
test('legacy installed GGUF behavior is preserved until explicit host save',()=>{
 const p=normalizeProfile(DEFAULT_PROFILE);
 assert.equal(p.contextTokens,4096);assert.equal(p.maxNewTokens,512);
 assert.equal(p.temperature,0.0);assert.equal(p.threads,4);
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

test('saved R23 larger settings remain bounded within current Lu native ceiling',()=>{
 const p=normalizeProfile({...DEFAULT_PROFILE,contextTokens:262144,
   maxNewTokens:4096,temperature:0.6,threads:8});
 assert.equal(p.contextTokens,262144);
 assert.equal(p.maxNewTokens,4096);
 for(const budget of [511,4097,8192])
   assert.throws(()=>normalizeProfile({...p,maxNewTokens:budget}));
});
test('profile cannot be set with malformed sampling or output limits',()=>{
 for(const value of [0,-0.1,NaN,Infinity])
   assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,topP:value}));
 assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,threads:0}));
});

