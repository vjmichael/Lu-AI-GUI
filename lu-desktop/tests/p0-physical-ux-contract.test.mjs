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

test('one gear launches native Settings; Model Library and chat are distinct Tauri roots',()=>{
  const windows=readFileSync(new URL('../web/native_windows.jsx',import.meta.url),'utf8');
  const capability=JSON.parse(readFileSync(new URL('../src-tauri/capabilities/default.json',import.meta.url),'utf8'));
  assert.match(ui,/className="quietSettings" title="Settings"/);
  assert.match(ui,/invoke\('lu_open_aux_window',\{kind:'settings'\}\)/);
  assert.match(ui,/invoke\('lu_open_aux_window',\{kind:'model-library'\}\)/);
  assert.match(ui,/luWindow==='settings'\?<SettingsWindow\/>/);
  assert.match(ui,/luWindow==='model-library'\?<ModelLibraryWindow\/>/);
  assert.match(rust,/fn lu_open_aux_window\(/);
  assert.match(rust,/WebviewWindowBuilder::new/);
  assert.match(rust,/spec\.query/);
  assert.deepEqual(capability.windows,['main','settings','model-library']);
  assert.match(windows,/aria-label="Lu settings"/);
  assert.match(windows,/aria-label="Model Library"/);
  assert.match(windows,/Model-specific settings/);
  assert.match(windows,/Lu runtime/);
  assert.match(windows,/Tools & permissions/);
  assert.match(windows,/Browser & research/);
  assert.match(windows,/Memory & ANSF/);
  assert.match(windows,/Privacy & Observe/);
  assert.match(windows,/Hugging Face/);
  assert.match(windows,/No registered local GGUF models/);
});

test('Observe & Learn and configuration are mounted only in an independent native Settings root',()=>{
  const windows=readFileSync(new URL('../web/native_windows.jsx',import.meta.url),'utf8');
  assert.match(windows,/invoke\('lu_observe_'\+action,\{chatId\}\)/);
  assert.match(windows,/lu_observe_forget_notes/);
  assert.match(windows,/Request native consent/);
  assert.match(windows,/OFF by default/);
  assert.doesNotMatch(ui,/chatToolsDrawer|settingsBackdrop|modelLibraryOpen&&/);
  assert.match(ui,/window\.location\.search/);
  assert.match(ui,/<SettingsWindow\/>/);
  assert.match(ui,/<ModelLibraryWindow\/>/);
});

test('emoji reactions are contextual and open a picker below message',()=>{
  assert.match(ui,/className="reactionTrigger"/);
  assert.match(ui,/aria-expanded=\{reactionPickerSeq===entry.seq\}/);
  assert.match(ui,/className="reactionPicker"/);
  assert.match(ui,/setChatMessageReaction\(entry,emoji\)/);
  assert.match(css,/\.messageReactions\{display:flex;flex-direction:column/);
});

test('specialist manual buttons are not mounted in composer; host native routes remain',()=>{
  assert.doesNotMatch(ui,/chatToolsDrawer|<summary>Tools<\/summary>/);
  assert.match(rust,/fn lu_native_route_catalog\(/);
  assert.match(rust,/reviewed_app_desktop::explicit_media_metadata_request/);
  assert.match(rust,/workspace_desktop::explicit_read_workspace_request\(&prompt\)/);
  assert.doesNotMatch(ui,/routeExplicitLocalFolderRequest\(input\)/);
});

test('model-proposed Office and document drafts remain inert until separate native preview and user consent',()=>{
  const action=readFileSync(new URL('../src-tauri/src/native_action_proposal.rs',import.meta.url),'utf8');
  assert.match(action,/pub struct NativeDocumentDraft/);
  assert.match(action,/pub struct NativeOfficePatch/);
  assert.match(action,/fn valid_patch\(/);
  assert.match(action,/model_drafts_are_bounded_inert_and_action_specific/);
  assert.match(ui,/if\(approved\.documentDraft\)/);
  assert.match(ui,/if\(approved\.officePatch\)/);
  assert.match(ui,/if\(approved\.officeBatch\)/);
  assert.match(rust,/fn lu_confirm_document_create\(/);
  assert.match(rust,/fn lu_confirm_office_revision\(/);
  assert.match(rust,/fn lu_confirm_office_batch\(/);
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
