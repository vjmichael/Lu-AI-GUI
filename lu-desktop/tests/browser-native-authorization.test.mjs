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

test('human verification can only be resumed by native user control',()=>{
  assert.ok(host.includes('async fn lu_browser_human_verified('));
  assert.ok(host.includes('state.browser_runtime.resolve_human_verification()?'));
  assert.ok(ui.includes("invoke('lu_browser_human_verified')"));
  assert.ok(ui.includes('browserRuntime?.humanVerificationRequired'));
  assert.ok(ui.includes('Complete the website challenge in the visible Chromium window.'));
  assert.ok(ui.includes("invoke('lu_browser_runtime_status')"));
  assert.ok(!host.includes('fn lu_browser_human_solve('));
});
test('actual browser tool-call path reads persisted USER turn and supervised model',()=>{
  assert.ok(host.includes('async fn run_authenticated_browser_model_turn('));
  assert.ok(host.includes('state.chats.trusted_user_turn_text(chat_id,user_turn_id)'));
  assert.ok(host.includes('supervisor.reserve_generation_with_history('));
  assert.ok(host.includes('supervisor.run_reserved_browser_function_calls('));
  assert.ok(host.includes('dispatch_authenticated_browser_model_call('));
});

test('user Watch/Hide uses the owned Chromium session and not model-facing tools',()=>{
  assert.ok(host.includes('async fn lu_browser_watch('));
  assert.ok(host.includes('async fn lu_browser_hide('));
  assert.ok(host.includes('state.browser_runtime.set_user_visible(visible)?'));
  assert.ok(ui.includes("lu_browser_watch"));
  assert.ok(ui.includes("lu_browser_hide"));
  assert.ok(ui.includes('browserRuntime?.humanVerificationRequired ||'));
  assert.ok(ui.includes("browserMode === 'always-visible'"));
  assert.ok(!host.includes('fn lu_browser_window_by_pid('));
});

test('browser research enters the same queued durable turn as ordinary chat',()=>{
  assert.ok(host.includes('async fn lu_generate_browser('));
  assert.ok(host.includes('state.generation_queue.enqueue('));
  assert.ok(host.includes('state.chats.append_user_with_turn_id(&chat_id,&generation_id,&prompt)?'));
  assert.ok(host.includes('ticket.wait_for_slot()?'));
  assert.ok(host.includes('start_browser_for_turn('));
  assert.ok(host.includes('gid.clone()'));
  assert.ok(host.includes('state.chats.append_browser_observation('));
  assert.ok(host.includes('supervisor.run_reserved_browser_function_calls('));
  assert.ok(host.includes('supervisor.run_reserved_with_history('));
  assert.ok(host.includes('state.chats.complete(&chat_id,&gid)?'));
  assert.ok(host.includes('state.browser_authority.revoke_task(&grant.task_id)'));
  assert.ok(host.includes('if !collected_read{'));
  assert.ok(ui.includes("invoke('lu_generate_browser'"));
});
test('queued browser CAPTCHA recovery requires real human takeover',()=>{
  assert.ok(host.includes('phase:"needs-human"'));
  assert.ok(host.includes('if ticket.cancelled()'));
  assert.ok(host.includes('human_verification_required'));
  assert.ok(host.includes('Duration::from_secs(600)'));
  assert.ok(host.includes('Browser closed during human verification'));
  assert.ok(ui.includes("invoke('lu_browser_human_verified')"));
});
test('browser research cannot silently opt ordinary chat into tool use',()=>{
  assert.ok(ui.includes('const [browserResearch, setBrowserResearch] = useState(false)'));
  assert.ok(ui.includes("Research with Lu browser (read only)"));
  assert.ok(ui.includes("await invoke('lu_generate'"));
  assert.ok(ui.includes("await invoke('lu_generate_browser'"));
  assert.ok(ui.includes('Native approval is required for each research turn.'));
  assert.ok(host.includes('Browser research requires an exact public HTTPS origin'));
});

test('browser research is progress-driven rather than capped at four rounds or eight reads',()=>{
  assert.ok(host.includes('let research_deadline=std::time::Instant::now()'));
  assert.ok(host.includes('Browser research deadline exceeded before verified reading'));
  assert.ok(host.includes('round=round.saturating_add(1)'));
  assert.ok(host.includes('if collected_read{break;}'));
  assert.ok(host.includes('if observed.len()>6{observed.remove(0);}'));
  assert.ok(host.includes('state.chats.append_browser_observation('));
  assert.ok(host.includes('if !seen.insert(fingerprint)'));
  assert.ok(!host.includes('for round in 0..4usize'));
  assert.ok(!host.includes('Browser observation count exceeded'));
});
