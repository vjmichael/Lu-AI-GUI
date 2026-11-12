import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const files={
  adapter:'../../src-tauri/lu-core/src/p7_koboldcpp_backend.rs',
  custody:'../../src-tauri/lu-core/src/p7_kobold_custody.rs',
  preflight:'../../src-tauri/lu-core/src/p7_backend_evaluation.rs',
  native:'../../src-tauri/lu-core/tests/p7l_kobold_process_custody.rs',
  protocol:'../../src-tauri/lu-core/tests/p7l_kobold_http_protocol.rs',
  mock:'../../src-tauri/lu-core/src/bin/lu_p7l_kobold_http_fixture.rs',
  compare:'../../src-tauri/lu-core/tests/p7l_real_kobold_comparison.rs',
  desktop:'../src-tauri/src/lib.rs',
  original:'../../src-tauri/lu-core/src/persistent_llama_backend.rs',
  cargo:'../../src-tauri/lu-core/Cargo.toml',
  modules:'../../src-tauri/lu-core/src/lib.rs',
  analyzer:'../../scripts/perf01-p7-backend-comparison.mjs',
};
const source=Object.fromEntries(Object.entries(files)
    .map(([key,path])=>[key,readFileSync(new URL(path,import.meta.url),'utf8')]));

test('P7L is a concrete original InferenceBackend but cannot become installed Lu automatically',()=>{
  assert.match(source.adapter,/pub struct KoboldCppBackend/);
  assert.match(source.adapter,/impl InferenceBackend for KoboldCppBackend/);
  assert.match(source.adapter,/CandidateKind::KoboldCpp/);
  assert.match(source.modules,/pub mod p7_kobold_custody;/);
  assert.match(source.modules,/pub mod p7_koboldcpp_backend;/);
  assert.match(source.desktop,/PersistentLlamaBackend::new\(PersistentEngineConfig/);
  assert.match(source.desktop,/tuning:None/);
  assert.doesNotMatch(source.desktop,/KoboldCppBackend::new\(/);
  assert.match(source.desktop,/backend:"koboldcpp",engine:"KoboldCpp",packaged:false/);
  assert.match(source.desktop,/selectable:false,selected_on_next_start:false/);
});
test('P7L child runs CPU/loopback, pins model and secret in env not argv',()=>{
  for(const flag of ['--model','--contextsize','--threads','--usecpu',
    '--gpulayers','--host','--port','--quiet','--mmproj']){
    assert.ok(source.adapter.includes(flag),'missing '+flag);
  }
  assert.match(source.adapter,/command\.env_clear\(\)/);
  assert.match(source.adapter,/\.env\("KCPP_PASSWORD",&self\.config\.host_secret\)/);
  assert.match(source.adapter,/\.arg\("127\.0\.0\.1"\)/);
  assert.doesNotMatch(source.adapter,/\.arg\("--password"\)/);
  assert.doesNotMatch(source.adapter,/--remotetunnel|--agent|--mcpfile/);
  assert.match(source.adapter,/candidate\.recheck_originals\(\)/);
  assert.match(source.preflight,/pub fn recheck_originals\(&self\)/);
  assert.match(source.preflight,/Some\(actual\)!=self\.projector_sha256/);
  assert.match(source.adapter,/KoboldCppBackend::authenticated_probe/);
});
test('P7L native custody covers original Job and process-group kill, failing closed on restart',()=>{
  assert.match(source.adapter,/Job::new\(\)/);
  assert.match(source.adapter,/job\.assign\(&child\)/);
  assert.match(source.adapter,/command\.process_group\(0\)/);
  assert.match(source.adapter,/kill\(-\(pid as i32\),9\)/);
  assert.match(source.adapter,/child\.wait\(\)/);
  assert.match(source.adapter,/self\.finish_after_reap\(reaped\)/);
  assert.match(source.custody,/PRAGMA journal_mode=WAL/);
  assert.match(source.custody,/PRAGMA synchronous=FULL/);
  assert.match(source.custody,/TransactionBehavior::Immediate/);
  assert.match(source.custody,/state IN \('STARTING','RUNNING','REAPED'\)/);
  assert.match(source.custody,/CustodyError::ExistingUnreconciled/);
  assert.match(source.native,/fn durable_starting_state_survives_reopen_and_refuses_unverified_restart/);
  assert.match(source.native,/fn native_cancellation_interrupts_stream_and_reaps_original_child/);
});
test('P7L transport uses original role-protected streaming and tool collector without text scraping',()=>{
  assert.match(source.adapter,/build_structured_chat_request\(/);
  assert.match(source.adapter,/generate_chat_stream_body\(/);
  assert.match(source.adapter,/build_read_only_browser_tool_request\(/);
  assert.match(source.adapter,/generate_browser_tool_stream_body\(/);
  assert.match(source.adapter,/BTreeSet::from\(\["lu_browser_read"\.into\(\)\]\)/);
  assert.match(source.adapter,/if result\.is_err\(\)\|\|cancelled/);
  assert.match(source.adapter,/self\.unload\(\)/);
  assert.match(source.protocol,/fn streamed_browser_call_requires_original_function_name_and_valid_json/);
  assert.match(source.protocol,/fn consumer_cancellation_interrupts_live_original_sse_reader/);
  assert.match(source.native,/fn malformed_original_sse_is_terminal_and_reaps_child/);
  assert.match(source.native,/fn unauthorized_original_tool_delta_is_terminal_without_dispatch/);
});
test('P7L native fixture is opt-in, synthetic and omitted from production Lu package',()=>{
  assert.match(source.cargo,/p7l-native-fixture = \[\]/);
  assert.match(source.cargo,/required-features = \["p7l-native-fixture"\]/);
  assert.match(source.native,/#!\[cfg\(feature="p7l-native-fixture"\)\]/);
  assert.match(source.mock,/TEST-ONLY child/);
  assert.match(source.mock,/KCPP_PASSWORD/);
  assert.match(source.mock,/P7L_FAKE_BAD_SSE/);
  assert.match(source.mock,/P7L_FAKE_UNAUTHORIZED_TOOL/);
  assert.doesNotMatch(source.desktop,/p7l-native-fixture/);
  assert.match(source.compare,/#\[ignore = "requires reviewed physical/);
  assert.match(source.compare,/#\[ignore = "requires original Kobold/);
  assert.match(source.compare,/"physicalOwnerAcceptance":false/);
  assert.match(source.compare,/"generatedTokens":null/);
  assert.match(source.analyzer,/installedBackendPromotionAllowed:false/);
});
