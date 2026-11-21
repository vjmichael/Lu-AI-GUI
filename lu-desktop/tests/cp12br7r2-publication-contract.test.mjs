import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
// Exact branch gates are verified against the current source-locked CI candidate;
// normalize Windows CRLF only in the in-memory assertion input.
const wf=readFileSync(new URL('../../.github/workflows/lu-cp12br7r1-windows-collaboration.yml',import.meta.url),'utf8').replace(/\r\n?/g,'\n');
const pub=readFileSync(new URL('../../scripts/cp12br7r2-publish-exact-source.ps1',import.meta.url),'utf8');
const label='CP-12B-R7-R2 source-locked Windows CI handoff';
test('CP-10 consolidated manual run requires original Windows success and source lock',()=>{
  assert.match(wf,/^on:\n  workflow_dispatch:/m);
  assert.doesNotMatch(wf,/^  (?:push|pull_request|pull_request_target|schedule):/m);
  assert.ok(wf.includes("if: ${{ github.event_name == 'workflow_dispatch' }}"));
  assert.ok(wf.includes("if: ${{ github.event_name == 'workflow_dispatch' && inputs.include_desktop_source_check }}"));
  assert.ok(wf.includes("needs: [windows-native-protocol, windows-desktop-source-check]"));
  assert.ok(wf.includes("github.ref == 'refs/heads/lu/cp10-consolidated-ux-20261120'"));
  assert.ok(wf.includes("needs.windows-native-protocol.result == 'success'"));
  assert.ok(wf.includes("needs.windows-desktop-source-check.result == 'success'"));
  assert.doesNotMatch(wf,/github\.event\.head_commit\.message/);
  assert.equal((wf.match(/github\.event\.head_commit\.message/g)||[]).length,0);
  assert.doesNotMatch(wf,/^\s+schedule:|^\s+pull_request:|^\s+pull_request_target:/m);
  assert.match(wf,/ref: \$\{\{ github\.sha \}\}/);
  assert.match(wf,/if \(\(git rev-parse HEAD\)/);
  assert.match(wf,/persist-credentials: false/);
  assert.match(wf,/concurrency:\n  group: lu-cp12br7r1-\$\{\{ github\.sha \}\}/);
});
test('R7-R2 publication refuses mismatch, occupied ref, force and remote check bypass',()=>{
  for (const token of [
    'Bundle checksum mismatch','Exact local commit or Git tree mismatch',
    'verify-source-tree.mjs','Remote branch unexpectedly exists',
    'remote readback', 'REMOTE_SOURCE_VERIFIED',
    'REMOTE_PUBLICATION=NOT_ATTEMPTED', 'HOSTED_CI=NOT_YET_VERIFIED',
    'ls-remote --heads','--porcelain', '-Publish'
  ]) assert.ok(pub.includes(token),token);
  assert.doesNotMatch(pub,/--force|force-with-lease|credential\.helper|\$env:GITHUB_TOKEN|gh auth login/i);
  assert.match(pub,/if \(-not \$Publish\)/);
  assert.match(pub,/function Invoke-GitStrict/);
  assert.match(pub,/Get-Command git -CommandType Application/);
  assert.doesNotMatch(pub,/function Git\(/);
});
