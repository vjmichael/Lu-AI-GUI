import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const workflow=readFileSync(new URL('../.github/workflows/lu-cp10-r3-windows-nsis.yml',import.meta.url),'utf8').replace(/\r\n?/g,'\n');
const baseline=readFileSync(new URL('../.github/workflows/lu-core-c6-hosted-dual-nsis.yml',import.meta.url),'utf8').replace(/\r\n?/g,'\n');
const native=readFileSync(new URL('../.github/workflows/lu-cp12br7r1-windows-collaboration.yml',import.meta.url),'utf8').replace(/\r\n?/g,'\n');

test('CP-10-R3 qualifies one exact source SHA and does not open broad CI triggers',()=>{
  assert.match(workflow,/^  push:\n    branches: \['lu\/cp10-r3-windows-nsis-20261120'\]\n    paths: \['\.github\/workflows\/lu-cp10-r3-windows-nsis\.yml'\]/m);
  assert.match(workflow,/github\.event\.head_commit\.message == 'CP-10-R3 source-locked Windows NSIS qualification'/);
  assert.doesNotMatch(workflow,/^\s+(pull_request|pull_request_target|schedule|workflow_dispatch):/m);
  for(const token of ['github.sha','git rev-parse HEAD','verify-source-tree.mjs','persist-credentials: false','cancel-in-progress: false','runs-on: windows-2025']) assert.ok(workflow.includes(token),token);
  assert.doesNotMatch(workflow,/continue-on-error:\s*true|curl\s+.+\|\s*(bash|pwsh)|-SkipValidation/);
});

test('R3 preserves native protocol, explicit consent and Lu single user interface before installer',()=>{
  for(const token of ['cp12b-local-diagnostic-exchange.test.mjs','cp07-08-ux-r2-no-modes.test.mjs','cargo test --locked --manifest-path scripts/cp12br2-windows-transport-check/Cargo.toml --features windows_dx_eval','cargo test --locked --manifest-path scripts/cp12br2-windows-transport-check/Cargo.toml -- --test-threads=1','npm run build','npm run check']) assert.ok(workflow.includes(token),token);
  assert.match(workflow,/needs: source-and-native-windows/);
  assert.match(workflow,/needs\.source-and-native-windows\.result == 'success'/);
  assert.match(workflow,/uses: \.\/\.github\/workflows\/lu-core-c6-hosted-dual-nsis\.yml/);
  assert.match(native,/no installed GUI, independent GGUF inference/i);
});

test('original C6 hosted NSIS remains reviewed manual-only, genuine source-built, installed and data preserving',()=>{
  const events=baseline.split('\non:\n')[1].split('\npermissions:')[0];
  assert.match(events,/workflow_dispatch:/);assert.match(events,/workflow_call:/);
  assert.doesNotMatch(events,/push:|pull_request:|schedule:/);
  for(const token of ['scripts/build-current-windows-nsis.ps1','scripts/m3-windows-nsis-cycle.ps1','choco install nsis --version=3.11.0','choco install vulkan-sdk --version=1.4.350','ExpectedVulkanSha256','PASS_INSTALLED_UNIFIED_TABS','PASS_UNTOUCHED_APPDATA_SENTINEL','PASS_BINARIES_ABSENT','actions/upload-artifact@v4']) assert.ok(baseline.includes(token),token);
  assert.doesNotMatch(baseline,/continue-on-error:\s*true/);
});

test('hosted NSIS and physical Radeon acceptance remain separate gates',()=>{
  assert.match(baseline,/NOT physical R9700/);
  assert.match(baseline,/Browser package intentionally absent/);
  assert.match(baseline,/No model tool grant or physical AMD test asserted|physical R9700, Qwen 27B, MMProj and real tool approval NOT TESTED/i);
  assert.doesNotMatch(workflow,/PhysicalWindowsConfirmed|Get-CimInstance Win32_VideoController|GGUF.*download/i);
});
