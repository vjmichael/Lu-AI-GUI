import { readFileSync } from 'node:fs';
import test from 'node:test';
import assert from 'node:assert/strict';
import { forbiddenPath, suspectSource, verifyInventory, reviewedIdePath, reviewedAcceptanceFixture, reviewedR14AcceptanceFixture, reviewedR4PrecoreSource, integratedIdePaths, scopeChanges } from './core01-verify-scope.mjs';
test('IDE source paths and build workflows cannot enter CORE-01', () => {
  for (const p of ['ide/worker.rs','src-tauri/lu-ide/src/lib.rs','lu-desktop/src-tauri/src/ide_worker.rs','.github/workflows/ide-native.yml','docs/LU_IDE_GATE.md']) assert.equal(forbiddenPath(p), true, p);
  assert.equal(forbiddenPath('src-tauri/lu-core/src/independent_outcome.rs'), false);
});
test('experimental provider additions are refused including nested paths', () => {
  for (const p of ['src-tauri/lu-core/src/p7_koboldcpp_backend.rs','docs/p7l-native.md','.github/workflows/lu-perf01-p7l1-native.yml','x/y/koboldcpp.toml']) assert.equal(forbiddenPath(p), true, p);
});
test('provider registration and dependency injection are refused even in allowed paths', () => {
  assert.ok(suspectSource('src-tauri/lu-core/src/lib.rs', 'mod koboldcpp;').length);
  assert.ok(suspectSource('lu-desktop/src-tauri/src/lib.rs', 'use lu_ide::worker;').length);
  assert.ok(suspectSource('src-tauri/lu-core/Cargo.toml', 'koboldcpp = "1"').length);
  assert.equal(suspectSource('src-tauri/lu-core/src/lib.rs', 'mod persistent_llama_backend;').length, 0);
});
test('docs can record exclusions without falsely adding a runtime provider', () => {
  assert.deepEqual(suspectSource('docs/LU_CORE01_SOURCE_CONVERGENCE_20261112.md', 'KoboldCpp excluded'), []);
});
test('manifest rejects extras and modified blobs, not just path prefixes', () => {
  const a=[{path:'a.rs',blob:'a'.repeat(40)}], m={schema:1,trackedFileCount:1,files:[{path:'a.rs',blob:'a'.repeat(40)}]};
  assert.doesNotThrow(()=>verifyInventory(a,m));
  assert.throws(()=>verifyInventory([...a,{path:'extra.rs',blob:'b'.repeat(40)}],m),/REFUSED/);
  assert.throws(()=>verifyInventory(a,{...m,files:[{path:'a.rs',blob:'b'.repeat(40)}]}),/REFUSED/);
});

test('PERF-01 warm model source retains the pressure branch outside comments', () => {
  const original = readFileSync(new URL('../src-tauri/lu-core/src/warm_model_residency.rs', import.meta.url), 'utf8');
  assert.ok(original.includes('if idle_for.is_some_and(|elapsed| elapsed >= Duration::from_secs(30))'));
  assert.ok(original.includes('return WarmDecision::EvictMeasuredPressure;'));
  assert.ok(!original.includes('\\n        if idle_for'), 'Literal escaped newlines must not swallow control flow');
});

test('integrated scope accepts only known IDE-owned paths, never cross-imports or excluded providers', () => {
  const parent = new Set(['ide/worker.rs', '.github/workflows/ide-native.yml', 'src-tauri/lu-ide/src/lib.rs', 'ide/koboldcpp.rs', 'docs/LU_IDE_GATE.md']);
  assert.equal(reviewedIdePath('ide/worker.rs', parent), true);
  assert.equal(reviewedIdePath('.github/workflows/ide-native.yml', parent), true);
  for (const path of ['ide/unreviewed.rs','src-tauri/lu-ide/src/lib.rs','ide/koboldcpp.rs','docs/LU_IDE_GATE.md']) assert.equal(reviewedIdePath(path, parent), false);
  assert.equal(forbiddenPath('ide/worker.rs'), true);
  assert.ok(suspectSource('lu-desktop/src-tauri/src/lib.rs', 'use lu_ide::worker;').length);
});

test('integrated 03H permits only the exact reviewed native confirmation test bytes', () => {
  const path='ide/app/fixtures/test_dev01_native_confirmation_scope.py';
  const blob='acfcd77984123f0a53770826e2bc408258b74b83';
  assert.equal(reviewedAcceptanceFixture(path,blob),true);
  assert.equal(reviewedAcceptanceFixture(path,'a'.repeat(40)),false);
  assert.equal(reviewedAcceptanceFixture(path+'.bak',blob),false);
  assert.equal(forbiddenPath(path),true);
});


