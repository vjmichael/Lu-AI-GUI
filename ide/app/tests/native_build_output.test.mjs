import assert from 'node:assert/strict';
import test from 'node:test';
import {acceptNativeOutput,MAX_OUTPUT_LINES} from '../web/native_build_output.js';
const projectId='ide-'+'a'.repeat(32);
const page=(chunks,lastSequence)=>({chunks,lastSequence,truncated:false,liveUnverified:true});
test('readback preserves separate Linux streams and deduplicates cursor',()=>{
  const a=acceptNativeOutput(null,projectId,'job-1',page([
    {sequence:1,stream:'stdout',text:'build start'},
    {sequence:2,stream:'stderr',text:'warning'}],2));
  assert.equal(a.chunks.length,2);
  assert.equal(a.lastSequence,2);
  assert.equal(acceptNativeOutput(a,projectId,'job-1',page([],2)).chunks.length,2);
});
test('no cross-project, forged stream or cursor override',()=>{
  const a=acceptNativeOutput(null,projectId,'job-1',page([],0));
  assert.equal(acceptNativeOutput(a,'ide-'+'b'.repeat(32),'job-2',page([],0)).jobId,'job-2');
  assert.equal(acceptNativeOutput(a,projectId,'job-1',page([
    {sequence:1,stream:'credential',text:'secret'}],1)),null);
  assert.equal(acceptNativeOutput(a,projectId,'job-1',page([
    {sequence:9,stream:'stdout',text:'forged'}],3)),null);
});
test('bounded and truncated with untrusted large text',()=>{
  let state=null;
  for(let i=0;i<350;i++)state=acceptNativeOutput(state,projectId,'job',page([
    {sequence:i+1,stream:'combined',text:'x'.repeat(500)}],i+1));
  assert.ok(state.chunks.length <= MAX_OUTPUT_LINES);
  assert.ok(state.chunks.length > 0);
  assert.ok(state.chunks.reduce((n,item)=>n+item.text.length,0)<=65536);
  assert.ok(state.truncated);
});
test('requires native provenance marker and known job identity',()=>{
  assert.equal(acceptNativeOutput(null,projectId,'job',{}),null);
  assert.equal(acceptNativeOutput(null,projectId,'../../etc/passwd',page([],0)),null);
});
