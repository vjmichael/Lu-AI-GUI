import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const core=readFileSync(new URL('../../src-tauri/lu-core/src/p6_compute_fabric.rs',import.meta.url),'utf8');
const tuning=readFileSync(new URL('../../src-tauri/lu-core/src/inference_tuning.rs',import.meta.url),'utf8');
const backend=readFileSync(new URL('../../src-tauri/lu-core/src/persistent_llama_backend.rs',import.meta.url),'utf8');
const desktop=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const view=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const superv=readFileSync(new URL('../../src-tauri/lu-core/src/inference_supervisor.rs',import.meta.url),'utf8');

test('P6 installation keeps original offline single-model path active',()=>{
  assert.match(desktop,/p6_local_gpu:lu_core::p6_compute_fabric::LocalGpuCoordinator/);
  assert.match(desktop,/p6_remote_nodes:lu_core::p6_compute_fabric::OptionalComputeNodes/);
  assert.match(desktop,/tuning:None/);
  assert.match(desktop,/p6_remote_dispatch_enabled:false/);
  assert.doesNotMatch(desktop,/admit_remote_request\(/);
  assert.match(superv,/current: Mutex<Option<Arc<Session<B>>>>/);
});
test('P6 GPU placement cannot bypass fresh exact-pinned device+VRAM evidence',()=>{
  assert.match(core,/pub const MAX_LOCAL_DEVICES:usize=2/);
  assert.match(core,/pub const MAX_OBSERVATION_AGE_SECS:u64=5/);
  assert.match(core,/native_memory_probe_verified:bool/);
  assert.match(core,/state\.leases\.len\(\)>=MAX_LOCAL_RESERVATIONS/);
  assert.match(core,/VRAM_HEADROOM_MIB/);
  assert.match(core,/pub fn reserve_best_independent\(/);
  assert.match(core,/pub fn release_verified\(/);
  assert.match(core,/ProcessCustodyUncertain/);
  assert.match(core,/Weak<Mutex<GpuState>>/);
  assert.match(core,/pub fn verify_engine\(&self,sha:\[u8;32\]\)/);
  assert.match(backend,/P6 GPU reservation revoked before launch/);
  assert.match(backend,/P6 placement belongs to a different engine/);
});
test('P6 only host-pinned --device, layer split and independent placement flags',()=>{
  assert.match(core,/--device/);
  assert.match(core,/--split-mode/);
  assert.match(core,/--tensor-split/);
  assert.match(core,/--split-mode".into\(\),"none".into\(\)/);
  assert.match(core,/ComputeMode::LayerSplit/);
  assert.match(core,/ComputeMode::IndependentInstance/);
  assert.match(tuning,/gpu_placement:Option<ApprovedLocalGpuPlacement>/);
  assert.match(tuning,/values\.extend\(local\.flags\(\)\)/);
  assert.match(backend,/command\.args\(tuning\.launch_args\(\)\)/);
});
test('P6 independent supervisor selection cannot auto-retry model/tool effects',()=>{
  assert.match(core,/pub struct LocalIndependentFleet/);
  assert.match(core,/lane\.supervisor\.status\(\)/);
  assert.match(core,/EnginePhase::Ready/);
  assert.match(core,/pub fn choose_ready\(/);
  assert.match(core,/fn two_original_supervisors_choose_separate_resident_instances/);
  assert.doesNotMatch(core,/\.generate\(.*prompt/);
  assert.doesNotMatch(core,/std::net::TcpStream/);
});
test('P6 optional remote node registry never sends text without mTLS acceptance',()=>{
  assert.match(core,/pinned_tls_cert_sha256:\[u8;32\]/);
  assert.match(core,/pinned_engine_sha256:\[u8;32\]/);
  assert.match(core,/pinned_model_sha256:\[u8;32\]/);
  assert.match(core,/operator_approved:bool/);
  assert.match(core,/valid_remote_hostname/);
  assert.match(core,/Err\(GpuScheduleError::RemoteTransportDisabled\)/);
  assert.doesNotMatch(core,/reqwest::/);
  assert.doesNotMatch(core,/HTTP\/1\.1/);
  assert.match(view,/p6RemoteDispatchEnabled/);
  assert.match(view,/No prompts, tools, images or source data are sent/);
});
