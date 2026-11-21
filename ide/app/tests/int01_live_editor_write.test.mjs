// Deferred regression suite. NOT executed during the implementation-only phase.
import test from 'node:test';
import assert from 'node:assert/strict';
import { captureLiveEditorIntent, stillCurrentEditorIntent,
  reconcileSavedEditorTab } from '../web/editor_live_write.js';
import { newBufferIdentity, advanceBufferIdentity,
  rebaseBufferAfterSave } from '../web/buffer_identity.js';

const originalSha='a'.repeat(64), changedSha='b'.repeat(64);
const projectId='ide-'+'c'.repeat(32), relativePath='src/main.rs';
const key=projectId+':'+relativePath;
const bufferId='12345678-aaaa-bbbb-cccc-123456789abc';
const state=text=>({sliceDoc:()=>text});
const tab=(draft='disk',savedText='disk')=>({
  key,projectId,relativePath,revision:originalSha,
  savedText,draft,dirty:draft!==savedText,generation:0,bufferRevision:0,
});
const identity=()=>newBufferIdentity(projectId,relativePath,originalSha,bufferId);

test('INT01A Save captures latest CodeMirror text rather than stale React draft',()=>{
  const oldRender=tab();
  const intent=captureLiveEditorIntent(oldRender,advanceBufferIdentity(identity()),
    state('disk and new typing'),projectId);
  assert.equal(intent.status,'current');
  assert.equal(intent.dirty,true);
  assert.equal(intent.text,'disk and new typing');
  assert.equal(oldRender.dirty,false);
  assert.equal(oldRender.draft,'disk');
});

test('INT01A no-op Save ignores render dirty flag after undo restores disk bytes',()=>{
  const render=tab('stale unsaved text');
  const intent=captureLiveEditorIntent(render,advanceBufferIdentity(identity()),
    state('disk'),projectId);
  assert.equal(intent.status,'current');
  assert.equal(intent.dirty,false);
  assert.equal(render.dirty,true);
});

test('INT01A only exact registered project/path/base/buffer can be saved',()=>{
  const t=tab();
  const id=identity();
  assert.equal(captureLiveEditorIntent(t,id,state('new'),projectId).status,'current');
  assert.equal(captureLiveEditorIntent(t,id,state('new'),'wrong-project').status,
    'stale_editor_identity');
  assert.equal(captureLiveEditorIntent({...t,relativePath:'wrong.rs'},id,
    state('new'),projectId).status,'stale_editor_identity');
  assert.equal(captureLiveEditorIntent({...t,revision:changedSha},id,
    state('new'),projectId).status,'stale_editor_identity');
  assert.equal(captureLiveEditorIntent({...t,key:'forged'},id,state('new'),
    projectId).status,'stale_editor_identity');
});

test('INT01A Review cannot persist after typing or replacing editor buffer',()=>{
  const t=tab(),id=identity();
  const reviewed=captureLiveEditorIntent(t,id,state('reviewed'),projectId);
  assert.equal(stillCurrentEditorIntent(reviewed,t,id,state('reviewed'),projectId),true);
  assert.equal(stillCurrentEditorIntent(reviewed,t,
    advanceBufferIdentity(id),state('reviewed'),projectId),false);
  assert.equal(stillCurrentEditorIntent(reviewed,t,id,state('newer'),projectId),false);
  assert.equal(stillCurrentEditorIntent(reviewed,t,newBufferIdentity(projectId,
    relativePath,originalSha,'abcdefab-0000-0000-0000-123456789abc'),
    state('reviewed'),projectId),false);
});

test('INT01A Save completion keeps typing performed during native write dirty',()=>{
  const t=tab(),start=identity(),capture=captureLiveEditorIntent(t,
    start,state('first saved text'),projectId);
  const continued=advanceBufferIdentity(start);
  const rebased=rebaseBufferAfterSave(continued,start,changedSha);
  assert.equal(rebased.status,'rebased');
  const next=reconcileSavedEditorTab(t,capture,changedSha,rebased.identity,
    state('first saved text + later edit'));
  assert.equal(next.revision,changedSha);
  assert.equal(next.savedText,'first saved text');
  assert.equal(next.draft,'first saved text + later edit');
  assert.equal(next.dirty,true);
  assert.equal(next.bufferRevision,1);
});

test('INT01A successful save rejects rebasing closed/reopened/moved tab',()=>{
  const t=tab(),start=identity(),capture=captureLiveEditorIntent(t,
    start,state('saved'),projectId);
  const rebased=rebaseBufferAfterSave(start,start,changedSha);
  assert.equal(reconcileSavedEditorTab(t,capture,changedSha,rebased.identity,
    state('saved')).dirty,false);
  const reopened=newBufferIdentity(projectId,relativePath,originalSha,
    'abcdefab-0000-0000-0000-123456789abc');
  assert.equal(rebaseBufferAfterSave(reopened,start,changedSha).status,'replaced_buffer');
  assert.equal(reconcileSavedEditorTab({...t,revision:changedSha},
    capture,changedSha,rebased.identity,state('saved')),null);
  assert.equal(reconcileSavedEditorTab(t,capture,changedSha,reopened,state('saved')),null);
});

test('INT01A Close/Reload decisions fail closed if current buffer is unknown',()=>{
  const current=captureLiveEditorIntent(tab(),identity(),state('unsaved'),projectId);
  assert.equal(current.dirty,true);
  assert.equal(captureLiveEditorIntent(tab(),null,state('unsaved'),
    projectId).status,'stale_editor_identity');
  assert.equal(stillCurrentEditorIntent(current,tab(),identity(),
    state('unsaved again'),projectId),false);
});
