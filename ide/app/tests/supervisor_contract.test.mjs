import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

test('D3 supervisor is isolated, denies OS admission and does not expose a Run command', () => {
  const core = read('../../supervisor/src/lib.rs');
  const desktop = read('../src-tauri/src/lib.rs');
  const host = read('../../host/src/lib.rs');
  const schemas = JSON.parse(read('../../host/src/capability_schemas.json'));
  assert.match(core, /pub fn production_worker_admission\(\)/);
  assert.match(core, /Err\(SupervisorError::NoContainment\)/);
  assert.match(core, /fn cancel_running_with\(/);
  assert.match(core, /fn reconcile_interrupted_with\(/);
  assert.match(core, /JobPhase::RecoveryRequired/);
  assert.match(core, /receipt_digest\(/);
  assert.match(core, /GRANT_TTL_MS: u64 = 60_000/);
  assert.match(core, /execution_verified: false/);
  assert.doesNotMatch(core, /std::process::Command|Command::new|\.spawn\(/);
  assert.doesNotMatch(desktop, /ide_run_job|ide_launch_worker|ide_execute_job/);
  assert.doesNotMatch(host, /pub mod execution_supervisor/);
  assert.ok(!Object.keys(schemas.operations).includes('job.run'));
  assert.ok(!Object.keys(schemas.operations).includes('job.execute'));
});
test('D3 production OS policy requires rights, quotas, and cleanup, not Job Object alone', () => {
  const core = read('../../supervisor/src/lib.rs');
  for(const name of ['restricted_identity','filesystem_access_limited',
    'network_access_limited','child_contained_before_resume',
    'process_tree_kill_verified','memory_quota','cpu_quota',
    'pid_quota','protected_journal_broker']) {
    assert.ok(core.includes(name), name);
  }
});


test('D5 worker is never reachable through desktop native request contracts', () => {
  const stage = read('../../supervisor/src/snapshot.rs');
  const linux = read('../../supervisor/src/linux_worker.rs');
  const cargo = read('../../supervisor/Cargo.toml');
  const native = read('../src-tauri/src/lib.rs');
  assert.match(stage, /pub fn publish_input/);
  assert.match(stage, /pub fn verify_input/);
  assert.match(stage, /MAX_TOTAL_BYTES: u64 = 512 \* 1024 \* 1024/);
  assert.match(stage, /UntrustedSource/);
  assert.match(linux, /fn attach_pid_in_pre_exec/);
  assert.match(linux, /cgroup\.kill/);
  assert.match(linux, /--unshare-all/);
  assert.match(linux, /--offline/);
  assert.match(linux, /--locked/);
  assert.match(linux, /fn scan_output_budget/);
  assert.match(cargo, /linux-worker = \[\]/);
  assert.doesNotMatch(native, /ide_run_job|ide_launch_worker|ide_execute_job/);
});