test('integrated 04B permits only exact CI handoff and hostile fixture source, never in Core-only scope', () => {
  assert.equal(reviewedAcceptanceFixture('ide/packaging/b2_b4_installer_handoff.py','568a7513560e4282b10fda79f7268c235ab36329'),true);
  assert.equal(reviewedAcceptanceFixture('ide/packaging/b2_b4_installer_handoff.py','a'.repeat(40)),false);
  assert.equal(forbiddenPath('ide/packaging/b2_b4_installer_handoff.py'),true);
  assert.equal(reviewedAcceptanceFixture('ide/packaging/test_b2_b4_installer_handoff.py','d6332db3e7d822911e05da28dd481272d155374b'),true);
  assert.equal(reviewedAcceptanceFixture('ide/packaging/test_b2_b4_installer_handoff.py','a'.repeat(40)),false);
  assert.equal(forbiddenPath('ide/packaging/test_b2_b4_installer_handoff.py'),true);
  assert.equal(reviewedAcceptanceFixture('ide/packaging/other.py','a'.repeat(40)),false);
});

test('integrated 04K admits only exact acceptance fixture blobs', () => {
  assert.equal(reviewedAcceptanceFixture('ide/app/fixtures/test_wo9_review_control_policy.py','99d92eb7f1ba4803322c41cf6f9cfa4a292a2224'),true);
  assert.equal(reviewedAcceptanceFixture('ide/app/fixtures/test_wo9_review_control_policy.py','a'.repeat(40)),false);
  assert.equal(forbiddenPath('ide/app/fixtures/test_wo9_review_control_policy.py'),true);
  assert.equal(reviewedAcceptanceFixture('ide/app/fixtures/x11_openbox_readiness.sh','a6015822363cf0c47bd9a04368456ba963569f17'),true);
  assert.equal(reviewedAcceptanceFixture('ide/app/fixtures/x11_openbox_readiness.sh','a'.repeat(40)),false);
  assert.equal(forbiddenPath('ide/app/fixtures/x11_openbox_readiness.sh'),true);
});

// Exact published source inventory with the formatting-only R11 vcpkg postimage.
const r14Pins = [
  ['.github/workflows/ide-dev02-technical-installed-lsp.yml','e4dbd01bf09cf77ca6b69274df85f4c4b26fd50c'],
  ['ide/app/fixtures/Run-DEV02TechnicalGtkSession.sh','8a0f37bc60a394d756ce9806ac75d014b23a2e57'],
  ['ide/app/fixtures/Test-DEV02TechnicalInstalledDeb.sh','b485652458b6c75c916aaa2bfe5072d96366283a'],
  ['ide/app/fixtures/Test-DEV02TechnicalInstalledNsis.ps1','9a9bd966abbeb4908394a516b34a416f4224fce5'],
  ['ide/app/fixtures/dev02_technical_installed_linux.py','459b6ccee788928e49398a69f89851741be30afd'],
  ['ide/app/fixtures/test_dev01_native_confirmation_scope.py','6d5258f6ebc6e97c1a54e6b4bc49b3f448a6795b'],
  ['ide/app/src-tauri/tauri.dev02.technical-only.conf.json','31010228b7fac0cd8d0dbf08df074c8aafb94124'],
  ['ide/packaging/ide-05b-r3-outside-cohort.json','90e56a2c0769f68ac7a647f965c35e5efd294d71'],
  ['ide/packaging/p06-observer-cohort.json','a11faff282ad98d435adfb13f55ec74f8d2544cc'],
  ['ide/packaging/test_ide_05b_r3_workflow_dispositions.py','d8dccd6cbaa34fb8666c8ed58cf3756b15f4f3b7'],
  ['ide/packaging/test_p06_technical_observation.py','f6718d7fd6c91a1bd901296652be95896a7395df'],
  ['ide/packaging/test_verify_technical_lsp_install.py','2fcb753a04ae0523bfe11d7629be21471474e9db'],
  ['ide/packaging/verify_technical_lsp_install.py','0e8071c51d3918a3884d111dcb44f3e8d5334f5d'],
  ['ide/supervisor/examples/dev03_linux_runtime_acceptance.rs','1ae254bb5e1915c3db295b09cf59d81870525a81'],
  ['ide/supervisor/examples/ide08_native_vcpkg_acceptance.rs','eca321d18d23fcfa6d412a73a18eb831da435a9c'],
  ['ide/supervisor/examples/wo6_linux_interrupted_recovery.rs','08a615bb342377786af23e6f98dfd2d48f6da9e8'],
  ['ide/supervisor/examples/wo6_linux_rustdoc_admission.rs','0a0f42afe6a639058858de1beb38071845e1eb69'],
  ['ide/supervisor/fixtures/debug/dap_launch_contract.py','c0fee68885eee5a04ab2098332e250b0152314fd'],
  ['ide/supervisor/fixtures/debug/test_dap_launch_contract.py','0713ca357d7b50217682ec4a6a30abd600ff1348'],
  ['ide/supervisor/fixtures/debug/test_dap_native_source_contract.py','971035a8011369ba581c0f95832dc0ddc7af8883'],
  ['ide/tools/tests/test_provision_cmake_candidate.py','b59f7724e3dfaf41d8f98ebd91103fe87d28a70a'],
];
const r14Items = r14Pins.map(([path,blob]) => ({path,blob,mode:'100644'}));
const requiredOldPaths = ['ide/supervisor/examples/support/cancel_history.rs',
  'ide/llvm18_owner/apply_clangd20_windows_startup.py',
  'ide/llvm18_owner/Build-PinnedClangdWindows.ps1',
  'ide/llvm18_owner/test_clangd20_windows_startup.py']
  .map(path => ({path,blob:'b'.repeat(40),mode:'100644'}));
