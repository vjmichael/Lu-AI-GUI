#!/usr/bin/env node
// Lu REPO-01 tracked-source identity gate. Run from repository root.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, relative, isAbsolute } from 'node:path';

const [mode, manifestArg = 'source-identity.json'] = process.argv.slice(2);
const root = process.env.LU_SOURCE_ROOT ? resolve(process.env.LU_SOURCE_ROOT) : process.cwd();
const manifestFile = resolve(root, manifestArg);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const canonical = value => {
  if (typeof value !== 'string' || !value || value.includes('\\') || value.includes('\0') || isAbsolute(value) || value.split('/').some(s => !s || s === '.' || s === '..')) {
    throw new Error(`unsafe source path: ${JSON.stringify(value)}`);
  }
  return value;
};
const manifestPath = canonical(relative(root, manifestFile).replaceAll('\\', '/'));
function gitFiles() {
  const raw = execFileSync('git', ['-C', root, 'ls-files', '-z', '--cached'], { encoding: 'buffer', maxBuffer: 64 * 1024 * 1024 });
  const paths = raw.toString('utf8').split('\0').filter(Boolean).map(canonical).filter(p => p !== manifestPath).sort();
  if (new Set(paths).size !== paths.length) throw new Error('duplicate git-tracked paths');
  return paths;
}
function inventory() {
  return gitFiles().map(path => ({ path, sha256: digest(readFileSync(resolve(root, path))) }));
}
function inventoryDigest(files) {
  return digest(Buffer.from(files.map(f => `${f.path}\0${f.sha256}\n`).join(''), 'utf8'));
}
try {
  if (!['generate', 'verify'].includes(mode)) throw new Error('usage: source-identity.mjs generate|verify [manifest-file]');
  if (mode === 'generate') {
    const files = inventory();
    const obj = { schema: 1, trackedFileCount: files.length, inventorySha256: inventoryDigest(files), files };
    writeFileSync(manifestFile, JSON.stringify(obj, null, 2) + '\n');
    console.log(`generated ${files.length} files; inventory ${obj.inventorySha256}`);
  } else {
    const actual = inventory();
    const expected = JSON.parse(readFileSync(manifestFile, 'utf8'));
    if (expected.schema !== 1 || !Number.isSafeInteger(expected.trackedFileCount) || !Array.isArray(expected.files)) throw new Error('invalid manifest schema');
    if (expected.trackedFileCount !== expected.files.length) throw new Error('manifest declared count mismatch');
    let prev = '';
    for (const f of expected.files) {
      canonical(f.path);
      if (f.path <= prev || !/^[a-f0-9]{64}$/.test(f.sha256)) throw new Error('manifest noncanonical, duplicate, or invalid hash');
      prev = f.path;
    }
    if (expected.inventorySha256 !== inventoryDigest(expected.files)) throw new Error('manifest inventory digest mismatch');
    if (actual.length !== expected.files.length) throw new Error(`tracked file count mismatch: actual=${actual.length}, expected=${expected.files.length}`);
    for (let i = 0; i < actual.length; i++) {
      if (actual[i].path !== expected.files[i].path || actual[i].sha256 !== expected.files[i].sha256) {
        throw new Error(`source mismatch at index ${i}: actual=${actual[i].path}, expected=${expected.files[i].path}`);
      }
    }
    console.log(`PASS: ${actual.length} tracked files; inventory ${expected.inventorySha256}`);
  }
} catch (error) {
  console.error(`SOURCE_IDENTITY_FAIL: ${error.message}`);
  process.exitCode = 1;
}
