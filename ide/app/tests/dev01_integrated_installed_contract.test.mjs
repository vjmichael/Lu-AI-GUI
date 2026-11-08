import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read = path => readFileSync(new URL(path,import.meta.url),'utf8');
const windows=read('../fixtures/Test-Wo8NativeDesktopUI.ps1');
const linux=read('../fixtures/dev01_linux_installed_atspi.py');
const yaml=read('../../.github/workflows/ide-dev01-installed-integration-ci.yml');

test('Windows integrated acceptance exercises real installed UIA, OS sources and force-kill restart',()=>{
  for(const needle of [
    "'Dev01Integrated','Dev01RecoveryRestart'",
    "function Confirm-Dev01OwnedBrowserPrompt",
    "DEV01_INSTALLED_WINDOWS_LARGE_PROJECT_QUICK_OPEN=PASS",
    "DEV01_INSTALLED_WINDOWS_NATIVE_NO_CLOBBER_FILE_MOVE=PASS",
    "DEV01_INSTALLED_WINDOWS_TRASH_AND_RESTORE=PASS",
    "DEV01_INSTALLED_WINDOWS_SPLIT_EDITOR_SECONDARY_SAVE=PASS",
    "DEV01_INSTALLED_WINDOWS_DRAFT_CHECKPOINT_BEFORE_KILL=PASS",
    "DEV01_INSTALLED_WINDOWS_PREVIOUS_SESSION_DRAFT_RECOVERY=PASS",
    "Stop-Process -Id $proc.Id -Force",
    "Get-FileHash -Algorithm SHA256",
    "Source editor text",
  ]) assert.ok(windows.includes(needle),needle);
  for(const forbidden of ["Invoke-WebRequest", "window.__TAURI_INTERNALS__", "ExecuteScript",
    "javascript:", "Test-Path -LiteralPath $source -PathType Leaf; Write-Host 'DEV01_INSTALLED_WINDOWS_ALL"]) {
    assert.ok(!windows.includes(forbidden),forbidden);
  }
});

test('Linux integrated acceptance only uses actual native GTK, AT-SPI, X11 and real OS restart',()=>{
  for(const needle of [
    "import wo8_linux_editor_atspi as native",
    "native.select_native_folder()",
    "native.editor_project()",
    "DEV01_LINUX_INSTALLED_LARGE_REPO_INDEX_UI=PASS",
    "DEV01_LINUX_INSTALLED_NATIVE_FILE_MOVE=PASS",
    "DEV01_LINUX_INSTALLED_NATIVE_TRASH_RESTORE=PASS",
    "DEV01_LINUX_INSTALLED_SPLIT_SECONDARY_SAVE=PASS",
    "DEV01_LINUX_INSTALLED_PREVIOUS_SESSION_RECOVERY=PASS",
    "first.kill()",
    "second = native.start()",
    "native_owned_confirmation(",
    "pyatspi",
  ]) assert.ok(linux.includes(needle),needle);
  for(const forbidden of ["window.__TAURI_INTERNALS__", "invoke('ide_", "webview.execute", "localStorage",
      "requests.post", "subprocess.run(['node'"]) assert.ok(!linux.includes(forbidden),forbidden);
});

test('native installed workflow compiles, legally identifies and uninstalls same SHA on both OSes',()=>{
 for(const needle of [
   'windows-2022', 'ubuntu-24.04',
   'tauri.installed-smoke.conf.json', 'tauri.wo8.linux.conf.json',
   'verify_p06_candidate_lock.py', 'verify_p04_license_bundle.py',
   'collect_verification_licenses.py', 'upstream_notice_receipt.py',
   'Test-Wo8NativeDesktopUI.ps1',
   'dev01_linux_installed_atspi.py',
   'Dev01Integrated', 'Dev01RecoveryRestart',
   'Start-Process -FilePath $uninstallers[0].FullName',
   'sudo dpkg --remove "$package"',
   'DEV01_INSTALLED_WINDOWS_ALL_FOUR_AND_UNINSTALL=PASS',
   'DEV01_LINUX_REAL_DEBIAN_ALL_FOUR=PASS',
   'DEV01_SIGNED_RELEASE=NOT_ACCEPTED',
 ]) assert.ok(yaml.includes(needle),needle);
 for(const forbidden of ['skip-tests','continue-on-error: true','--no-verify',
   'mockWebview', 'window.__TAURI_INTERNALS__']) assert.ok(!yaml.includes(forbidden),forbidden);
});
