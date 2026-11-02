import assert from 'node:assert/strict';
import { readFileSync, statSync } from 'node:fs';
const ui = readFileSync(new URL('../web/main.jsx', import.meta.url), 'utf8');
const host = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8');
const store = readFileSync(new URL('../src-tauri/src/chat_store.rs', import.meta.url), 'utf8');
const css = readFileSync(new URL('../web/styles.css', import.meta.url), 'utf8');
const js = readFileSync(new URL('../dist/assets/app.js', import.meta.url), 'utf8');
const tests = [
  ['native GGUF import and generate remain Lu-only', ui.includes("invoke('lu_import_model')") && ui.includes("invoke('lu_generate'")],
  ['no Jan execution paths introduced', !/jan_llama|JanInference|jan-cowork/.test(ui + host)],
  ['new chats and conversation history are host-owned', ui.includes("invoke('lu_new_chat')") && ui.includes("invoke('lu_chats')") && host.includes('fn lu_new_chat(')],
  ['one-level folders and favorites are durable', ui.includes("invoke('lu_new_folder'") && ui.includes("invoke('lu_favorite_chat'") && store.includes('pub fn favorite(')],
  ['models available from a quick dropdown', ui.includes('modelSwitcher') && ui.includes('modelMenu')],
  ['model management only in Settings', ui.includes('settingsOpen') && ui.includes('Settings') && !ui.includes('MODEL CONTROL')],
  ['no visible Lu Native Runtime chip', !ui.includes('Lu Native Runtime')],
  ['selection does not start model inference', !/onClick=\{\(\) => \{ setSelectedModel\([^}]*loadModel/.test(ui) && ui.includes('Selection does not load')],
  ['model discovery stays host owned and explicit', host.includes('fn scan_unindexed_models(') && host.includes('fn lu_rescan_models(')],
  ['no auto load in application setup', !/\.setup\([\s\S]*?lu_load_model\(/.test(host)],
  ['prompt and image inputs preserved on failure', ui.includes('if (completed) {') && ui.includes("setPrompt('');")],
  ['intentional cancellation clears partial text and red error', ui.includes("setStream('');") && ui.includes('Cancelled|Canceled')],
  ['composer is resizable and collapsible', ui.includes('beginComposerDrag') && ui.includes('composerCollapsed') && css.includes('composerResizer')],
  ['brighter green model loaded indicator', css.includes('#22f66e')],
  ['fixed host image ID length matches Lu Core', host.includes('image_ref.len() != 36') && !host.includes('image_ref.len() != 68')],
  ['model discovery never imports MMProj as a main model', host.includes('lowered.starts_with("mmproj")')],
  ['frontend build exists and includes chat commands', statSync(new URL('../dist/assets/app.js', import.meta.url)).size > 100000 && js.includes('lu_new_chat') && js.includes('lu_rescan_models')],
];
for (const [name, pass] of tests) {
  console.log(`${pass ? 'PASS' : 'FAIL'} ${name}`);
  assert.ok(pass, name);
}
console.log(`R15 independent desktop source and bundle checks: ${tests.length}/${tests.length}`);
