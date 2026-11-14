import { readFileSync } from 'node:fs';
import test from 'node:test';
import assert from 'node:assert/strict';
import { forbiddenPath, suspectSource, verifyInventory } from './core01-verify-scope.mjs';
test('IDE source paths and build workflows cannot enter CORE-01', () => {
  for (const p of ['ide/worker.rs','src-tauri/lu-ide/src/lib.rs','lu-desktop/src-tauri/src/ide_worker.rs','.github/workflows/ide-native.yml','docs/LU_IDE_GATE.md']) assert.equal(forbiddenPath(p), true, p);
  assert.equal(forbiddenPath('src-tauri/lu-core/src/independent_outcome.rs'), false);
});
test('experimental provider additions are refused including nested paths', () => {
  for (const p of ['src-tauri/lu-core/src/p7_koboldcpp_backend.rs','docs/p7l-native.md','.github/workflows/lu-perf01-p7l1-native.yml','x/y/koboldcpp.toml']) assert.equal(forbiddenPath(p), true, p);
});
test('provider registration and dependency injection are refused even in allowed paths', () => {
  assert.ok(suspectSource('src-tauri/lu-core/src/lib.rs', 'mod koboldcpp;').length);
  assert.ok(suspectSource('lu-desktop/src-tauri/src/lib.rs', 'use lu_ide::worker;').length);
  assert.ok(suspectSource('src-tauri/lu-core/Cargo.toml', 'koboldcpp = "1"').length);
  assert.equal(suspectSource('src-tauri/lu-core/src/lib.rs', 'mod persistent_llama_backend;').length, 0);
});
test('docs can record exclusions without falsely adding a runtime provider', () => {
  assert.deepEqual(suspectSource('docs/LU_CORE01_SOURCE_CONVERGENCE_20261112.md', 'KoboldCpp excluded'), []);
});
test('manifest rejects extras and modified blobs, not just path prefixes', () => {
  const a=[{path:'a.rs',blob:'a'.repeat(40)}], m={schema:1,trackedFileCount:1,files:[{path:'a.rs',blob:'a'.repeat(40)}]};
  assert.doesNotThrow(()=>verifyInventory(a,m));
  assert.throws(()=>verifyInventory([...a,{path:'extra.rs',blob:'b'.repeat(40)}],m),/REFUSED/);
  assert.throws(()=>verifyInventory(a,{...m,files:[{path:'a.rs',blob:'b'.repeat(40)}]}),/REFUSED/);
});

test('PERF-01 warm model source retains the pressure branch outside comments', () => {
  const original = readFileSync(new URL('../src-tauri/lu-core/src/warm_model_residency.rs', import.meta.url), 'utf8');
  assert.ok(original.includes('if idle_for.is_some_and(|elapsed| elapsed >= Duration::from_secs(30))'));
  assert.ok(original.includes('return WarmDecision::EvictMeasuredPressure;'));
  assert.ok(!original.includes('\\n        if idle_for'), 'Literal escaped newlines must not swallow control flow');
});
