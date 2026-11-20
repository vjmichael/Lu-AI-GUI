import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const wf=readFileSync(new URL('../../.github/workflows/lu-cp12br7r1-windows-collaboration.yml',import.meta.url),'utf8');
const pub=readFileSync(new URL('../../scripts/cp12br7r2-publish-exact-source.ps1',import.meta.url),'utf8');
const label='CP-12B-R7-R2 source-locked Windows CI handoff';
test('R7-R2 first-push event is narrow and gated by one named commit',()=>{
  assert.match(wf,/^  push:\n    branches: \['lu\/cp12b-r7-windows-stage-eval'\]\n    paths: \['\.github\/workflows\/lu-cp12br7r1-windows-collaboration\.yml'\]/m);
  assert.match(wf,/github\.event\.head_commit\.message == 'CP-12B-R7-R2 source-locked Windows CI handoff'/);
  assert.equal((wf.match(/github\.event\.head_commit\.message/g)||[]).length,2);
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
