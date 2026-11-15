import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const path = name => fileURLToPath(new URL(name, import.meta.url));
const generator = path('../scripts/make-icon.mjs');
const icon = path('../src-tauri/icons/icon.ico');
const png = path('../src-tauri/icons/icon.png');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');

test('reviewed first-party Windows icon and Linux PNG are checked in and reproducible', t => {
  const temp = mkdtempSync(join(tmpdir(), 'lu-ide-icon-contract-'));
  t.after(() => rmSync(temp, { recursive: true, force: true }));
  mkdirSync(join(temp, 'scripts'));
  copyFileSync(generator, join(temp, 'scripts', 'make-icon.mjs'));
  const run = spawnSync(process.execPath, [join(temp, 'scripts', 'make-icon.mjs')],
    { cwd: temp, encoding: 'utf8', timeout: 10_000 });
  assert.equal(run.status, 0, 'first-party icon generator must succeed: ' + run.stderr);
  for (const [tracked, name, expected] of [
    [icon, 'icon.ico', 'c7cffbab57aac9ca8ccf2182a1d1130d401448c7e8be3849a4228f049e9e8391'],
    [png, 'icon.png', '7849faf6dd0e2aa0417cee24b9f09c841ae8e4374f1486ccc59418c6784d6327'],
  ]) {
    const committed = readFileSync(tracked);
    const regenerated = readFileSync(join(temp, 'src-tauri', 'icons', name));
    assert.deepEqual(committed, regenerated, name + ' changed from reviewed generator');
    assert.equal(sha(committed), expected);
  }
  const ico = readFileSync(icon);
  assert.equal(ico.readUInt16LE(2), 1);
  assert.equal(ico.readUInt16LE(4), 1);
  assert.equal(ico.readUInt32LE(14), ico.length - 22);
  assert.equal(ico.readUInt32LE(22), 40);
  assert.equal(ico.readInt32LE(26), 64);
  assert.equal(ico.readInt32LE(30), 128);
  assert.equal(readFileSync(png).subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
});

test('Windows owner and integrated installed gates use tracked icon not CI-only shim', () => {
  const owner = readFileSync(path('../../llvm18_owner/Test-PinnedOwnerWindows.ps1'), 'utf8');
  const grouped = readFileSync(path('../../../.github/workflows/ide-group6f-integrated-native.yml'), 'utf8');
  assert.match(owner, /verify_reviewed_ide_icons\.py --repo-root \./);
  assert.match(owner, /cargo check --manifest-path ide\/app\/src-tauri\/Cargo\.toml --features dev046d-native-dialog/);
  assert.equal((grouped.match(/verify_reviewed_ide_icons\.py --repo-root \./g) || []).length, 2);
  assert.doesNotMatch(grouped, /make_g04d_ci_icon\.py/);
  assert.match(grouped, /Build real unsigned NSIS desktop from current source/);
  assert.match(grouped, /dev01-modern-real-installed-windows/);
});
