// CI-REPAIR-01: verify the current split Tauri Browser authority boundary.
// The 2026-11-05 version asserted deleted monolithic implementation names,
// producing 16 false failures even when the native protections remained.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const read=relative=>readFileSync(new URL(relative,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs');
const model=read('../src-tauri/src/browser_model_desktop.rs');
const runtime=read('../src-tauri/src/native_browser_host.rs');
const store=read('../src-tauri/src/chat_store.rs');
const ui=read('../web/main.jsx');
const stage=read('../scripts/stage-browser-runtime.mjs');
const tauri=JSON.parse(read('../src-tauri/tauri.conf.json'));
const scoped=read('../../src-tauri/lu-core/src/native_browser_tool.rs');

test('browser is bound to a persisted authenticated human turn and read-only authority',()=>{
  assert.match(host,/latest_authenticated_user_turn_id\(&chat_id\)/);
  assert.match(host,/browser_authority\.open_user_task\(TaskAuthority/);
  assert.match(host,/user_turn_id:.*clone\(\)/);
  assert.match(host,/effect:ToolEffect::NetworkRead/);
  assert.match(store,/pub fn latest_authenticated_user_turn_id\(/);
  assert.match(model,/struct BrowserReadOnlyPolicy/);
  assert.match(model,/ToolEffect::NetworkRead/);
  assert.match(model,/Browser model has read-only permission/);
});
test('native modal and package integrity gate run before Chromium launch',()=>{
  assert.match(host,/Read-only Browser grant/);
  assert.match(host,/\.blocking_show\(\)/);
  assert.match(host,/if !approved\{return Err\("Browser grant cancelled"/);
  assert.match(host,/browser_runtime\.require_launch_ready\(\)\?/);
  assert.match(host,/browser_authority\.open_user_task/);
  assert.match(host,/browser_runtime\.launch_for_native_grant/);
  assert.match(runtime,/fn package_verified\(&self\)->bool/);
  assert.match(runtime,/EMBEDDED_BROWSER_MANIFEST/);
  assert.match(runtime,/include_str!\("\.\.\/resources\/lu-browser\/manifest\.json"\)/);
  assert.equal(tauri.bundle.resources['resources/lu-browser'],'lu-browser');
  assert.match(stage,/LU_BROWSER_MANIFEST_SHA256/);
});
test('stop or failed start revokes the grant and ends owned Chromium process tree',()=>{
  assert.match(host,/async fn lu_browser_stop\(/);
  assert.match(host,/browser_authority\.revoke_task\(&g\.task_id\)/);
  assert.match(host,/owner\.browser_runtime\.stop\(\)/);
  assert.match(host,/Browser start was cancelled/);
  assert.match(runtime,/impl Drop for DesktopBrowserHost/);
  assert.doesNotMatch(host,/fn lu_browser_execute\(/);
});
test('native tool registry is constructed under exact task identity; no general WebView executor',()=>{
  assert.match(model,/register_scoped_browser_tools\(/);
  assert.match(model,/ScopedBrowserTask\{/);
  assert.match(model,/conversation_id:chat_id\.clone\(\),user_turn_id:/);
  assert.match(model,/scope:BrowserOriginScope::from_host_grant/);
  assert.match(model,/BrowserReadOnlyPolicy/);
  assert.match(model,/catalog\.encode_call\(/);
  assert.match(model,/registry\.dispatch\(invocation,policy\.as_ref\(\)\)/);
  assert.match(scoped,/pub fn register_scoped_browser_tools\(/);
  assert.doesNotMatch(host,/fn lu_browser_execute\(/);
});
test('model browser read verifies native result, redirect origin and live grant before persistence',()=>{
  assert.match(model,/trusted_user_turn_text\(/);
  assert.match(model,/run_reserved_browser_function_calls\(/);
  assert.match(model,/result_scope\.allows\(/);
  assert.match(model,/Browser read escaped approved HTTPS origins/);
  assert.match(model,/owner\.research\.record_browser_read\(/);
  assert.match(model,/grant_current\(/);
  assert.match(model,/browser-call-/);
});
test('CAPTCHA is human-owned and user watch/hide is never a model-facing network tool',()=>{
  assert.match(host,/async fn lu_browser_human_verified\(/);
  assert.match(host,/browser_runtime\.resolve_human_verification\(\)\?/);
  assert.match(host,/async fn lu_browser_set_visible\(/);
  assert.match(host,/browser_runtime\.set_user_visible\(visible\)\?/);
  assert.match(runtime,/pub fn set_user_visible\(&self,visible:bool\)/);
  assert.match(ui,/lu_browser_set_visible/);
  assert.match(ui,/lu_browser_human_verified/);
  assert.match(model,/human_verification_required/);
  assert.match(model,/phase:"needs-human"/);
  assert.doesNotMatch(model,/fn lu_browser_human_solve\(/);
});
test('Browser research creates a durable queued USER turn and revokes it on exit',()=>{
  assert.match(model,/async fn lu_generate_browser\(/);
  assert.match(model,/generation_queue\.enqueue\(/);
  assert.match(model,/chats\.append_user_with_turn_id\(&chat_id,&gid,&prompt\)/);
  assert.match(model,/ticket\.wait_for_slot\(\)\?/);
  assert.match(model,/start_browser_for_exact_turn\(/);
  assert.match(model,/chats\.append_browser_observation\(/);
  assert.match(model,/supervisor\.run_reserved_with_history\(/);
  assert.match(model,/owner\.browser_authority\.revoke_task\(&grant\.task_id\)/);
  assert.match(ui,/lu_generate_browser/);
});
test('Browser research opt-in is per Send and cannot become ordinary chat permission',()=>{
  assert.match(ui,/browserResearchNextSend/);
  assert.match(ui,/setBrowserResearchNextSend\(false\)/);
  assert.match(ui,/useBrowserResearch\s*\?/);
  assert.match(ui,/lu_generate'\s*,/);
  assert.match(model,/Browser research requires exact public HTTPS origins/);
  assert.match(host,/fn browser_origin_list\(raw:&str\)/);
  assert.match(host,/BrowserOriginScope::from_host_grant/);
});
test('research source collection is progress-based, time bounded and deduplicated',()=>{
  assert.match(model,/let deadline=std::time::Instant::now\(\)\+Duration::from_secs\(600\)/);
  assert.match(model,/attempts>120/);
  assert.match(model,/let mut calls_seen=HashSet::<String>::new\(\)/);
  assert.match(model,/if !calls_seen\.insert\(fingerprint\)/);
  assert.match(model,/let mut evidence=BTreeMap::<String,/);
  assert.match(model,/if evidence\.is_empty\(\)/);
  assert.match(model,/if recent\.len\(\)>6/);
  assert.doesNotMatch(model,/for round in 0\.\.4usize/);
});
test('only real read/accessibility snapshots enter citations, not navigation or challenges',()=>{
  assert.match(model,/matches!\(kind,"READ"\|"ACCESSIBILITY"\)/);
  assert.match(model,/scope\.allows\(url\)/);
  assert.match(model,/let identity=format!\("\{\}#\{\}",saved\.source_url,saved\.observed_sha256\)/);
  assert.match(model,/owner\.research\.record_browser_read\(/);
  assert.match(model,/Navigation\/status is not proof that text was read/);
  assert.match(model,/browser_research_prompt\(/);
  assert.match(model,/Do not cite omitted contents/);
});
test('citation gate cannot stream or complete unverified model prose',()=>{
  const start=model.indexOf('let mut answer=String::new()');
  const audit=model.indexOf('audit_browser_research_answer(&answer,&citation_sources)',start);
  const progress=model.indexOf('owner.chats.append_progress(&chat_id,&gid,&answer)',start);
  const finish=model.indexOf('owner.chats.complete(&chat_id,&gid)?',start);
  assert.ok(start>=0&&start<audit&&audit<progress&&progress<finish);
  assert.doesNotMatch(model.slice(start,audit),/app\.emit\("lu-token"/);
  assert.match(model,/render_observed_source_appendix\(&citation_sources\)/);
});
