import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, statSync } from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

test('independent desktop package has no Lu desktop or Lu Core runtime dependency', () => {
  const cargo = read('../src-tauri/Cargo.toml');
  assert.match(cargo, /name = "lu-ide-desktop"/);
  assert.match(cargo, /lu-ide-workspace/);
  assert.doesNotMatch(cargo, /lu-core|lu-desktop/);
  assert.match(read('../src-tauri/tauri.conf.json'), /org\.lu\.ide/);
});

test('WebView exposes only project lifecycle, source browse and explicit save commands', () => {
  const web = read('../web/main.jsx');
  const calls = [...web.matchAll(/(?:invoke|operation)\('(ide_[a-z_]+)'/g)].map(v => v[1]);
  assert.ok(calls.includes('ide_list_projects'));
  assert.ok(calls.includes('ide_choose_project'));
  assert.ok(calls.includes('ide_close_project'));
  assert.ok(calls.includes('ide_open_project'));
  assert.ok(calls.includes('ide_list_source_directory'));
  assert.ok(calls.includes('ide_preview_source_file'));
  assert.ok(calls.includes('ide_review_source_change'));
  assert.ok(calls.includes('ide_save_source_file'));
  assert.ok(calls.includes('ide_unregister_project'));

  assert.deepEqual([...new Set(calls)].sort(), [
    'ide_choose_project', 'ide_close_project', 'ide_list_projects',
    'ide_list_source_directory', 'ide_open_project', 'ide_preview_source_file',
    'ide_review_source_change', 'ide_save_source_file', 'ide_unregister_project'
  ]);
  for(const term of ['lu_generate','lu_load_model','shell_exec','exec_process','package_install']) {
    assert.ok(!web.includes(term), term);
  }
});

test('host uses native chooser and does not accept WebView project paths', () => {
  const host = read('../src-tauri/src/lib.rs');
  assert.match(host, /blocking_pick_folder\(\)/);
  assert.match(host, /IdeWorkspaceRegistry::open/);
  assert.match(host, /project_id: String/);
  assert.doesNotMatch(host, /project_path: String|root_path: String|\.command\(/);
});

test('standalone UI build exists and uses no external services', () => {
  const html = read('../dist/index.html');
  assert.match(html, /assets\/app\.js/);
  const js = read('../dist/assets/app.js');
  assert.ok(statSync(new URL('../dist/assets/app.js', import.meta.url)).size > 50000);
  assert.ok(js.includes('ide_choose_project'));
  assert.ok(!html.includes('https://'));
});

test('source explorer displays escaped text only and never executes a project file', () => {
  const web = read('../web/main.jsx');
  const host = read('../../host/src/browser.rs');
  assert.match(web, /<CodeEditor/);
  assert.match(web, /editorState=\{editorStates\.current\.get\(activeTab\.key\)\}/);
  assert.match(web, /entry\.kind === 'directory'/);
  assert.match(web, /entry\.kind === 'file'/);
  assert.doesNotMatch(web, /dangerouslySetInnerHTML|innerHTML|eval\(|new Function\(/);
  assert.match(host, /cap_std/);
  assert.match(host, /MAX_TEXT_BYTES/);
  assert.match(host, /symlink_metadata/);
  assert.doesNotMatch(host, /Command::new|std::process::Command/);
});

test('R3 editor has explicit revision-checked save, dirty buffers and conflict handling', () => {
  const web = read('../web/main.jsx');
  const native = read('../src-tauri/src/lib.rs');
  const editor = read('../web/code_editor.jsx');
  assert.match(web, /expectedRevision: base/);
  assert.match(web, /text: content/);
  assert.match(web, /FILE_CHANGED_ON_DISK/);
  assert.match(web, /Your unsaved edits are preserved/);
  assert.match(web, /window\.confirm/);
  assert.match(web, /key === 's'/);
  assert.match(web, /saveTab\(activeTab\)/);
  assert.match(web, /beforeunload/);
  assert.match(native, /fn ide_save_source_file/);
  assert.match(native, /\.save_source_file\(&project_id, &relative_path, &expected_revision, &text\)/);
  assert.match(editor, /EditorView\.updateListener\.of/);
  assert.match(editor, /basicSetup/);
  assert.match(editor, /rust\(\)/);
  assert.match(editor, /python\(\)/);
  assert.match(editor, /javascript\(/);
  assert.doesNotMatch(editor, /eval\(|Function\(/);
});

test('saved preview includes revision and uses capability-relative atomic replace', () => {
  const browser = read('../../host/src/browser.rs');
  const editor = read('../../host/src/editor.rs');
  assert.match(browser, /pub revision: String/);
  assert.match(browser, /Sha256::digest/);
  assert.match(editor, /create_new\(true\)/);
  assert.match(editor, /file\.sync_all\(\)/);
  assert.match(editor, /parent\.rename\(&temp, &parent, &leaf\)/);
  assert.match(editor, /BrowserError::Conflict/);
  assert.match(editor, /self\.source_capability\(project_id\)/);
  assert.doesNotMatch(editor, /Command::new|std::process::Command/);
});

test('CodeMirror stylesheet uses Tauri nonce without weakening CSP', () => {
  const html = read('../web/index.html');
  const editor = read('../web/code_editor.jsx');
  const config = JSON.parse(read('../src-tauri/tauri.conf.json'));
  assert.match(html, /<style id="lu-ide-editor-style-nonce"><\/style>/);
  assert.match(editor, /getElementById\('lu-ide-editor-style-nonce'\)/);
  assert.match(editor, /EditorView\.cspNonce\.of\(styleNonce\)/);
  assert.match(config.app.security.csp, /style-src 'self'/);
  assert.doesNotMatch(config.app.security.csp, /unsafe-inline|unsafe-eval/);
  assert.doesNotMatch(config.app.security.csp, /script-src[^;]*unsafe/);
});

test('Remove from IDE only delists catalog and confirms disk folders are retained', () => {
  const ui = read('../web/main.jsx');
  const native = read('../src-tauri/src/lib.rs');
  const host = read('../../host/src/lib.rs');
  const schema = read('../../proto/lu/ide/v1/workspace.proto');
  assert.match(ui, /Remove from IDE<\/button>/);
  assert.match(ui, /window\.confirm\(message\)/);
  assert.match(ui, /original project folder and ALL files on disk will be kept/);
  assert.match(ui, /Unsaved edits in its open tabs will be discarded/);
  assert.match(ui, /operation\('ide_unregister_project'/);
  assert.match(native, /fn ide_unregister_project/);
  assert.match(native, /\.unregister\(&project_id\)/);
  assert.match(host, /pub fn unregister\(&self, id: &str\)/);
  assert.match(host, /save_catalog\(&self\.catalog, &next\)\?/);
  assert.match(schema, /UnregisterProject unregister = 7/);
  const method = host.split('pub fn unregister(&self, id: &str)')[1].split('pub fn projects(&self)')[0];
  assert.doesNotMatch(method, /remove_dir|remove_file|remove_dir_all|Command::new/);
});

test('project cards keep normal column width after registrations shrink', () => {
  const css = read('../web/styles.css');
  assert.match(css, /\.projectGrid\s*\{[^}]*grid-template-columns:\s*repeat\(auto-fill,/);
  assert.doesNotMatch(css, /\.projectGrid\s*\{[^}]*grid-template-columns:\s*repeat\(auto-fit,/);
});

test('bounded change reviews are explicit and independent of AI execution', () => {
  const web = read('../web/main.jsx');
  const native = read('../src-tauri/src/lib.rs');
  const host = read('../../host/src/review.rs');
  assert.match(web, /ide_review_source_change/);
  assert.match(web, /Review changes/);
  assert.match(web, /Save reviewed changes/);
  assert.match(web, /review\.truncated/);
  assert.match(web, /tab\.draft !== review\.draft/);
  assert.match(web, /tab\.revision !== review\.expectedRevision/);
  assert.match(web, /saveTab\(tab\)/);
  assert.match(native, /fn ide_review_source_change/);
  assert.match(native, /\.review_source_change\(&project_id, &relative_path, &expected_revision, &proposed_text\)/);
  assert.match(host, /const MAX_REVIEW_LINES: usize = 240/);
  assert.match(host, /const MAX_REVIEW_OUTPUT_BYTES: usize = 32 \* 1024/);
  assert.match(host, /self\.preview_source_file\(project_id, relative_path\)/);
  assert.doesNotMatch(web, /dangerouslySetInnerHTML/);
  assert.doesNotMatch(host, /Command::new|std::process::Command|\.save_source_file\(/);
});

test('R3.4 renders separate diff hunks with old and new line numbers', () => {
  const web = read('../web/main.jsx');
  const styles = read('../web/styles.css');
  const rust = read('../../host/src/review.rs');
  assert.match(web, /review\.hunks\.map/);
  assert.match(web, /hunk\.header/);
  assert.match(web, /line\.oldLine/);
  assert.match(web, /line\.newLine/);
  assert.match(web, /review\.countsExact/);
  assert.match(styles, /\.reviewHunkHeader/);
  assert.match(rust, /TextDiff::from_lines/);
  assert.match(rust, /grouped_ops\(CONTEXT_LINES\)/);
  assert.match(rust, /const MAX_DIFF_INPUT_LINES: usize = 8_000/);
  assert.doesNotMatch(web, /review\.lines\.map/);
});

test('future Lu proposal envelope is untrusted data, never an executable command', () => {
  const host = read('../../host/src/proposal.rs');
  const native = read('../src-tauri/src/lib.rs');
  assert.match(host, /deny_unknown_fields/);
  assert.match(host, /struct ProposalEnvelope/);
  assert.match(host, /claimed_producer/);
  assert.match(host, /schema_version != 1/);
  assert.match(host, /self\.edits\.len\(\) > MAX_EDIT_FILES/);
  assert.match(host, /pub fn review\(\s*&self,\s*registry: &IdeWorkspaceRegistry,?\s*\)/);
  assert.doesNotMatch(host, /\.save_source_file\(|Command::new|std::process::Command/);
  assert.doesNotMatch(native, /fn ide_apply_proposal|fn ide_execute_proposal/);
});

test('successful source saves display amber confirmation without recoloring conflicts', () => {
  const web = read('../web/main.jsx');
  const css = read('../web/styles.css');
  assert.match(web, /hint\.startsWith\('Saved '\) \? ' saveConfirmation' : ''/);
  assert.match(web, /role="status">\{hint\}/);
  assert.match(css, /\.saveConfirmation\s*\{[^}]*color:\s*#ffe48c/);
  assert.match(css, /\.saveConfirmation\s*\{[^}]*background:\s*#332b17/);
  assert.match(css, /\.errorMsg\s*\{[^}]*background:\s*#4c2532/);
});
