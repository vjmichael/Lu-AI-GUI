import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const rust=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const reviewed=readFileSync(new URL('../src-tauri/src/reviewed_app_desktop.rs',import.meta.url),'utf8');
const chat=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const windows=readFileSync(new URL('../web/native_windows.jsx',import.meta.url),'utf8');
const cap=JSON.parse(readFileSync(new URL('../src-tauri/capabilities/default.json',import.meta.url),'utf8'));

test('secondary Tauri windows have fixed native labels/paths and cannot open arbitrary URLs',()=>{
  assert.match(rust,/fn auxiliary_window_spec\(caller:&str,kind:&str\)/);
  assert.match(rust,/\("main","settings"\)/);
  assert.match(rust,/\("main"\|"settings","model-library"\)/);
  assert.match(rust,/WebviewUrl::App\(spec\.query\.into\(\)\)/);
  assert.match(rust,/fn lu_close_aux_window\(caller:tauri::WebviewWindow\)/);
  assert.deepEqual(cap.windows,['main','settings','model-library']);
});
test('native windows mount instead of Chat App and keep distinct focus, no background worker effects',()=>{
  assert.match(chat,/luWindow==='settings'\?<SettingsWindow\/>/);
  assert.match(chat,/luWindow==='model-library'\?<ModelLibraryWindow\/>:<App\/>/);
  assert.match(windows,/These roots mount INSTEAD of App/);
  assert.match(windows,/invoke\('lu_close_aux_window'\)/);
  assert.match(windows,/aria-label="Settings sections"/);
  assert.match(windows,/aria-label="Model library tabs"/);
  assert.match(chat,/\{false && <details className="chatToolsDrawer"/);
});
test('settings inventory is model-neutral but never a tool grant or verification claim',()=>{
  assert.match(rust,/fn lu_native_route_catalog\(\)->Vec<LuNativeRouteView>/);
  assert.match(rust,/NativeCapabilityView from a granted model turn/);
  assert.match(rust,/ConversationAuthority and registered codecs/);
  assert.match(windows,/not automatic grants/);
  assert.match(windows,/not independently verified/);
  assert.match(rust,/id:"reviewed\.media"/);
  assert.match(rust,/id:"reviewed\.git"/);
});
test('ordinary user chat includes a conservative media route with native review and real model choice',()=>{
  assert.match(reviewed,/fn explicit_media_metadata_request\(prompt:&str\)->bool/);
  assert.match(reviewed,/original_human_media_fact_requests_offer_only_native_picker/);
  assert.match(rust,/reviewed_app_desktop::explicit_media_metadata_request\(&prompt\)/);
  assert.match(rust,/reviewed_app_desktop::lu_reviewed_app_ask/);
  assert.match(rust,/workspace_desktop::explicit_read_git_request/);
  assert.match(rust,/browser_model_desktop::explicit_browse_origin/);
  assert.match(rust,/reader_desktop::explicit_reader_document_request/);
});
