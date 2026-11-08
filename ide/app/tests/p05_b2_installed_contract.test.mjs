import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
test('P05 B2 native Windows draft must use real CodeMirror and clipboard, with no implicit Save', () => {
  const script = read('../fixtures/Test-Wo8NativeDesktopUI.ps1');
  const part = script.split("if ($Scenario -eq 'DraftBufferReference') {",2)[1]?.split("if ($Scenario -eq 'EditorPersistence') {",1)[0];
  assert.ok(part, 'native installed scenario missing');
  for (const term of ["Source editor text","Copy draft reference","System.Windows.Forms.Clipboard",
    "Get-FileHash","draft_reference_implicit_disk_save",
    "draft_undo_did_not_advance_same_session_revision",
    "P05_B2_INSTALLED_DRAFT_REFERENCE_COPY_UNDO_NO_IMPLICIT_SAVE=PASS"]) assert.ok(part.includes(term),term);
  assert.doesNotMatch(part,/window\.__TAURI_INTERNALS__|invoke_tauri|shell_exec/i);
});
test('Native B2 unsigned NSIS acceptance must preserve exact source and license evidence', () => {
  const ci=read('../../../.github/workflows/ide-p05-b2-installed-native.yml');
  for (const term of ['runs-on: windows-2022','-NoProfile -Sta',
    'DraftBufferReference','npm ci --ignore-scripts',
    'collect_verification_licenses.py','verify_p04_license_bundle.py',
    'P05_B2_INSTALLED_NATIVE_WINDOWS_DRAFT_NO_IMPLICIT_SAVE=PASS',
    "uninstallers","Get-FileHash"]) assert.ok(ci.includes(term),term);
  assert.doesNotMatch(ci,/New-SelfSignedCertificate|releaseApproved: true/);
});

test('B2 native installed Save must rebase reference and restart must rotate session ID', () => {
  const script=read('../fixtures/Test-Wo8NativeDesktopUI.ps1');
  const before=script.split("if ($Scenario -eq 'DraftBufferReference') {",2)[1]?.split(
    "if ($Scenario -eq 'DraftBufferReferenceRestart') {",1)[0];
  const after=script.split("if ($Scenario -eq 'DraftBufferReferenceRestart') {",2)[1]?.split(
    "if ($Scenario -eq 'EditorPersistence') {",1)[0];
  assert.ok(before && after, 'missing real native B2 OS scenarios');
  for (const expected of [
    "draft_post_undo_implicit_disk_save",
    "P05_B2_EXPLICIT_SAVE", "Click 'Save · Ctrl+S'", "Saved main.rs",
    "Copy draft reference", "baseDisk=", "draftSha256=",
    "saved_draft_reference_not_rebased_to_actual_native_disk_sha",
    "P05_B2_NATIVE_SESSION_BUFFER_ID=",
    "P05_B2_INSTALLED_DRAFT_EXPLICIT_SAVE_REBASE=PASS",
  ]) assert.ok(before.includes(expected), expected);
  for (const expected of [
    "ExpectedPreviousBufferId", "previous_actual_disk_save_missing_after_restart",
    "Copy draft reference", "Get-FileHash", "baseDisk=",
    "native_restart_reused_stale_session_draft_reference",
    "P05_B2_INSTALLED_RESTART_NEW_SESSION_ID_AND_SAVED_SHA=PASS",
  ]) assert.ok(after.includes(expected), expected);
  assert.doesNotMatch(before+after,/window\.__TAURI_INTERNALS__|invoke_tauri|shell_exec/i);
  const native=read('../../../.github/workflows/ide-p05-b2-installed-native.yml');
  for (const expected of [
    'DraftBufferReferenceRestart', 'ExpectedPreviousBufferId',
    'P05_B2_NATIVE_SESSION_BUFFER_ID=', 'P05_B2_EXPLICIT_SAVE',
    'P05_B2_INSTALLED_NATIVE_WINDOWS_DRAFT_UNDO_SAVE_RESTART=PASS',
  ]) assert.ok(native.includes(expected), expected);
  assert.doesNotMatch(native,/continue-on-error:\s*true|New-SelfSignedCertificate/i);
});
