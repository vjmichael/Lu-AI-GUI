#!/usr/bin/env node
/**
 * CORE-01A: exact Git-tree source, ownership and provider guard.
 * This is a source gate only, never native or installed acceptance.
 */
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
const BASE = '9b14dde7a01ad36208642be7536159ccb1f42e93';
const LOCK = 'source-tree.lock.json';
const fail = reason => { throw new Error('CORE01A_REFUSED: ' + reason); };
const git = (...args) => execFileSync('git', args, { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] }).trimEnd();
export function forbiddenPath(p) {
  return /^(?:ide\/|src-tauri\/lu-ide\/|lu-desktop\/src\/ide\/|lu-desktop\/src-tauri\/src\/ide_|\.github\/workflows\/[^/]*ide[^/]*|docs\/[^/]*IDE[-_])/i.test(p)
    || /(?:^|\/)(?:[^/]*koboldcpp[^/]*|p7l[-_][^/]*|[^/]*p7l1[^/]*)/i.test(p);
}
export function suspectSource(path, contents) {
  const executable = /(?:^|\/)(?:src|src-tauri|web)\/.*\.(?:rs|tsx?|jsx?|c|cc|cpp|h|hpp)$/i.test(path)
    || /(?:^|\/)(?:Cargo\.toml|package\.json|core\.proto|Cargo\.lock)$/i.test(path);
  if (!executable) return [];
  const hits = [];
  if (/kobold\s*cpp|koboldcpp|p7l[_-]|kobold_server/i.test(contents)) hits.push('excluded backend reference');
  if (/\b(?:ide::|lu_ide::|crate::ide::|mod\s+ide\s*;|path\s*=\s*["'][^"']*\/ide\/)/i.test(contents)) hits.push('IDE source import');
  return hits;
}
export function verifyInventory(actual, manifest) {
  if (manifest.schema !== 1 || !Array.isArray(manifest.files)) fail('invalid lock schema');
  if (manifest.trackedFileCount !== manifest.files.length) fail('incorrect lock count');
  if (actual.length !== manifest.files.length) fail('Git tree count differs from lock');
  for (let i = 0; i < actual.length; i++) {
    const entry = manifest.files[i], item = actual[i];
    if (!entry || entry.path !== item.path || entry.blob !== item.blob)
      fail('Git blob mismatch at ' + (item?.path || entry?.path || i));
  }
}
function inventory(ref) {
  const raw = git('ls-tree', '-r', '-z', ref).split('\0').filter(Boolean);
  return raw.map(item => {
    const tab = item.indexOf('\t');
    if (tab < 0) fail('invalid tree entry');
    const [mode, type, blob] = item.slice(0, tab).split(' ');
    const path = item.slice(tab + 1);
    if (type !== 'blob' || !/^[a-f0-9]{40}$/.test(blob) || !path || path.startsWith('/') || path.includes('..') || path.includes('\\')) fail('unsafe Git tree entry ' + path);
    return { path, blob, mode };
  }).filter(item => item.path !== LOCK).sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
}
function main() {
  const head = git('rev-parse', 'HEAD');
  if (!/^[a-f0-9]{40}$/.test(head)) fail('invalid HEAD');
  try { git('merge-base', '--is-ancestor', BASE, head); } catch { fail('reviewed baseline is not ancestor'); }
  if (git('status', '--porcelain', '--untracked-files=normal')) fail('checkout not clean');
  const actual = inventory('HEAD');
  verifyInventory(actual, JSON.parse(readFileSync(LOCK, 'utf8')));
  const old = new Map(inventory(BASE).map(x => [x.path, x]));
  const updated = actual.filter(item => old.get(item.path)?.blob !== item.blob);
  const deleted = [...old.keys()].filter(path => !actual.some(item => item.path === path));
  const outOfScope = [...updated.map(x => x.path), ...deleted].filter(forbiddenPath);
  if (outOfScope.length) fail('prohibited changed path(s): ' + outOfScope.join(', '));
  const sourceIssues = [];
  for (const item of updated) {
    if (!/(?:^|\/)(?:src|src-tauri|web)\/.*\.(?:rs|tsx?|jsx?|c|cc|cpp|h|hpp)$|(?:^|\/)(?:Cargo\.toml|package\.json|core\.proto|Cargo\.lock)$/i.test(item.path)) continue;
    const contents = git('show', 'HEAD:' + item.path);
    for (const problem of suspectSource(item.path, contents)) sourceIssues.push(item.path + ': ' + problem);
  }
  if (sourceIssues.length) fail(sourceIssues.join('; '));
  console.log(JSON.stringify({ gate: 'CORE01A_SOURCE_SCOPE_AND_IDENTITY', head, baseline: BASE, tracked: actual.length, changed: updated.length, deleted: deleted.length, prohibited: 0, source_only: true, native_accepted: false, installed_accepted: false }));
}
if (process.argv[1] && /core01-verify-scope\.mjs$/.test(process.argv[1])) {
  try { main(); } catch (e) { console.error(e.message); process.exitCode = 1; }
}
