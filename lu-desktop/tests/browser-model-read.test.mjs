import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const file=(relative)=>readFileSync(new URL(relative,import.meta.url),'utf8');
const host=file('../src-tauri/src/lib.rs');
const bridge=file('../src-tauri/src/browser_model_desktop.rs');
const ui=file('../web/main.jsx');
const supervisor=file('../../src-tauri/lu-core/src/inference_supervisor.rs');
const engine=file('../../src-tauri/lu-core/src/persistent_llama_backend.rs');
const transport=file('../../src-tauri/lu-core/src/multimodal_http.rs');

test('Browser model probes use the same model session and queue, not direct WebView commands',()=>{
  assert.match(host,/browser_model_desktop::lu_browser_model_read,/);
  assert.match(bridge,/pub\(crate\) async fn lu_browser_model_read\(/);
  assert.match(bridge,/owner\.generation_queue\s*\.enqueue\(/);
  assert.match(bridge,/ticket\.wait_for_slot\(\)/);
  assert.match(bridge,/owner\.supervisor\.reserve_generation_with_history\(/);
  assert.match(bridge,/owner\.supervisor\.run_reserved_browser_function_calls\(/);
  assert.match(supervisor,/pub fn run_reserved_browser_function_calls\(/);
  assert.match(engine,/build_read_only_browser_tool_request\(/);
  assert.match(transport,/generate_browser_tool_stream_body\(/);
  assert.match(ui,/invoke\('lu_browser_model_read'/);
  assert.doesNotMatch(host,/fn lu_browser_execute\(/);
});
test('Model proposals always cross the strict native authority and approved HTTPS origins',()=>{
  assert.match(bridge,/trusted_user_turn_text\(&chat_id,&user_turn_id\)/);
  assert.match(bridge,/latest_authenticated_user_turn_id\(&chat_id\)/);
  assert.match(bridge,/BrowserOriginScope::from_host_grant\(origins\)/);
  assert.match(bridge,/register_scoped_browser_tools\(/);
  assert.match(bridge,/BTreeSet::from\(\["lu_browser_read"/);
  assert.match(bridge,/catalog\.encode_call\(/);
  assert.match(bridge,/registry\.dispatch\(invocation,policy\.as_ref\(\)\)/);
  assert.match(bridge,/catalog\.decode_result\(/);
  assert.match(bridge,/descriptor\.effect==ToolEffect::NetworkRead/);
  assert.match(bridge,/source_verified_for_citation:false/);
});
test('Browser Stop revokes the task and interrupts the queued or active model read',()=>{
  assert.match(host,/active_read:Option<\(String,String\)>/);
  assert.match(host,/owner\.generation_queue\.cancel\(session_id,generation_id\)/);
  assert.match(host,/owner\.supervisor\.cancel\(session_id,generation_id/);
  assert.match(host,/owner\.browser_authority\.revoke_task\(&g\.task_id\)/);
  assert.match(bridge,/struct ProbeLease/);
  assert.match(bridge,/if ticket\.cancelled\(\)\|\|!grant_current\(/);
  // Simplified Lu chat removed the legacy explicit browser button. Original
  // human chat intent is routed by the host, never by an ambient UI control.
  assert.match(host,/browser_model_desktop::explicit_browse_origin/);
  assert.doesNotMatch(ui,/Let Lu inspect this approved browser/);
});
test('Preview is bounded untrusted evidence, never raw screenshots or invented citations',()=>{
  assert.match(bridge,/take\(2048\)/);
  assert.match(bridge,/if calls\.len\(\)>8/);
  assert.match(bridge,/source_is_untrusted:true/);
  assert.match(bridge,/source_verified_for_citation:false/);
  // Untrusted preview stays host-owned. The old preview panel was removed;
  // its absence is not evidence of permission, citation, or execution.
  const settings=file('../web/native_windows.jsx');
  assert.match(settings,/Browser & research/);
  assert.match(settings,/Research source checking, cited page reading and retrieved documents are initiated through the conversation/);
  assert.doesNotMatch(settings,/invoke\('lu_browser_model_read'/);
  assert.doesNotMatch(ui,/Model-guided browser observations|page\.excerpt&&<pre>/);
  assert.doesNotMatch(bridge,/response\.image|base64|screenshot_bytes/);
});

test('text-only browser model tool schema does not advertise screenshots',()=>{
  const codec=file('../../src-tauri/lu-core/src/browser_model_codec.rs');
  const request=file('../../src-tauri/lu-core/src/browser_model_request.rs');
  assert.match(codec,/BrowserModelKind::Read=>vec!\["navigate","read","status","accessibility"\]/);
  assert.match(request,/let expected=\["navigate","read","status","accessibility"\]/);
  assert.doesNotMatch(codec,/BrowserModelKind::Read,"screenshot"/);
  assert.match(codec,/model_read_catalog_excludes_screenshot_bytes/);
});
