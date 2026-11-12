import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const repoRoot=new URL('../../',import.meta.url);
const source=path=>readFileSync(new URL(path,repoRoot),'utf8');
const policy=source('src-tauri/lu-core/src/inference_tuning.rs');
const fingerprint=source('src-tauri/lu-core/src/gguf_metadata_summary.rs');
const runtime=source('src-tauri/lu-core/src/persistent_llama_backend.rs');
const desktop=source('lu-desktop/src-tauri/src/lib.rs');
const comparison=source('src-tauri/lu-core/tests/perf01_p5_real_tuning.rs');
const smokeFiles=[
  'src-tauri/lu-core/examples/inference_supervisor_smoke.rs',
  'src-tauri/lu-core/examples/linux_persistent_llama_resilience_smoke.rs',
  'src-tauri/lu-core/examples/persistent_llama_resilience_smoke.rs',
  'src-tauri/lu-core/examples/persistent_llama_smoke.rs',
  'src-tauri/lu-core/tests/perf01_kv_cache_real.rs',
];

test('P5 deployed desktop remains untuned until exact pinned acceptance',()=>{
  assert.match(desktop,/tuning:None/);
  assert.match(runtime,/pub tuning: Option<TuningProfile>/);
  assert.match(runtime,/if let Some\(tuning\)=self\.config\.tuning\.as_ref\(\)/);
  assert.match(runtime,/command\.args\(tuning\.launch_args\(\)\)/);
  assert.match(runtime,/verify_engine\(&self\.canonical_executable/);
  for(const path of smokeFiles){
    assert.match(source(path),/tuning:\s*None/,path+' altered baseline');
  }
});

test('P5 experimental arguments are bounded enums and not model-supplied flags',()=>{
  for(const flag of ['--threads-batch','--batch-size','--ubatch-size',
    '--cache-type-k','--cache-type-v','--spec-type',
    '--spec-draft-model','--spec-draft-ngl','--spec-draft-n-max']){
    assert.ok(policy.includes(flag),'missing known llama flag: '+flag);
  }
  assert.match(policy,/self\.ubatch_size>self\.batch_size/);
  assert.match(policy,/self\.batch_size>context_tokens/);
  assert.match(policy,/self\.gpu_layers>99/);
  assert.match(policy,/Speculative::Disabled=>"TUNED_NO_SPECULATION"/);
  assert.doesNotMatch(desktop,/tuning:Some\(/);
  assert.doesNotMatch(policy,/Command::new\(/);
  assert.doesNotMatch(policy,/std::env::var\(/);
});

test('P5 draft requires independently admitted files and tokenizer metadata identity',()=>{
  assert.match(policy,/target\.recheck\(\)/);
  assert.match(policy,/draft\.recheck\(\)/);
  assert.match(policy,/target\.path\(\)==draft\.path\(\)/);
  assert.match(policy,/target_meta\.architecture==draft_meta\.architecture/);
  assert.match(policy,/tokenizer_metadata_sha256\(target\.path\(\)\)/);
  assert.match(policy,/target_tokens!=draft_tokens/);
  assert.match(policy,/if projector_present/);
  assert.match(runtime,/pinned_draft_file: Option<File>/);
  assert.match(runtime,/self\.pinned_draft_file = draft_file/);
  assert.match(runtime,/self\.pinned_draft_file = None/);
  assert.match(fingerprint,/key\.starts_with\("tokenizer\."\)/);
  assert.match(fingerprint,/key=="tokenizer\.ggml\.tokens"/);
  assert.match(fingerprint,/fn p5_draft_tokenizer_fingerprint_rejects_equal_labels_but_different_vocab/);
  assert.match(policy,/fn draft_model_preflight_checks_original_distinct_admitted_files_and_vocab/);
});

test('P5 paired benchmark is owner opt-in and never claims accepted real device tests',()=>{
  assert.match(comparison,/#\[ignore = "physical owner/);
  assert.match(comparison,/LU_P5_ENGINE_SHA256/);
  assert.match(comparison,/LU_P5_MODEL_SHA256/);
  assert.match(comparison,/LU_P5_API_KEY/);
  assert.match(comparison,/LU_P5_TRIALS/);
  assert.match(comparison,/trial%2==0/);
  assert.match(comparison,/ttft_p50_ms/);
  assert.match(comparison,/ttft_p95_ms/);
  assert.match(comparison,/exact_rubric_passes/);
  assert.match(comparison,/output_sha256/);
  assert.match(comparison,/"quality_equivalence_verified":false/);
  assert.match(comparison,/"installed_physical_acceptance":false/);
  assert.doesNotMatch(comparison,/assert_eq!\(.*output_sha256/);
});
