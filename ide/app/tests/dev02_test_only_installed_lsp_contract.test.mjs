import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read = path => readFileSync(new URL(path,import.meta.url),'utf8');
const workflow=read('../../../.github/workflows/ide-dev02-test-only-installed-lsp-ci.yml');
const stager=read('../../packaging/stage_test_only_lsp_resources.py');
const linux=read('../src-tauri/tauri.dev02.technical-linux.conf.json');
const windows=read('../src-tauri/tauri.dev02.technical-windows.conf.json');

test('technical LSP packages include real reviewed hashes and notices without release permission',()=>{
  for(const marker of [
    'stage_test_only_lsp_resources.py',
    'lsp_upstream_archive_candidates_20261108.json',
    'lsp_original_upstream_notices_20261108.json',
    'extract_one(archive,entry["kind"],entry["binary"],binary)',
    'linux_tree_sha(server)',
    'windows_lsp_tree_digest(server)',
    'sourceHead', 'releaseApproved', 'redistributionReviewed',
    'signedServiceInstalled', 'runLanguageServers',
    'installedProtectedSessionAccepted',
    'DEVELOPMENT-ONLY-NOT-FOR-DISTRIBUTION.txt',
    'resource output must remain within RUNNER_TEMP',
  ]) assert.ok(stager.includes(marker),marker);
  assert.ok(stager.includes('raise ValueError("official release archive digest mismatch")'));
  assert.ok(!stager.includes('releaseApproved":True'));
  assert.ok(!stager.includes('subprocess.run('));
  assert.ok(!stager.includes('os.system('));
});

test('exact native Windows/Linux CI builds and installs real packages, validates source and uninstalls',()=>{
  for(const marker of [
    'ubuntu-24.04','windows-2022',
    'source-sha "$SOURCE_SHA"',
    '--fetch-locked',
    'verify_language_servers.py --release',
    'test_stage_test_only_lsp_resources.py',
    '--features native-lsp',
    'tauri.dev02.technical-linux.conf.json',
    'tauri.dev02.technical-windows.conf.json',
    '--bundles "${{ matrix.bundle }}"',
    'dpkg --install "$deb"',
    'sudo dpkg --remove "$pkg"',
    'Start-Process -FilePath $packages[0].FullName',
    'Test-Wo8NativeDesktopUI.ps1',
    'dev02_test_only_installed_linux.py',
    'DEV02_TEST_ONLY_INSTALLED_NSIS_UNINSTALL=PASS',
    'DEV02_TEST_ONLY_INSTALLED_DEBIAN_APP_AND_RESOURCES=PASS',
    'DEV02_TECHNICAL_INSTALLERS_AND_PROVIDER_BINARIES_NOT_UPLOADED',
    'DEV02_LEGAL_DISTRIBUTION_APPROVED=NO'
  ]) assert.ok(workflow.includes(marker),marker);
  for(const forbidden of ['continue-on-error: true','--no-verify','disable-default-grants',
    'actions/upload-artifact@', 'sc.exe create', 'shell: cmd']) {
    assert.ok(!workflow.includes(forbidden),forbidden);
  }
});

test('technical package configurations require clear no-release warnings and original NOTICE resources',()=>{
  const records=[JSON.parse(linux),JSON.parse(windows)];
  for(const [i,record] of records.entries()){
    assert.deepEqual(record.bundle.targets,[i===0?'deb':'nsis']);
    const resources=record.bundle.resources;
    assert.equal(resources['test-lsp-resources/language-servers/'],'language-servers');
    assert.equal(resources['verification-notices/'],'THIRD_PARTY_LICENSES');
    assert.ok(Object.keys(resources).some(k=>k.includes('DEVELOPMENT-ONLY-NOT-FOR-DISTRIBUTION')));
    assert.ok(Object.keys(resources).some(k=>k.includes('technical-lsp-resource-receipt')));
    assert.ok(!Object.keys(resources).some(k=>k.includes('recovery/')));
  }
});
