import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = relative => readFileSync(new URL(relative, import.meta.url), 'utf8');

test('E2 real server launch still requires a separately verified D7D native worker', () => {
  const desktop = read('../src-tauri/src/lib.rs');
  const editor = read('../web/main.jsx');
  const supervisor = read('../../supervisor/src/lsp_transport.rs');
  const codeMirror = read('../web/code_editor.jsx');
  assert.match(desktop, /async fn ide_lsp_worker_status/);
  assert.match(desktop, /ready: false/);
  assert.match(desktop, /LSP_PROTECTED_WORKER_UNVERIFIED/);
  assert.match(supervisor, /trait RestrictedLspProcess/);
  assert.match(supervisor, /terminate_tree_and_verify/);
  assert.match(supervisor, /job.phase != JobPhase::Running/);
  assert.match(editor, /!lspWorker\?\.ready/);
  assert.match(editor, /ide_lsp_feature/); // request reserved, not registered
  assert.doesNotMatch(desktop, /async fn ide_lsp_feature|fn ide_lsp_read_diagnostics/);
  assert.doesNotMatch(supervisor, /std::process::Command|Command::new|\.spawn\(/);
  assert.match(codeMirror, /lspDiagnosticField/);
  assert.match(codeMirror, /completionApplyRequest\.expectedDraft/);
});

test('E2 bundling is explicitly unapproved without provenance, notices and hashes', () => {
  const manifest = JSON.parse(read('../../packaging/language-servers.manifest.json'));
  assert.equal(manifest.releaseApproved, false);
  assert.equal(manifest.autoInstallOnProjectOpen, false);
  assert.equal(manifest.allowUnpinnedExecutables, false);
  assert.equal(manifest.servers.length, 5);
  for (const row of manifest.servers) {
    assert.notEqual(row.mode,'bundled');
    assert.deepEqual(row.assetByPlatform, {});
    assert.deepEqual(row.sha256ByPlatform, {});
    assert.equal(row.copyrightReviewed,false);
  }
});
