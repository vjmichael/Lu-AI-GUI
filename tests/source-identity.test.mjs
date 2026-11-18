import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync, unlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';

const script = resolve(import.meta.dirname, '../scripts/source-identity.mjs');
function fixture(fn) {
  const dir = mkdtempSync(join(tmpdir(), 'lu-source-'));
  try {
    execFileSync('git', ['init', '-q', dir]);
    writeFileSync(join(dir, 'a.txt'), 'alpha');
    writeFileSync(join(dir, 'b.txt'), 'beta');
    execFileSync('git', ['-C', dir, 'add', 'a.txt', 'b.txt']);
    const run = op => spawnSync(process.execPath, [script, op, 'source-identity.json'], { cwd: dir, encoding: 'utf8' });
    fn({ dir, run });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
test('matching inventory passes', () => fixture(({ run }) => {
  assert.equal(run('generate').status, 0);
  assert.equal(run('verify').status, 0);
}));
test('modified source fails', () => fixture(({ dir, run }) => {
  run('generate'); writeFileSync(join(dir, 'a.txt'), 'tamper');
  assert.equal(run('verify').status, 1);
}));
test('deleted source fails', () => fixture(({ dir, run }) => {
  run('generate'); unlinkSync(join(dir, 'b.txt'));
  assert.equal(run('verify').status, 1);
}));
test('unexpected tracked source fails', () => fixture(({ dir, run }) => {
  run('generate'); writeFileSync(join(dir, 'c.txt'), 'new');
  execFileSync('git', ['-C', dir, 'add', 'c.txt']);
  assert.equal(run('verify').status, 1);
}));
test('manifest digest alteration fails', () => fixture(({ dir, run }) => {
  run('generate'); const p = join(dir, 'source-identity.json');
  const m = JSON.parse(readFileSync(p)); m.inventorySha256 = '0'.repeat(64);
  writeFileSync(p, JSON.stringify(m));
  assert.equal(run('verify').status, 1);
}));
test('duplicate manifest entry fails', () => fixture(({ dir, run }) => {
  run('generate'); const p = join(dir, 'source-identity.json');
  const m = JSON.parse(readFileSync(p)); m.files[1] = m.files[0];
  writeFileSync(p, JSON.stringify(m));
  assert.equal(run('verify').status, 1);
}));
