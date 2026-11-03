#!/usr/bin/env node
// REPO-01: verify the exact checkout against a frozen Git blob inventory.
// The manifest itself is deliberately excluded to avoid a recursive digest.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
const root = process.cwd();
function git(...args) {
  return execFileSync('git', ['-C', root, ...args], {encoding:'utf8',maxBuffer:16*1024*1024}).trim();
}
const fail = message => { throw new Error('REPO01_SOURCE_MISMATCH: '+ message); };
try {
  const manifest = JSON.parse(readFileSync(join(root,'source-tree.lock.json'),'utf8'));
  if (manifest.schema !== 1 || !Array.isArray(manifest.files)) fail('invalid lock schema');
  const excluded = 'source-tree.lock.json';
  const tracked = git('ls-files','--cached','-z').split('\0').filter(Boolean)
    .filter(path => path !== excluded).sort();
  if (tracked.length !== manifest.files.length) fail('tracked file count '+tracked.length+' != '+manifest.files.length);
  if (manifest.trackedFileCount !== manifest.files.length) fail('lock count invalid');
  for (let i=0;i<tracked.length;i++) {
    const item = manifest.files[i];
    const path=tracked[i];
    if (item.path !== path || !/^[0-9a-f]{40}$/.test(item.blob)) fail('manifest path or hash at '+i);
    if (path.startsWith('/') || path.includes('..') || path.includes('\\')) fail('unsafe path');
    const blob = git('hash-object','--',path);
    if (blob !== item.blob) fail('changed or missing '+path);
    const staged = git('rev-parse',':'+path);
    if (staged !== item.blob) fail('staged mismatch '+path);
  }
  const head=git('rev-parse','HEAD');
  console.log('REPO01_SOURCE_PASS tracked='+tracked.length+' commit='+head);
} catch(e) { console.error(e.message);process.exitCode=1; }
