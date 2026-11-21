import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const text = relative => readFileSync(new URL(relative, import.meta.url), 'utf8');

test('integrated Windows and Linux source build actual upstream license evidence before Tauri', () => {
  const source = text('../../../.github/workflows/ide-group6f-integrated-native.yml');
  // R4's reviewed bridge preserves the original P06/MERGE02C verifiers and
  // independently pins current R2 IPC and supervisor lock successors.
  const lockBridge = text('../../packaging/verify_ide_precore_r4_native_lock_chain.py');
  assert.match(lockBridge, /verify_p06_candidate_lock\.py/);
  assert.match(lockBridge, /--ide-merge02-successor/);
  assert.match(lockBridge, /verify_ide_merge02c_native_locks\.py/);
  assert.match(lockBridge, /verify_ipc_successor\(root\)/);
  assert.match(lockBridge, /verify_supervisor_successor\(root, data\)/);
  assert.match(lockBridge, /worktree", "add", "--detach"/);
  assert.match(lockBridge, /HISTORICAL_SHA = "f1f0ed4bc1e740d2a9f85615b988b779a46651db"/);

  const batches = [
    source.split('  dev01-modern-real-installed-linux:')[1]?.split('  dev01-modern-real-installed-windows:')[0],
    source.split('  dev01-modern-real-installed-windows:')[1],
  ];
  assert.equal(batches.length, 2);
  for (const [index, batch] of batches.entries()) {
    assert.ok(batch, 'both real native installed build jobs are required');
    assert.match(batch, /cargo fetch --locked --manifest-path src-tauri\/Cargo\.toml/);
    assert.equal((batch.match(/verify_ide_precore_r4_native_lock_chain\.py/g) || []).length, 2,
      'current source locks must be checked before build and again after Cargo fetch');
    assert.match(batch, /--platform (?:ubuntu-24\.04|windows-2022)/);
    assert.ok(batch.indexOf('verify_ide_precore_r4_native_lock_chain.py') <
      batch.indexOf('upstream_notice_receipt.py'),
      'reviewed original and successor source locks must precede licensing and package creation');
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
  assert.match(confirm, /_native_process_id\(node\) != pid/);
  assert.match(confirm, /_same_window_geometry\(node, geometry\)/);
  assert.match(confirm, /node\.parent\.getRoleName\(\)\.casefold\(\) != "application"/);
  assert.match(confirm, /message_seen and len\(buttons\) == 1/);
  assert.match(confirm, /_active_confirmation_window\(proc, expected_title\) != \(active, pid, title\)/);
  assert.match(confirm, /action\.doAction\(indices\[0\]\)/);
  assert.doesNotMatch(confirm, /native\.activate\(/);
  const clangd = text('../../llvm18_owner/Build-PinnedClangdWindows.ps1');
  assert.match(clangd, /Get-Command ninja\.exe[^\n]+Select-Object -First 1/);
  assert.match(clangd, /Test-Path -LiteralPath \$ninja -PathType Leaf/);
  assert.match(clangd, /ninjaSha256=/);
});

// Real native receipts remain the gate; these source checks prevent restoring stale selectors or bypassing evidence.
test('03G accessible status and native review selectors remain bound to actual UI state', () => {
  const web=text('../web/main.jsx');
  const win=text('../fixtures/Test-Wo8NativeDesktopUI.ps1');
  const b2win=text('../fixtures/Test-B2InstalledDraftRecovery.ps1');
  const linux=text('../fixtures/dev01_modern_linux_installed_atspi.py');
  const b2linux=text('../fixtures/b2_linux_installed_draft_atspi.py');
  const b5linux=text('../fixtures/b5_linux_installed_perf_atspi.py');
  const atspi=text('../fixtures/wo8_linux_editor_atspi.py');
  assert.match(web, /aria-label="Remember open tabs for restart"/);
  for (const body of [win,linux]) assert.match(body,/Remember open tabs for restart/);
  for (const body of [win,b2win,linux,b2linux]) {
    assert.match(body,/Recovered private checkpoint into unsaved editor only/);
    assert.doesNotMatch(body,/Recovered private checkpoint to editor only/);
  }
  assert.match(web,/problem\.startsWith\('Open-file budget'\) \? problem/);
  assert.match(web,/aria-label=\{workingSetNotice\}/);
  assert.match(web,/aria-label="No current project-owned native terminal stream"/);
  assert.match(web,/role="region" aria-label="Projects in workflow"/);
  assert.match(web,/getAll\('relativeSourcePath'\)/);
  assert.match(web,/verifiedRelativeSourcePath\(submittedPath\)/);
  const nativeCommit = win.split('# Exactly one verified foreground pointer dispatch')[1].split("Find-Control ('Local commit: '+$after) '' 15 -Exact")[0];
  assert.equal((nativeCommit.match(/Click 'Confirm separate local commit \(no push\)'/g)||[]).length,1);
  assert.match(win,/\$hit\.Equals\(\$fresh\)/);
  assert.match(win,/\$bounds\.Contains\(\$point\)/);
  assert.match(win,/\$fresh\.Current\.Name -cne \$label/);
  assert.match(win,/Confirm-NativeOneUseDialog 'Confirm local Git commit'/);
  assert.match(b2win,/FocusedElement/);
  assert.match(b2win,/Native clipboard did not observe exact changed editor text/);
  assert.equal((b5linux.match(/48 \/ 48 tabs in this project/g)||[]).length,2);
  assert.match(b5linux,/text\.getText\(0,text\.characterCount\)==name/);
  assert.match(atspi,/yield from descendants\(groups\[0\]\)/);
  assert.match(atspi,/STATE_CHECKED/);
});

test('dirty tab layout requires genuine native metadata-only confirmation before host save', () => {
  const web=text('../web/main.jsx');
  const host=text('../src-tauri/src/lib.rs');
  const route=host.split('async fn ide_workspace_layout_save(')[1].split('async fn ide_workspace_layout_clear(')[0];
  const save=web.split('async function rememberWorkspaceForRestart()')[1].split('async function forgetRememberedWorkspace()')[0];
  assert.doesNotMatch(save,/window\.confirm/);
  assert.match(save,/invoke\('ide_workspace_layout_save',\{layout\}\)/);
  assert.match(route,/file\.was_dirty/);
  assert.match(route,/\.title\("Lu IDE · Remember tab layout"\)/);
  assert.match(route,/B9_LAYOUT_NOT_CONFIRMED/);
  assert.ok(route.indexOf('B9_LAYOUT_NOT_CONFIRMED')<route.indexOf('layouts.save(&registry, layout)'));
  assert.match(text('../fixtures/Test-Wo8NativeDesktopUI.ps1'),/Confirm-NativeOneUseDialog 'Remember tab layout' 'Remember navigation'/);
  assert.match(text('../fixtures/dev01_modern_linux_installed_atspi.py'),/expected_title="Lu IDE · Remember tab layout", caption="Remember navigation"/);
});

// Accessible names describe the actual result state; no fabricated success.
test('source action and stale saved-search notifications have accessible state labels', () => {
  const web=text('../web/main.jsx');
  const banner=web.split('{problem && <div className="errorMsg appError"')[1].split('{isWorkbench')[0];
  assert.match(banner,/problem\.startsWith\('Source action not verified:'\)/);
  assert.match(banner,/problem\.startsWith\('Cannot open that saved search hit:'\)\) \? problem/);
  assert.match(banner,/hint\.startsWith\('Source action completed:'\) \? hint/);
  assert.match(banner,/\{problem\}/);
  assert.match(banner,/\{hint\}/);
});
