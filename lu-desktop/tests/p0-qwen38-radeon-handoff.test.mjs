import test from 'node:test';import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';
const s=readFileSync(new URL('../../scripts/p0-qwen38-radeon-physical.ps1',import.meta.url),'utf8');
test('Radeon handoff measures actual owned processes and never fabricates weight progress',()=>{
  assert.match(s,/PhysicalWindowsConfirmed/);
  assert.match(s,/llama-server-vulkan.exe/);
  assert.match(s,/Get-FileHash/);
  assert.match(s,/WorkingSet64/);
  assert.match(s,/PrivateMemorySize64/);
  assert.match(s,/GPU Process Memory/);
  assert.match(s,/ggufTensorLoadedPercent='NOT_MEASURED_BACKEND_TENSOR_TELEMETRY_REQUIRED'/);
  assert.match(s,/OPERATOR_REPORTED_PASS/);
  assert.match(s,/Nothing is uploaded/);
});