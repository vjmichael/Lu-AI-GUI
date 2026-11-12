#!/usr/bin/env node
/**
 * CORE-01 source isolation gate.
 *
 * Compare the working HEAD with the immutable, owner-reviewed P0 Core base.
 * A prohibition on changing files does not imply historical paths were
 * removed from the old source tree. Only the convergence delta is scoped.
 *
 * Run: node scripts/core01-verify-scope.mjs
 */
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const baseline = '9b14dde7a01ad36208642be7536159ccb1f42e93';
const git = (...args) => execFileSync('git', args, {
  cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'],
});
const deny = [
  /^(?:ide\/|src-tauri\/lu-ide\/|lu-desktop\/src-tauri\/src\/ide_|lu-desktop\/src\/ide\/)/i,
  /^(?:\.github\/workflows\/.*ide.*|docs\/.*IDE[-_].*|.*\/.*(?:lldb|debugger|appcontainer).*ide.*)$/i,
  /(?:^|\/)(?:p7l[_-]|.*koboldcpp|lu-perf01-p7l1-native\.yml)/i,
];
const head = git('rev-parse', 'HEAD').trim();
if (!/^[a-f0-9]{40}$/.test(head)) {
  throw Error('CORE-01: invalid HEAD identity');
}
const ancestry = git('merge-base', '--is-ancestor', baseline, head);
if (ancestry !== '') {
  throw Error('CORE-01: unexpected ancestry output');
}
const paths = git('diff', '--name-only', '-z', baseline, head)
  .split('\0').filter(Boolean);
const forbidden = paths.filter(path => deny.some(pattern => pattern.test(path)));
if (forbidden.length) {
  console.error('CORE-01 blocked changes outside Lu Core scope:');
  for (const path of forbidden) console.error(path);
  process.exitCode = 1;
} else {
  console.log(JSON.stringify({
    gate: 'CORE01_SOURCE_DELTA_SCOPE',
    baseline, head, changed_paths: paths.length,
    ide_or_koboldcpp_changed: 0,
    scope_only: true, native_accepted: false,
    installed_accepted: false
  }));
}
