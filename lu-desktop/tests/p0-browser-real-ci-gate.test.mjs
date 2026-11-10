import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const workflow=readFileSync(new URL('../../.github/workflows/lu-browser-research-acceptance.yml',import.meta.url),'utf8');
const runtime=readFileSync(new URL('../src-tauri/src/native_browser_host.rs',import.meta.url),'utf8');
test('real candidate gated to exact reviewed source SHA and no implicit production permission',()=>{
  assert.match(workflow,/LU_P0_BROWSER_NATIVE_SHA=/);
  assert.match(workflow,/github\.event\.pull_request\.head\.sha/);
  assert.match(workflow,/!contains\(github\.event\.changes\.body\.from/);
  assert.match(workflow,/assemble-browser-ci-candidate\.mjs/);
  assert.match(workflow,/browser-ci-executable\.test\.mjs/);
  assert.match(workflow,/xvfb-run -a/);
  assert.match(workflow,/compression-level: 0/);
  assert.match(workflow,/path: out\/lu-browser-candidate\//);
  assert.match(workflow,/m\.status!=="candidate"/);
  assert.match(runtime,/manifest\.status!="reviewed"/);
  assert.match(runtime,/package_verified/);
  assert.doesNotMatch(workflow,/automatic.*status.*reviewed/i);
});
