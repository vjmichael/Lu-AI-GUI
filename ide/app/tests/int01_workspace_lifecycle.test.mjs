// Deferred regression/source contract for IDE-INT-01B; do NOT run during
// the implementation-only phase. Execute with the eventual batched matrix.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const source=readFileSync('web/main.jsx','utf8');

test('INT01B closing projects takes a live-buffer witness rather than React dirty',()=>{
  assert.match(source,/function projectEditorWitness\(projectId\)/);
  assert.match(source,/captureLiveEditorIntent\(tab,/);
  assert.match(source,/function projectEditorWitnessChanged\(projectId, before\)/);
  assert.match(source,/stillCurrentEditorIntent\(entry.intent,/);
  assert.match(source,/witness\.some\(entry=>entry\.intent\.status!=='current' \|\| entry\.intent\.dirty\)/);
});

test('INT01B native close completion retains drafts if editor changed while awaiting',()=>{
  assert.match(source,/if \(projectEditorWitnessChanged\(args\.projectId,witness\)\)/);
  assert.match(source,/Live drafts were kept in memory/);
  assert.match(source,/discardProjectTabs\(args\.projectId\);/);
  assert.match(source,/setActiveProject\(null\)/);
});

test('INT01B search navigation does not apply saved source offsets to a new live draft',()=>{
  assert.match(source,/const existingLive = existing \? captureLiveEditorIntent\(existing,/);
  assert.match(source,/existingLive\.status !== 'current' \|\| existingLive\.dirty/);
  assert.match(source,/Search refers to saved disk bytes/);
  assert.match(source,/existing\.revision !== opened\.revision/);
});

test('INT01A synchronous Save reservation and native revision check persist',()=>{
  assert.match(source,/savingKeyRef\.current=tab\.key/);
  assert.match(source,/expectedRevision: started\.savedRevision/);
  assert.match(source,/text: started\.text/);
  assert.match(source,/rebaseBufferAfterSave\(currentBuffer,saveStartedBuffer,saved\.revision\)/);
  assert.match(source,/reconcileSavedEditorTab\(item,started,saved\.revision,/);
});

test('INT01A asynchronous Review/Reload reject changed buffer identity',()=>{
  assert.match(source,/proposal\.expectedRevision!==intent\.savedRevision/);
  assert.match(source,/stillCurrentEditorIntent\(review\.liveIntent,tab,/);
  assert.match(source,/Reload stopped: editor changed after confirmation/);
  assert.match(source,/Discard your live unsaved edits and reload from disk/);
});
