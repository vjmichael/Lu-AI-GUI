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
// R14 is a separate integrated-source-only disposition for the 21 exact
// later acceptance paths rejected by CORE-01A on a596d0b. It does not
// change the historical five fixture pins, Core-only policy, reviewed
// parent identity, source scanning, or any native/installed gate.
// The vcpkg fixture is the R11 formatting-only postimage, not its
// unformatted published preimage. Any later source edit needs review.
const R14_ACCEPTANCE_PINS = new Map([
  ['.github/workflows/ide-dev02-technical-installed-lsp.yml', 'e4dbd01bf09cf77ca6b69274df85f4c4b26fd50c'],
  ['ide/app/fixtures/Run-DEV02TechnicalGtkSession.sh', '8a0f37bc60a394d756ce9806ac75d014b23a2e57'],
  ['ide/app/fixtures/Test-DEV02TechnicalInstalledDeb.sh', 'b485652458b6c75c916aaa2bfe5072d96366283a'],
  ['ide/app/fixtures/Test-DEV02TechnicalInstalledNsis.ps1', '9a9bd966abbeb4908394a516b34a416f4224fce5'],
  ['ide/app/fixtures/dev02_technical_installed_linux.py', '459b6ccee788928e49398a69f89851741be30afd'],
  ['ide/app/fixtures/test_dev01_native_confirmation_scope.py', '6d5258f6ebc6e97c1a54e6b4bc49b3f448a6795b'],
  ['ide/app/src-tauri/tauri.dev02.technical-only.conf.json', '31010228b7fac0cd8d0dbf08df074c8aafb94124'],
  ['ide/packaging/ide-05b-r3-outside-cohort.json', '90e56a2c0769f68ac7a647f965c35e5efd294d71'],
  ['ide/packaging/p06-observer-cohort.json', 'a11faff282ad98d435adfb13f55ec74f8d2544cc'],
  ['ide/packaging/test_ide_05b_r3_workflow_dispositions.py', 'd8dccd6cbaa34fb8666c8ed58cf3756b15f4f3b7'],
  ['ide/packaging/test_p06_technical_observation.py', 'f6718d7fd6c91a1bd901296652be95896a7395df'],
  ['ide/packaging/test_verify_technical_lsp_install.py', '2fcb753a04ae0523bfe11d7629be21471474e9db'],
  ['ide/packaging/verify_technical_lsp_install.py', '0e8071c51d3918a3884d111dcb44f3e8d5334f5d'],
  ['ide/supervisor/examples/dev03_linux_runtime_acceptance.rs', '1ae254bb5e1915c3db295b09cf59d81870525a81'],
  ['ide/supervisor/examples/ide08_native_vcpkg_acceptance.rs', 'eca321d18d23fcfa6d412a73a18eb831da435a9c'],
  ['ide/supervisor/examples/wo6_linux_interrupted_recovery.rs', '08a615bb342377786af23e6f98dfd2d48f6da9e8'],
  ['ide/supervisor/examples/wo6_linux_rustdoc_admission.rs', '0a0f42afe6a639058858de1beb38071845e1eb69'],
  ['ide/supervisor/fixtures/debug/dap_launch_contract.py', 'c0fee68885eee5a04ab2098332e250b0152314fd'],
  ['ide/supervisor/fixtures/debug/test_dap_launch_contract.py', '0713ca357d7b50217682ec4a6a30abd600ff1348'],
  ['ide/supervisor/fixtures/debug/test_dap_native_source_contract.py', '971035a8011369ba581c0f95832dc0ddc7af8883'],
  ['ide/tools/tests/test_provision_cmake_candidate.py', 'b59f7724e3dfaf41d8f98ebd91103fe87d28a70a'],
]);
export function reviewedR14AcceptanceFixture(path, blob, mode) {
  return mode === '100644' && R14_ACCEPTANCE_PINS.has(path)
    && R14_ACCEPTANCE_PINS.get(path) === blob;
}

/**
 * R4 PRECORE reviewed IDE-only source additions and separately changed two
 * R14 technical diagnostics. These are exact immutable Git blob pins, not a
 * Core runtime import, blanket IDE scope exemption, worker authorization or
 * signed/installed acceptance. The Core-only gate is unchanged.
 */
