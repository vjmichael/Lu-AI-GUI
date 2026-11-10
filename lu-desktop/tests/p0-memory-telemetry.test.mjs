import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const native=readFileSync(new URL('../src-tauri/src/engine_memory.rs',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
test('memory measurements are process-owned native reads and not simulated progress',()=>{
  assert.match(host,/fn lu_load_memory_sample/);
  assert.match(host,/e.phase=="engine_spawned"/);
  assert.match(native,/GetProcessMemoryInfo/);
  assert.match(native,/PdhGetFormattedCounterArrayW/);
  assert.match(native,/GPU Process Memory/);
  assert.match(native,/VmRSS:/);
  assert.match(native,/verified_model_load_percent:None/);
  assert.match(ui,/invoke\('lu_load_memory_sample'\)/);
  assert.match(ui,/Tensor loading % unavailable/);
  assert.doesNotMatch(ui,/modelLoadSeconds/);
});
