import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');

const matrix=read('../../.github/workflows/lu-perf01-p7l1-native.yml');
const comparisons=read('../../src-tauri/lu-core/tests/p7l_real_kobold_comparison.rs');
const adapter=read('../../src-tauri/lu-core/src/p7_koboldcpp_backend.rs');
const fixture=read('../../src-tauri/lu-core/tests/p7l_kobold_process_custody.rs');
const cargo=read('../../src-tauri/lu-core/Cargo.toml');
const desktop=read('../src-tauri/src/lib.rs');

test('P7L-1 native matrix is manually admitted and pins the exact SHA in both OS jobs',()=>{
  assert.match(matrix,/workflow_dispatch:/);
  assert.doesNotMatch(matrix,/\bpush:\s*\n/);
  assert.doesNotMatch(matrix,/\bpull_request:\s*\n/);
  assert.match(matrix,/\[a-f0-9\]\{40\}/);
  assert.match(matrix,/test "\$\(git rev-parse HEAD\)" = "\$EXPECTED_SHA"/);
  assert.match(matrix,/os: \[ubuntu-24\.04, windows-2022\]/);
  assert.match(matrix,/ref: \$\{\{ inputs\.exact_head \}\}/);
  assert.match(matrix,/--lib p7_kobold_custody::tests/);
  assert.match(matrix,/--test p7l_kobold_http_protocol/);
  assert.match(matrix,/--test p7l_kobold_process_custody/);
  assert.match(matrix,/--test p7l_real_kobold_comparison --no-run/);
  assert.doesNotMatch(matrix,/--ignored --nocapture/);
});
test('P7L-1 CPU-vs-CPU comparison refuses deceptive Vulkan speedup figures',()=>{
  assert.match(comparisons,/tuning:Some\(TuningProfile\{/);
  assert.match(comparisons,/gpu_layers:0,gpu_placement:None/);
  assert.match(comparisons,/Speculative::Disabled/);
  assert.match(comparisons,/"requestedGpuLayersOnWindows":0/);
  assert.match(comparisons,/"baselineRequestedBackend":"CPU"/);
  assert.match(comparisons,/"baselineCpuVsKoboldCpuEquivalenceProven":false/);
  assert.match(comparisons,/"generatedTokens":null/);
  assert.match(comparisons,/#\[ignore = "requires reviewed physical/);
});
test('P7L-1 mock still tests actual child custody and never promotes Kobold',()=>{
  assert.match(adapter,/job\.assign\(&child\)/);
  assert.match(adapter,/finish_after_reap\(reaped\)/);
  assert.match(adapter,/process_group\(0\)/);
  assert.match(adapter,/self\.journal\.mark_spawned/);
  assert.match(fixture,/original_process_is_authenticated_streamed_reaped_then_admissible_again/);
  assert.match(fixture,/malformed_original_sse_is_terminal_and_reaps_child/);
  assert.match(fixture,/durable_starting_state_survives_reopen_and_refuses_unverified_restart/);
  assert.match(cargo,/required-features = \["p7l-native-fixture"\]/);
  assert.match(desktop,/backend:"koboldcpp",engine:"KoboldCpp",packaged:false/);
  assert.doesNotMatch(desktop,/KoboldCppBackend::new/);
});
