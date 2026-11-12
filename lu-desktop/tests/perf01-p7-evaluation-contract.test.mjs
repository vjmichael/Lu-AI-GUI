import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const core=read('../../src-tauri/lu-core/src/p7_backend_evaluation.rs');
const rustModules=read('../../src-tauri/lu-core/src/lib.rs');
const desktop=read('../src-tauri/src/lib.rs');
const picker=read('../src-tauri/src/preview_engine_choice.rs');
const candidateProbe=read('../../src-tauri/lu-core/tests/perf01_p7_candidate_protocol_real.rs');
const baseline=read('../../src-tauri/lu-core/src/persistent_llama_backend.rs');
const analyzer=read('../../scripts/perf01-p7-backend-comparison.mjs');

test('P7 keeps installed llama.cpp and single-GPU offline backend unchanged',()=>{
  assert.match(rustModules,/pub mod p7_backend_evaluation;/);
  assert.match(desktop,/PersistentLlamaBackend::new\(PersistentEngineConfig/);
  assert.match(desktop,/tuning:None/);
  assert.match(picker,/enum BackendChoice \{ Cpu, Vulkan \}/);
  assert.doesNotMatch(picker,/CandidateKind/);
  for(const id of ['mistral_rs','koboldcpp','vllm_gguf']){
    assert.ok(desktop.includes('backend:"'+id+'"'),id+' missing from inventory');
  }
  assert.match(desktop,/selectable:false,selected_on_next_start:false/);
  assert.doesNotMatch(desktop,/p7_backend_evaluation::CandidateManifest/);
});
test('P7 candidate review pins independent executable and GGUF plus exact MMProj',()=>{
  assert.match(core,/expected_executable_sha256:\[u8;32\]/);
  assert.match(core,/expected_model_sha256:\[u8;32\]/);
  assert.match(core,/expected_projector_sha256:Option<\[u8;32\]>/);
  assert.match(core,/fn digest_original_file\(/);
  assert.match(core,/if sha!=self\.expected_executable_sha256/);
  assert.match(core,/if model_sha!=self\.expected_model_sha256/);
  assert.match(core,/if actual!=expected/);
  assert.match(core,/projector\.is_some\(\)!=self\.expected_projector_sha256\.is_some\(\)/);
  assert.match(core,/ModelMismatch,ProjectorMismatch/);
  assert.match(core,/fn projector_hash_is_required_for_multimodal_evaluation/);
  assert.doesNotMatch(core,/Command::new\(/);
});
test('P7 alternative streaming retains original token/cancel and tool authority boundaries',()=>{
  assert.match(core,/pub struct EvaluatedInferenceBackend/);
  assert.match(core,/impl<B:InferenceBackend> InferenceBackend for EvaluatedInferenceBackend/);
  assert.match(core,/chunk\.index!=expected/);
  assert.match(core,/const MAX_OUTPUT_CHUNK_BYTES:usize=64\*1024/);
  assert.match(core,/const MAX_OUTPUT_BYTES:usize=1024\*1024/);
  assert.match(core,/if cancel\.is_cancelled\(\)/);
  assert.match(core,/P7 candidate MMProj not independently accepted/);
  assert.match(core,/P7 tool-call parity absent; model text is not a tool/);
  assert.match(core,/self\.native\.generate_browser_function_calls\(/);
  assert.match(core,/self\.native\.set_trusted_cache_scope\(None\)/);
  assert.match(baseline,/fn generate_with_images\(/);
  assert.match(baseline,/self\.cache_gate\.invalidate\(\)/);
});
test('P7 cannot self-promote performance, capabilities or nonlocal endpoints',()=>{
  assert.match(core,/pub fn production_selectable\(self\)->bool\{false\}/);
  assert.match(core,/pub fn candidate_can_replace_production\(&self\)->bool\{false\}/);
  assert.match(core,/fn no_candidate_is_promoted_from_documentation/);
  assert.match(core,/fn candidate_text_stream_is_bounded_and_vision_and_tools_fail_closed/);
  assert.doesNotMatch(core,/std::net::TcpStream/);
  assert.doesNotMatch(core,/reqwest/);
  assert.match(analyzer,/installedBackendPromotionAllowed:false/);
  assert.match(analyzer,/evidenceScope:'OWNER_SUPPLIED_RECEIPTS_NOT_INDEPENDENTLY_ATTESTED'/);
});
test('P7 opt-in live probe uses synthetic host messages only and cannot attest process custody',()=>{
  assert.match(candidateProbe,/#\[ignore = "requires separately reviewed/);
  assert.match(candidateProbe,/LU_P7_CANDIDATE_ENGINE_SHA256/);
  assert.match(candidateProbe,/LU_P7_MODEL_SHA256/);
  assert.match(candidateProbe,/LU_P7_CANDIDATE_API_KEY/);
  assert.match(candidateProbe,/build_structured_chat_request\(/);
  assert.match(candidateProbe,/generate_chat_stream_body\(/);
  assert.match(candidateProbe,/"candidateProcessIdentityIndependentlyEstablished":false/);
  assert.match(candidateProbe,/"nativeChildReapedOrCancellationProven":false/);
  assert.match(candidateProbe,/"productionPromotionAllowed":false/);
  assert.doesNotMatch(candidateProbe,/state\.chats/);
});
