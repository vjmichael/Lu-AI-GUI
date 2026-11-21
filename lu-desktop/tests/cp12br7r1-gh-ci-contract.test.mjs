import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
// Git's text checkout may use CRLF on Windows; assert semantic YAML lines, not platform-specific bytes.
const source=readFileSync(new URL('../../.github/workflows/lu-cp12br7r1-windows-collaboration.yml', import.meta.url),'utf8').replace(/\r\n?/g, '\n');
test('R7/CP-10 CI is exact SHA scoped, manual-only, and Windows-native',()=>{
  for(const token of ['workflow_dispatch:','ref: ${{ github.sha }}','git rev-parse HEAD','GITHUB_SHA','verify-source-tree.mjs','runs-on: windows-2025','contents: read','persist-credentials: false']) assert.ok(source.includes(token),token);
  assert.match(source,/^on:\n  workflow_dispatch:/m);
  assert.doesNotMatch(source,/^  (?:push|pull_request|pull_request_target|schedule):/m);
  assert.ok(source.includes("if: ${{ github.event_name == 'workflow_dispatch' }}"));
  assert.ok(source.includes("if: ${{ github.event_name == 'workflow_dispatch' && inputs.include_desktop_source_check }}"));
  assert.ok(source.includes("needs: [windows-native-protocol, windows-desktop-source-check]"));
  assert.ok(source.includes("github.ref == 'refs/heads/lu/cp10-consolidated-ux-20261120'"));
  assert.ok(source.includes("needs.windows-native-protocol.result == 'success'"));
  assert.ok(source.includes("needs.windows-desktop-source-check.result == 'success'"));
  assert.doesNotMatch(source,/github\.event\.head_commit\.message/);
  assert.doesNotMatch(source,/^\s+schedule:/m);
});
test('R7-R1 CI executes actual Windows transport and default-off with strict status',()=>{
  assert.match(source,/cargo test --locked --manifest-path scripts\/cp12br2-windows-transport-check\/Cargo\.toml --features windows_dx_eval/);
  assert.match(source,/cargo test --locked --manifest-path scripts\/cp12br2-windows-transport-check\/Cargo\.toml -- --test-threads=1/);
  assert.match(source,/throw 'Windows native collaboration test failed'/);
  assert.match(source,/throw 'Default-off Windows transport regression failed'/);
});
test('R7-R1 CI does not claim physical acceptance or download GGUF',()=>{
  assert.match(source,/not an installed GUI acceptance/);
  assert.match(source,/no installed GUI, independent GGUF inference/i);
  assert.doesNotMatch(source,/huggingface\.co|Set-Clipboard|Get-Clipboard|Stop-Process|net user/i);
  assert.match(source,/PowerShell syntax errors/);
});
