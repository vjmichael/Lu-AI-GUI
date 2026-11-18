import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const text = relative => readFileSync(new URL(relative, import.meta.url), 'utf8');

test('integrated Windows and Linux source build actual upstream license evidence before Tauri', () => {
  const source = text('../../../.github/workflows/ide-group6f-integrated-native.yml');
  const batches = [
    source.split('  dev01-modern-real-installed-linux:')[1]?.split('  dev01-modern-real-installed-windows:')[0],
    source.split('  dev01-modern-real-installed-windows:')[1],
  ];
  assert.equal(batches.length, 2);
  for (const [index, batch] of batches.entries()) {
    assert.ok(batch, 'both real native installed build jobs are required');
    assert.match(batch, /cargo fetch --locked --manifest-path src-tauri\/Cargo\.toml/);
    assert.match(batch, /verify_p06_candidate_lock\.py/);
    assert.match(batch, /--ide-merge02-successor/);
    assert.match(batch, /upstream_notice_receipt\.py/);
    assert.match(batch, /collect_verification_licenses\.py/);
    assert.match(batch, /--upstream-receipt src-tauri\/upstream-notice-receipt\.json/);
    assert.match(batch, /--output src-tauri\/verification-notices/);
    assert.match(batch, /verify_p04_license_bundle\.py/);
    assert.match(batch, /--require-upstream/);
    assert.match(batch, /--artifact/);
    assert.match(batch, /--receipt ide\/app\/src-tauri\/verification-package-receipt\.json/);
    assert.match(batch, /@tauri-apps\/cli@2\.12\.4 build/);
    assert.ok(batch.indexOf('collect_verification_licenses.py') <
      batch.indexOf('@tauri-apps/cli@2.12.4 build'), 'notices must precede real package build');
    assert.doesNotMatch(batch, /mkdir\s+-p[^\n]*verification-notices/);
    assert.doesNotMatch(batch, /dummy|placeholder license|skip(?:-license|-notice)/i);
    assert.ok(index < 2);
  }
});
test('actual installed NSIS/DEB must contain source-bound resolved and upstream notices', () => {
  const windows = text('../fixtures/Test-DEV01ModernInstalledNsis.ps1');
  const linux = text('../fixtures/Test-DEV01ModernInstalledDeb.sh');
  assert.match(windows, /THIRD_PARTY_LICENSES/);
  assert.match(windows, /inventory\.json/);
  assert.match(windows, /upstream-fetch\.json/);
  assert.match(windows, /\.sourceSha -cne \$SourceSha/);
  assert.match(windows, /licenseLegalReviewApproved -ne \$false/);
  assert.match(linux, /dpkg-deb --contents "\$deb"/);
  assert.match(linux, /THIRD_PARTY_LICENSES\/inventory\.json/);
  assert.match(linux, /THIRD_PARTY_LICENSES\/upstream-fetch\.json/);
  assert.match(linux, /native_four_feature_installed_os_regression_failed/);
});

// Source regression only: real installed UI and protected native execution remain mandatory.
test('DEV01 and pinned clangd retain fail-closed native repair boundaries', () => {
  const win = text('../fixtures/Test-Wo8NativeDesktopUI.ps1');
  const input = win.split('function Dev01-Input(')[1].split('function Dev01-Confirm(')[0];
  assert.match(input, /Find-Control \$label 'Edit' 2 -Exact/);
  assert.match(input, /FocusedElement/);
  assert.match(input, /FromPoint\(\$point\)/);
  assert.match(input, /\$exactHit/);
  assert.match(input, /GetForegroundWindow\(\)/);
  assert.match(input, /ValuePattern.*Current\.Value -ceq \$value/);
  assert.doesNotMatch(input, /\.SetValue\(/);
  const linux = text('../fixtures/dev01_modern_linux_installed_atspi.py');
  const confirm = linux.split('def owned_confirmation(')[1].split('def verify_fixture(')[0];
  assert.match(confirm, /getApplication\(\)\.get_process_id\(\) == pid/);
  assert.match(confirm, /message_seen and len\(buttons\) == 1/);
  assert.match(confirm, /_active_confirmation_window\(proc\) != \(active, pid, title\)/);
  assert.match(confirm, /action\.doAction\(indices\[0\]\)/);
  assert.doesNotMatch(confirm, /native\.activate\(/);
  const clangd = text('../../llvm18_owner/Build-PinnedClangdWindows.ps1');
  assert.match(clangd, /Get-Command ninja\.exe[^\n]+Select-Object -First 1/);
  assert.match(clangd, /Test-Path -LiteralPath \$ninja -PathType Leaf/);
  assert.match(clangd, /ninjaSha256=/);
});
