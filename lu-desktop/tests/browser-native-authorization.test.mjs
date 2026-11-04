import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const browser=readFileSync(new URL('../src-tauri/src/native_browser_host.rs',import.meta.url),'utf8');
const store=readFileSync(new URL('../src-tauri/src/chat_store.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const stage=readFileSync(new URL('../scripts/stage-browser-runtime.mjs',import.meta.url),'utf8');
const tauri=JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json',import.meta.url),'utf8'));

test('browser start binds a trusted persisted user turn, not a model claim',()=>{
  assert.ok(host.includes('latest_authenticated_user_turn_id(&chat_id)'));
  assert.ok(host.includes('state.browser_authority.open_user_task('));
  assert.ok(host.includes('user_turn_id:user_turn_id.clone()'));
  assert.ok(host.includes('BROWSER_READ_TOOL.into()'));
  assert.ok(host.includes('effect:ToolEffect::NetworkRead'));
  assert.ok(store.includes('pub fn latest_authenticated_user_turn_id('));
  assert.ok(store.includes('if !valid_id(&id,"g-")||!seen.insert(id.clone())'));
});
test('native modal is required before launching Chromium',()=>{
  assert.ok(host.includes('Authorize research browser'));
  assert.ok(host.includes('app.dialog().message(dialog_text)'));
  assert.ok(host.includes('.blocking_show()'));
  assert.ok(host.includes('if !approved{return Err("Browser request denied by user"'));
  assert.ok(host.includes('state.browser_runtime.require_launch_ready()?'));
  assert.ok(ui.includes("invoke('lu_browser_start'"));
  assert.ok(ui.includes('!browserRuntime?.launchReady'));
  assert.ok(browser.includes('fn package_verified(&self)->bool'));
});
test('stop, failed launch, and startup revocation shut down browser ownership',()=>{
  assert.ok(host.includes('state.browser_authority.revoke_task(&task.task_id)'));
  assert.ok(host.includes('let stop_result=state.browser_runtime.stop()'));
  assert.ok(host.includes('if !still_authorized{'));
  assert.ok(host.includes('browser startup was cancelled')||host.includes('Browser startup was cancelled'));
});
test('reviewed package embeds manifest and Tauri includes browser resources',()=>{
  assert.ok(browser.includes('EMBEDDED_BROWSER_MANIFEST'));
  assert.ok(browser.includes('include_str!("../resources/lu-browser/manifest.json")'));
  assert.equal(tauri.bundle.resources['resources/lu-browser'],'lu-browser');
  assert.ok(stage.includes('LU_BROWSER_MANIFEST_SHA256'));
  assert.ok(stage.includes('manifest.platform!==process.platform'));
  assert.ok(stage.includes('manifest.arch!==process.arch'));
});

test('desktop registers scoped browser model tools only after native consent and launch',()=>{
  assert.ok(host.includes('register_scoped_browser_tools('));
  assert.ok(host.includes('let scoped=lu_core::native_browser_tool::ScopedBrowserTask'));
  assert.ok(host.includes('authority:state.browser_authority.clone()'));
  assert.ok(host.includes('policy:Arc::new(BrowserReadOnlyPolicy)'));
  assert.ok(host.includes('runner:Arc::clone(&state.browser_runtime)'));
  assert.ok(host.includes('model_catalog=lu_core::model::ModelToolCatalog::new()'));
  assert.ok(host.includes('browser_tools: Mutex<Option<DesktopBrowserTools>>'));
  assert.ok(host.includes('tools.take()'));
  assert.ok(!host.includes('fn lu_browser_execute('));
});

test('host internal model calls use only the active trusted turn and read-only catalog',()=>{
  assert.ok(host.includes('async fn dispatch_authenticated_browser_model_call('));
  assert.ok(host.includes('task.chat_id!=chat_id||task.user_turn_id!=user_turn_id'));
  assert.ok(host.includes('task.execution_scope_id.clone()'));
  assert.ok(host.includes('allowed.insert("lu_browser_read".into())'));
  assert.ok(host.includes('tools.model_catalog.encode_call('));
  assert.ok(host.includes('tools.registry.dispatch(invocation,&BrowserReadOnlyPolicy).await'));
  assert.ok(host.includes('tools.model_catalog.decode_result('));
  assert.ok(!host.includes('fn lu_browser_execute('));
});
