import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const text = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const linux = text('../fixtures/dev02_linux_installed_lsp_atspi.py');
const windows = text('../fixtures/Test-Wo8NativeDesktopUI.ps1');
const linuxWorkflow = text('../../../.github/workflows/ide-dev02-linux-installed-protected-lsp.yml');
const windowsWorkflow = text('../../../.github/workflows/ide-dev02-windows-installed-protected-lsp.yml');
const protectedInstaller = text('../../supervisor/fixtures/Install-ProtectedRecoveryService.ps1');
const sourceWorkflow = text('../../../.github/workflows/ide-dev02-protected-multilang-native-ci.yml');

test('Linux native acceptance uses real installed Tauri, root-owned reviewed releases and OS-owned consent', () => {
  for (const marker of [
    'runs-on: [self-hosted, Linux, lu-ide-delegated]',
    'grep -E',
    'stage_linux_language_server_release.py',
    'verify_language_servers.py',
    'verify_p04_license_bundle.py',
    '--features native-lsp',
    '--bundles deb',
    'sudo dpkg --install',
    "import pyatspi",
    "native.select_native_folder()",
    "native.editor_project()",
    'genuine_native_approval()',
    "if not native._native_dialog_ancestor(control)",
    'run_semantic("rust"',
    'run_semantic("cpp"',
    'run_python()',
    'DEV02_LINUX_INSTALLED_ALL_THREE_PROTECTED_PROVIDERS=PASS',
  ]) {
    assert.ok(linux.includes(marker) || linuxWorkflow.includes(marker),marker);
  }
  for (const forbidden of ['continue-on-error: true','--no-verify',
     'window.__TAURI_INTERNALS__','invoke(\'ide_lsp','eval(']) {
    assert.ok(!linuxWorkflow.includes(forbidden),forbidden);
    assert.ok(!linux.includes(forbidden),forbidden);
  }
});

test('Windows native acceptance requires independently signed LocalService and real UIAutomation/OS process reaping',()=>{
  for(const marker of [
    'runs-on: [self-hosted, Windows, lu-ide-protected]',
    'Get-AuthenticodeSignature',
    'stage_language_server_release.py',
    'LU_IDE_E2_D7D_SERVICE_SHA256',
    'verify_p04_license_bundle.py',
    '--features native-lsp',
    '--bundles nsis',
    'LuIdeRecoveryBroker',
    'Install-ProtectedRecoveryService.ps1',
    '-ApprovedHostExecutable $exe',
    '-ExpectedHostSha256 $sha -Start',
    "Join-Path $env:ProgramFiles ('Lu-IDE-DEV02-Native-'",
    "DENIED: installed native LSP desktop host not Authenticode signed",
    "DENIED: restricted broker still exists after uninstall",
    'Dev02ProtectedLsp',
    "Confirm-NativeOneUseDialog 'Allow language analysis' 'Allow this session'",
    'Observe-RealLspWorker',
    'Require-LspReaped',
    "Test-RealInstalledSemanticProvider 'rust'",
    "Test-RealInstalledSemanticProvider 'cpp'",
    'DEV02_INSTALLED_WINDOWS_PYTHON_RUFF_PROTECTED_DIAGNOSTICS=PASS',
    'DEV02_INSTALLED_WINDOWS_ALL_THREE_PROTECTED_PROVIDERS=PASS',
    'DEV02_WINDOWS_REAL_INSTALLED_NSIS_UNINSTALL=PASS',
  ]) assert.ok(windows.includes(marker)||windowsWorkflow.includes(marker),marker);
  assert.ok(!windowsWorkflow.includes('& sc.exe create LuIdeRecoveryBroker'),
    'native Windows SCM service cannot bypass reviewed restricted installer');
  // The actual LocalService identity and private ACLs belong to the reviewed
  // native installer, not a weak hand-built sc.exe service in this workflow.
  for(const expected of [
    "NT AUTHORITY\\\\LocalService", "Get-AuthenticodeSignature",
    "sc.exe sidtype", "sc.exe privs", "approvedHostUserSid",
    "AreAccessRulesProtected", "service configuration ACL not private",
  ]) assert.ok(protectedInstaller.includes(expected),expected);
  for(const forbidden of ['continue-on-error: true','--no-verify',
    'window.__TAURI_INTERNALS__', 'test-mock-lsp']) {
    assert.ok(!windowsWorkflow.includes(forbidden),forbidden);
  }
});

test('ordinary development CI keeps release manifest unapproved and only proves native source',()=>{
  for(const marker of [
    'windows-2022', 'ubuntu-24.04',
    '--features native-lsp --all-targets --locked',
    '--features native-lsp --lib --locked',
    'test_stage_linux_language_server_release.py',
    'DEV02_DEFAULT_LSP_EXECUTION=DISABLED',
    'DEV02_PROTECTED_WIN_LINUX_INSTALLED_EXECUTION=NOT_ACCEPTED',
    'DEV02_PYTHON_SEMANTIC_PYRIGHT=NOT_APPROVED',
  ]) assert.ok(sourceWorkflow.includes(marker),marker);
  for(const forbidden of ['continue-on-error: true','cargo test --exclude',
    'releaseApproved=true','skip-protected']) assert.ok(!sourceWorkflow.includes(forbidden),forbidden);
});
