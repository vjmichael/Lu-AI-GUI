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