const R4_PRECORE_SOURCE_PINS = new Map([
  ['ide/app/src-tauri/src/native_core_health_menu.rs', '9ae00e5723d343ca4091dd1b99a3b726316e5f68'],
  ['ide/app/tests/native_core_health_menu.test.mjs', '58d25f3a83782bcf42d6cb29107a58cad6c8334f'],
  ['ide/docs/IDE_CORE_IPC_01B_NATIVE_CUSTODY.md', '52bf0190dddab4078743151aa3e523066c1110dd'],
  ['ide/docs/IDE_CORE_IPC_01B_R3_ISSUER_HANDOFF.md', '15dbdecd0569085d405166b17d15576c4983066f'],
  ['ide/docs/IDE_CORE_IPC_01B_R4_UPSTREAM_CORE_AUDIT.md', '08e7a355b5939c15439c0fab12ed20bb3bcd37fd'],
  ['ide/docs/IDE_CORE_IPC_01B_R5_CURRENT_CORE_CONTRACT.md', 'e8c0c45e5d119e6689fee0a01e825b8d0d3d3f75'],
  ['ide/docs/IDE_PRECORE_CLOSEOUT_R3_SIGNED_WINDOWS_ADMISSION.md', '92e8c11abfa250793cf282d1cdc9c939a3080b84'],
  ['ide/host/src/core_ipc_host.rs', '8a03da3466c5946a8ae7da51a349c7c36f065bbf'],
  ['ide/host/src/core_ipc_issuer.rs', '405669503461333f1e75f1d227f79f02f962e15f'],
  ['ide/host/src/core_ipc_lifecycle.rs', '6bb199509254c239322777dd9c9e9dd54df54acb'],
  ['ide/host/src/core_ipc_native_transport.rs', 'c3ac18fc3c34e4643c3ff8f9be8ada06d67e4674'],
  ['ide/host/src/core_ipc_readonly.rs', 'c6e84eb1bfdefa81c5020c9331f46422c17f36d0'],
  ['ide/host/src/core_ipc_windows_contract.rs', '717f51c554324501a1d8cacf9f2507501d317844'],
  ['ide/host/src/installed_core_health.rs', '1ba66ce4f44c8bba7e8192ac9397eb8f10d3262c'],
  ['ide/packaging/Test-IdePrecoreR3WindowsCandidate.ps1', '8a4d85b81c809ca8ee89b500a0cd9cbba83c2d28'],
  ['ide/packaging/ide-05b-r3-outside-cohort.json', 'd603439db9d17a433a0fc694dda409e63db154d3'],
  ['ide/packaging/ide-precore-r2-ipc-lock-successor.json', 'fa2c49d5d9149522ec6fc2e95fae98582d4aa729'],
  ['ide/packaging/ide-precore-r3-admission-register.json', '402228fe180287f0c74eb0e6bf4df2bad27a9904'],
  ['ide/packaging/test_ide_05b_r3_workflow_dispositions.py', '09d9fde5d2f857aae80032684b8a641f24ff34fc'],
  ['ide/packaging/test_ide_precore_lock_successor.py', '7ee94931f57a990dbaf51ff57e5a2b0f6c1a4d23'],
  ['ide/packaging/test_verify_ide_precore_r3.py', 'b2ae55023494314d4a8f76f24ca817094dd28c73'],
  ['ide/packaging/verify_ide_precore_lock_successor.py', '0a19ef664666e6e11643d45092e9d3ff5bcffdde'],
  ['ide/packaging/verify_ide_precore_r3.py', 'b9e551f5f8c7b1262527d46f8cb1393de4d3187f'],
  ['ide/proto/lu/ide/v1/native_broker.proto', '9906f7e727af1b8daa80c6898c645fe579eaf89a'],
  ['ide/proto/lu/workspace/v0/readonly.proto', 'fd0d88a26fcb712c5456518ee88550d54a922205'],
  ['ide/packaging/test_ide_precore_r4_native_lock_chain.py', 'eabcb6d2b43fc131fb86b59c19a91ba92561ef4f'],
  ['ide/packaging/verify_ide_precore_r4_native_lock_chain.py', '0d2b10380e5d370a13bc50409f65dbcf7b2a746d'],
]);
export function reviewedR4PrecoreSource(path, blob, mode) {
  return mode === '100644' && R4_PRECORE_SOURCE_PINS.get(path) === blob;
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
export function integratedIdePaths(parentFiles, actual) {
  const parentPaths = new Set(parentFiles.map(x => x.path));
  const idePaths = new Set(parentFiles.filter(x => reviewedIdePath(x.path, parentPaths)).map(x => x.path));
  // 03D adds only an acceptance-only, three-phase cancellation witness.
  // New runtime paths are not implicitly authorized by the integration.
  idePaths.add('ide/supervisor/examples/support/cancel_history.rs');
  for (const p of ['apply_clangd20_windows_startup.py', 'Build-PinnedClangdWindows.ps1', 'test_clangd20_windows_startup.py'])
    idePaths.add('ide/llvm18_owner/' + p);
  for (const item of actual) {
    if (reviewedAcceptanceFixture(item.path, item.blob) ||
        reviewedR14AcceptanceFixture(item.path, item.blob, item.mode) ||
        reviewedR4PrecoreSource(item.path, item.blob, item.mode))
      idePaths.add(item.path);
  }
  // A deleted originally reviewed IDE path is not authorized by merging.
  if ([...idePaths].some(path => !actual.some(x => x.path === path)))
    fail('reviewed IDE path missing');
  return idePaths;
}
export function scopeChanges(old, actual, idePaths = new Set()) {
  const updated = actual.filter(item => old.get(item.path)?.blob !== item.blob);
  const deleted = [...old.keys()].filter(path => !actual.some(item => item.path === path));
  const outOfScope = [...updated.map(x => x.path), ...deleted].filter(p => forbiddenPath(p) && !idePaths.has(p));
  return { updated, deleted, outOfScope };
}
function main(integrated = false) {
  const head = git('rev-parse', 'HEAD');
  if (!/^[a-f0-9]{40}$/.test(head)) fail('invalid HEAD');
  try { git('merge-base', '--is-ancestor', BASE, head); } catch { fail('reviewed baseline is not ancestor'); }
  if (git('status', '--porcelain', '--untracked-files=normal')) fail('checkout not clean');
  const actual = inventory('HEAD');
  verifyInventory(actual, JSON.parse(readFileSync(LOCK, 'utf8')));
  const old = new Map(inventory(BASE).map(x => [x.path, x]));
  let idePaths = new Set();
  if (integrated) {
    try { git('merge-base', '--is-ancestor', MERGE03C, head); }
    catch { fail('reviewed integration is not ancestor'); }
    if (git('show', '-s', '--format=%P', MERGE03C) !== IDE_PARENT + ' ' + CORE_PARENT)
      fail('reviewed integration parent identity differs');
    idePaths = integratedIdePaths(inventory(MERGE03C), actual);
  }
  const { updated, deleted, outOfScope } = scopeChanges(old, actual, idePaths);
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
