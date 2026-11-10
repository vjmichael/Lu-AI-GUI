import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const rust=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const clock=readFileSync(new URL('../src-tauri/src/host_clock.rs',import.meta.url),'utf8');
const css=readFileSync(new URL('../web/styles.css',import.meta.url),'utf8');

test('physical alpha preserves model load, queue, eject and conversation',()=>{
  assert.match(ui,/invoke\('lu_load_model'/);
  assert.match(ui,/unloadModel\(\)/);
  assert.match(ui,/Cancel queued/);
  assert.match(ui,/lu-generation-queued/);
  assert.match(ui,/Message Lu/);
});

test('only top right gear opens visible Settings; models have a separate window',()=>{
  assert.match(ui,/className="quietSettings" title="Settings"/);
  assert.doesNotMatch(ui,/<button className="settingsButton" onClick=\{\(\) => setSettingsOpen\(true\)\}>⚙ Settings/);
  assert.match(ui,/setModelLibraryOpen\(true\);\}\}>Manage models/);
  assert.match(ui,/aria-label="Lu settings"/);
  assert.match(ui,/aria-label="Model Library"/);
  assert.match(ui,/No registered local GGUF models/);
  assert.match(ui,/Observe & Learn privacy settings/);
  assert.match(ui,/Select a conversation to review its native observation status/);
  assert.match(ui,/false && settingsOpen && <div className="settingsBackdrop"/);
  assert.match(css,/\.luFullWindow\{/);
});

test('Observe & Learn is not mounted in chat composer; only settings owns active panel',()=>{
  const chatEnd=ui.indexOf('</main>');
  const newSettings=ui.indexOf('aria-label="Lu settings"');
  const observedPanel=ui.indexOf('aria-label="Observe and Learn — Lu-only pilot"');
  assert.ok(chatEnd>=0 && newSettings>chatEnd && observedPanel>newSettings);
  assert.match(ui,/Enable experimental Observe & Learn/);
  assert.match(ui,/Forget saved observation notes for this chat/);
});

test('emoji reactions are contextual and open a picker below message',()=>{
  assert.match(ui,/className="reactionTrigger"/);
  assert.match(ui,/aria-expanded=\{reactionPickerSeq===entry.seq\}/);
  assert.match(ui,/className="reactionPicker"/);
  assert.match(ui,/setChatMessageReaction\(entry,emoji\)/);
  assert.match(css,/\.messageReactions\{display:flex;flex-direction:column/);
});

test('chat footer hides manual specialist tools behind an opt-in Tools disclosure',()=>{
  assert.match(ui,/<details className="chatToolsDrawer"><summary>Tools<\/summary>/);
  assert.match(ui,/<\/div><\/details>/);
  assert.match(ui,/File operations/);
  assert.match(ui,/Research with Lu browser/);
});

test('fake elapsed timer is absent; actual memory loading percent remains explicitly unverified',()=>{
  assert.doesNotMatch(ui,/modelLoadSeconds|s elapsed/);
  assert.match(ui,/Tensor loading % unavailable/);
  assert.match(ui,/workingSetRamBytes/);
  assert.match(ui,/dedicatedGpuBytes/);
});

test('ordinary human chat turns inject a fresh non-authoritative native device time observation',()=>{
  assert.match(rust,/if origin==GenerationOrigin::Human \{\s*model_prompt\.push_str\(&host_clock::fresh_local_clock_context\(\)\);/);
  assert.match(clock,/fn GetLocalTime\(/);
  assert.match(clock,/UTC Unix epoch seconds:/);
  assert.match(clock,/not an external instruction or tool grant/);
});