test('R14 accepts the 21 exact regular-file fixtures only', () => {
  assert.equal(r14Pins.length,21);
  for (const {path,blob,mode} of r14Items) {
    assert.equal(reviewedR14AcceptanceFixture(path,blob,mode),true,path);
    assert.equal(reviewedR14AcceptanceFixture(path,'f'.repeat(40),mode),false,path);
    assert.equal(reviewedR14AcceptanceFixture(path,blob,'120000'),false,path);
    assert.equal(reviewedR14AcceptanceFixture(path,blob,'100755'),false,path);
    assert.equal(reviewedR14AcceptanceFixture(path+'.bak',blob,mode),false,path);
    assert.equal(reviewedR14AcceptanceFixture(path,blob,undefined),false,path);
    assert.equal(forbiddenPath(path),true,path);
  }
  assert.equal(reviewedR14AcceptanceFixture('ide/supervisor/examples/ide08_native_vcpkg_acceptance.rs',
    'e87ac3ece23c5b2bf9998fa4a2ec2fcd96c4bd66','100644'),false);
});
test('R14 integrated admission is not a Core-only exemption', () => {
  const actual = [...requiredOldPaths,...r14Items];
  const standalone = scopeChanges(new Map(), actual);
  assert.equal(standalone.outOfScope.length,actual.length);
  assert.equal(scopeChanges(new Map(),actual,integratedIdePaths([],actual)).outOfScope.length,0);
});
test('R14 integrated scope denies each substituted fixture blob and mode', () => {
  for (const item of r14Items) {
    for (const mutation of [{blob:'f'.repeat(40)}, {mode:'120000'}]) {
      const actual = [...requiredOldPaths,...r14Items.map(x => x.path===item.path ? {...x,...mutation} : x)];
      const changed = scopeChanges(new Map(),actual,integratedIdePaths([],actual));
      assert.deepEqual(changed.outOfScope,[item.path]);
    }
  }
});
test('R14 refuses unknown paths, deleted reviewed parents and cross-layer source', () => {
  const unknown = {path:'ide/new-runtime.rs',blob:r14Items[0].blob,mode:'100644'};
  const actual = [...requiredOldPaths,...r14Items,unknown];
  assert.deepEqual(scopeChanges(new Map(),actual,integratedIdePaths([],actual)).outOfScope,[unknown.path]);
  assert.throws(()=>integratedIdePaths([{path:'ide/old-reviewed.rs'}],actual),/reviewed IDE path missing/);
  assert.deepEqual(suspectSource('lu-desktop/src-tauri/src/lib.rs','use lu_ide::worker;'),['IDE source import']);
  assert.deepEqual(suspectSource('ide/host/src/lib.rs','mod koboldcpp;'),['excluded backend reference']);
});


test('R4 PRECORE pins are exact, source-only, and cannot widen the Core-only gate', () => {
  const accepted = [
    ['ide/host/src/core_ipc_issuer.rs', '405669503461333f1e75f1d227f79f02f962e15f'],
    ['ide/packaging/ide-05b-r3-outside-cohort.json', 'd603439db9d17a433a0fc694dda409e63db154d3'],
    ['ide/packaging/ide-precore-r3-admission-register.json', '402228fe180287f0c74eb0e6bf4df2bad27a9904'],
    ['ide/packaging/test_ide_05b_r3_workflow_dispositions.py', '09d9fde5d2f857aae80032684b8a641f24ff34fc'],
  ];
  assert.equal(accepted.length,4);
  for(const [path,blob] of accepted) {
    assert.equal(reviewedR4PrecoreSource(path,blob,'100644'),true,path);
    assert.equal(reviewedR4PrecoreSource(path,'f'.repeat(40),'100644'),false,path);
    assert.equal(reviewedR4PrecoreSource(path,blob,'120000'),false,path);
    assert.equal(reviewedR4PrecoreSource(path,blob,'100755'),false,path);
    assert.equal(forbiddenPath(path),true,path);
    assert.deepEqual(scopeChanges(new Map(),[{path,blob,mode:'100644'}]).outOfScope,[path]);
    assert.deepEqual(scopeChanges(new Map(),[{path,blob,mode:'100644'}],
      integratedIdePaths([],[{path,blob,mode:'100644'}])).outOfScope,[]);
  }
  assert.equal(reviewedR4PrecoreSource('ide/host/src/unknown.rs','a'.repeat(40),'100644'),false);
});

