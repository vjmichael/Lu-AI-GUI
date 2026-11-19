#!/usr/bin/env node
/**
 * CORE-01A: exact Git-tree source, ownership and provider guard.
 * This is a source gate only, never native or installed acceptance.
 */
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
const BASE = '9b14dde7a01ad36208642be7536159ccb1f42e93';
const LOCK = 'source-tree.lock.json';
const MERGE03C = 'ecd7a899d9f4bd63c13a3e8cd2e4c50e8cae68c5';
const IDE_PARENT = 'db166431ccadd76ab7a6f3a4277c326bbf74c612';
const CORE_PARENT = 'cfb501751fd3f0509c8aeed802ad2890c37baa3d';
// Only paths present in the reviewed IDE parent or the exact integrated
// merge tree may enter the
// integrated scope. The original Core-only policy is unchanged.
export function reviewedAcceptanceFixture(path, blob) {
  const exact = new Map([
    ['ide/app/fixtures/test_dev01_native_confirmation_scope.py', 'acfcd77984123f0a53770826e2bc408258b74b83'],
    ['ide/packaging/b2_b4_installer_handoff.py', '568a7513560e4282b10fda79f7268c235ab36329'],
    ['ide/packaging/test_b2_b4_installer_handoff.py', 'd6332db3e7d822911e05da28dd481272d155374b'],
    ['ide/app/fixtures/test_wo9_review_control_policy.py', '99d92eb7f1ba4803322c41cf6f9cfa4a292a2224'],
    ['ide/app/fixtures/x11_openbox_readiness.sh', 'a6015822363cf0c47bd9a04368456ba963569f17'],
  ]);
  return exact.has(path) && exact.get(path) === blob;
}
export function reviewedIdePath(path, parentPaths) {
  return parentPaths.has(path) && (/^ide\//.test(path) || /^\.github\/workflows\/ide-[^/]+\.yml$/.test(path))
    && !/(?:^|\/)(?:[^/]*koboldcpp[^/]*|p7l[-_][^/]*|[^/]*p7l1[^/]*)/i.test(path);
}
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
function main(integrated = false) {
  const head = git('rev-parse', 'HEAD');
  if (!/^[a-f0-9]{40}$/.test(head)) fail('invalid HEAD');
  try { git('merge-base', '--is-ancestor', BASE, head); } catch { fail('reviewed baseline is not ancestor'); }
  if (git('status', '--porcelain', '--untracked-files=normal')) fail('checkout not clean');
  const actual = inventory('HEAD');
  verifyInventory(actual, JSON.parse(readFileSync(LOCK, 'utf8')));
  const old = new Map(inventory(BASE).map(x => [x.path, x]));
  const updated = actual.filter(item => old.get(item.path)?.blob !== item.blob);
  const deleted = [...old.keys()].filter(path => !actual.some(item => item.path === path));
  let idePaths = new Set();
  if (integrated) {
    try { git('merge-base', '--is-ancestor', MERGE03C, head); }
    catch { fail('reviewed integration is not ancestor'); }
    if (git('show', '-s', '--format=%P', MERGE03C) !== IDE_PARENT + ' ' + CORE_PARENT)
      fail('reviewed integration parent identity differs');
    const parentFiles = inventory(MERGE03C);
    const parentPaths = new Set(parentFiles.map(x => x.path));
    idePaths = new Set(parentFiles.filter(x => reviewedIdePath(x.path, parentPaths)).map(x => x.path));
    // 03D adds only an acceptance-only, three-phase cancellation witness.
    // New runtime paths are not implicitly authorized by the integration.
    idePaths.add('ide/supervisor/examples/support/cancel_history.rs');
    for (const p of ['apply_clangd20_windows_startup.py', 'Build-PinnedClangdWindows.ps1', 'test_clangd20_windows_startup.py'])
      idePaths.add('ide/llvm18_owner/' + p);
    for (const item of actual) {
      if (reviewedAcceptanceFixture(item.path, item.blob)) idePaths.add(item.path);
    }
    // A deleted originally reviewed IDE path is not authorized by merging.
    if ([...idePaths].some(path => !actual.some(x => x.path === path)))
      fail('reviewed IDE path missing');
  }
  const outOfScope = [...updated.map(x => x.path), ...deleted].filter(p => forbiddenPath(p) && !idePaths.has(p));
  if (outOfScope.length) fail('prohibited changed path(s): ' + outOfScope.join(', '));
  const sourceIssues = [];
  for (const item of updated) {
    if (!/(?:^|\/)(?:src|src-tauri|web)\/.*\.(?:rs|tsx?|jsx?|c|cc|cpp|h|hpp)$|(?:^|\/)(?:Cargo\.toml|package\.json|core\.proto|Cargo\.lock)$/i.test(item.path)) continue;
    const contents = git('show', 'HEAD:' + item.path);
    for (const problem of suspectSource(item.path, contents)) {
      if (idePaths.has(item.path) && problem === 'IDE source import') continue;
      sourceIssues.push(item.path + ': ' + problem);
    }
  }
  if (sourceIssues.length) fail(sourceIssues.join('; '));
  console.log(JSON.stringify({ gate: integrated ? 'MERGE03C_COMBINED_SOURCE_SCOPE_AND_IDENTITY' : 'CORE01A_SOURCE_SCOPE_AND_IDENTITY', head, baseline: BASE, tracked: actual.length, changed: updated.length, deleted: deleted.length, prohibited: 0, source_only: true, native_accepted: false, installed_accepted: false }));
}
if (process.argv[1] && /core01-verify-scope\.mjs$/.test(process.argv[1])) {
  try {
    if (process.argv.length > 3 || (process.argv[2] && process.argv[2] !== '--integrated')) fail('unknown scope option');
    main(process.argv[2] === '--integrated');
  } catch (e) { console.error(e.message); process.exitCode = 1; }
}
