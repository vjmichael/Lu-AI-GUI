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
  for(const command of ['ide_import_proposal', 'ide_list_proposals', 'ide_review_proposal',
    'ide_decide_proposal', 'ide_stage_proposal', 'ide_discard_proposal']) assert.ok(calls.includes(command), command);

  assert.deepEqual([...new Set(calls)].sort(), [
    'ide_choose_project', 'ide_close_project', 'ide_list_projects',
    'ide_list_source_directory', 'ide_open_project', 'ide_preview_source_file',
    'ide_review_source_change', 'ide_save_source_file', 'ide_unregister_project',
    'ide_import_proposal', 'ide_list_proposals', 'ide_review_proposal',
    'ide_decide_proposal', 'ide_stage_proposal', 'ide_discard_proposal',
    'ide_read_source_span', 'ide_list_capabilities', 'ide_read_project_context', 'ide_capture_project_inventory', 'ide_compare_project_inventory', 'ide_read_git_status', 'ide_compare_live_github', 'ide_read_exact_head_ci', 'ide_record_task_update', 'ide_observe_task_ci', 'ide_prepare_execution_intent', 'ide_decide_execution_intent', 'ide_list_execution_intents', 'ide_read_git_diff', 'ide_read_git_history', 'ide_probe_project_resource',
    'ide_list_language_servers', 'ide_lsp_worker_status',
    'ide_lsp_start_session', 'ide_lsp_change_document',
    'ide_lsp_read_diagnostics', 'ide_lsp_stop_session',
    // WO6 native-only build/test controls are never a Lu capability grant.
    'ide_build_status', 'ide_build_start', 'ide_build_cancel', 'ide_build_reconcile',
    'ide_debug_status', 'ide_debug_run_fixture', 'ide_debug_cancel'
  ].sort());
  // E2's dynamic method string is restricted to the two inspected options.
  assert.match(web, /kind === 'diagnostics' \? 'ide_lsp_read_diagnostics' : 'ide_lsp_feature'/);
  const desktop = read('../src-tauri/src/lib.rs');
  assert.match(desktop, /LSP_PROTECTED_WORKER_UNVERIFIED/);
  assert.match(desktop, /cfg\(not\(all\(windows, feature = "native-lsp"\)\)\)/);
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

test('file change conflict messages use readable language but preserve internal error code checks', () => {
  const web = read('../web/main.jsx');
  assert.match(web, /includes\('FILE_CHANGED_ON_DISK'\)/);
  assert.match(web, /File changed on disk\./);
  assert.doesNotMatch(web, /setProblem\('FILE_CHANGED_ON_DISK:/);
  assert.doesNotMatch(web, /\? 'FILE_CHANGED_ON_DISK:/);
});

test('successful source saves display amber confirmation without recoloring conflicts', () => {
  const web = read('../web/main.jsx');
  const css = read('../web/styles.css');
  assert.match(web, /hint\.startsWith\('Saved '\) \? ' saveConfirmation'/);
  assert.match(web, /className="noticeContent">\{hint\}/);
  assert.match(css, /\.saveConfirmation\s*\{[^}]*color:\s*#ffe48c/);
  assert.match(css, /\.saveConfirmation\s*\{[^}]*background:\s*#332b17/);
  assert.match(css, /\.errorMsg\s*\{[^}]*background:\s*#4c2532/);
});

test('R3.5 standalone proposal inbox requires explicit review and never applies edits', () => {
  const web = read('../web/main.jsx');
  const native = read('../src-tauri/src/lib.rs');
  const inbox = read('../../host/src/inbox.rs');
  const css = read('../web/styles.css');
  assert.match(web, /id: 'proposals'/);
  assert.match(web, /aria-label="Coding proposal inbox"/);
  assert.match(web, /ide_import_proposal/);
  assert.match(web, /ide_review_proposal/);
  assert.match(web, /ide_decide_proposal/);
  assert.match(web, /ide_discard_proposal/);
  assert.match(web, /ide_stage_proposal/);
  assert.match(web, /Proposal staged as editor drafts/);
  assert.match(web, /stageConfirmation/);
  assert.match(web, /noticeDismiss/);
  assert.match(web, /aria-label="Dismiss notification"/);
  assert.match(native, /fn ide_stage_proposal/);
  assert.match(inbox, /pub fn staging/);
  assert.match(web, /Approve for staging only/);
  assert.match(web, /No source files are saved by importing a proposal/);
  assert.match(web, /It does NOT edit, save, or execute any files/);
  assert.match(web, /file\.truncated \|\| !file\.countsExact/);
  assert.match(native, /IdeProposalInbox::default\(\)/);
  assert.match(native, /fn ide_decide_proposal/);
  assert.match(inbox, /const MAX_INBOX_ITEMS: usize = 16/);
  assert.match(inbox, /entry\.status != ReviewStatus::Pending/);
  assert.match(inbox, /review\.truncated \|\| !review\.counts_exact/);
  assert.doesNotMatch(inbox, /\.save_source_file\(|std::process::Command|Command::new/);
  assert.doesNotMatch(native, /fn ide_apply_proposal|fn ide_execute_proposal/);
  assert.match(css, /\.proposalWorkspace/);
});

test('R3.7 in-file find navigates the active unsaved buffer without native IPC', () => {
  const web = read('../web/main.jsx');
  const editor = read('../web/code_editor.jsx');
  const css = read('../web/styles.css');
  assert.match(web, /key === 'f'/);
  assert.match(web, /role="search" aria-label="Find in active file"/);
  assert.match(web, /locateMatches\(activeTab\?\.draft/);
  assert.match(web, /aria-label="Previous match"/);
  assert.match(web, /aria-label="Next match"/);
  assert.match(read('../web/find_replace.js'), /MAX_FIND_MATCHES = 10000/);
  assert.match(editor, /scrollIntoView: true/);
  assert.match(editor, /view\.dispatch/);
  assert.match(css, /\.editorFindBar/);
});

test('Wave A1 copy source reference validates saved revision and selected line', () => {
  const web = read('../web/main.jsx');
  const cm = read('../web/code_editor.jsx');
  const tauri = read('../src-tauri/src/lib.rs');
  assert.match(web, /invoke\('ide_read_source_span'/);
  assert.match(web, /expectedRevision: tab\.revision/);
  assert.match(web, /navigator\.clipboard\.writeText\(reference\)/);
  assert.match(web, /if \(tab\.dirty\)/);
  assert.match(cm, /aria-label="Source code editor"/);
  assert.match(cm, /const head = view\.state\.selection\.main\.head/);
  assert.match(cm, /const line = view\.state\.doc\.lineAt\(head\)/);
  assert.match(cm, /cursorCallback\.current\?\.\(line\.number\)/);
  assert.match(tauri, /fn ide_read_source_span/);
});

test('Wave A2 registry advertises actual host operations without granting execution', () => {
  const registry = read('../../host/src/capabilities.rs');
  const tauri = read('../src-tauri/src/lib.rs');
  const web = read('../web/main.jsx');
  const css = read('../web/styles.css');
  const commands = [...registry.matchAll(/capability!\(\s*"[^"]+"\s*,\s*"(ide_[a-z_]+)"/g)]
    .map(match => match[1]);
  const registered = [...tauri.matchAll(/^\s+(ide_[a-z_]+),?$/gm)].map(match => match[1]);
  assert.equal(commands.length, 31, 'capabilities must include the E1 read-only LSP catalog');
  assert.equal(new Set(commands).size, commands.length);
  const nativeLspCommands = [
    'ide_lsp_worker_status', 'ide_lsp_start_session',
    'ide_lsp_change_document', 'ide_lsp_feature',
    'ide_lsp_read_diagnostics', 'ide_lsp_stop_session',
  ];
  assert.deepEqual(registered.filter(value => value.startsWith('ide_lsp_')).sort(),
    nativeLspCommands.slice().sort());
  const nativeBuildCommands = [
    'ide_build_status', 'ide_build_start', 'ide_build_cancel', 'ide_build_reconcile'
  ];
  assert.deepEqual(registered.filter(value => value.startsWith('ide_build_')).sort(),
    nativeBuildCommands.slice().sort());
  const nativeDebugCommands = ['ide_debug_status', 'ide_debug_run_fixture', 'ide_debug_cancel'];
  assert.deepEqual(registered.filter(value => value.startsWith('ide_debug_')).sort(),
    nativeDebugCommands.slice().sort());
  // Build/test/debug execution is privileged and intentionally NOT a general
  // Lu capability. Each requires a signed/pinned OS-restricted release feature.
  const buildBackend = read('../src-tauri/src/build_backend.rs');
  assert.match(buildBackend, /execute_windows_worker_after_staging_approval/);
  assert.match(buildBackend, /execute_linux_worker/);
  assert.match(buildBackend, /MessageDialogButtons::OkCancelCustom/);
  assert.match(buildBackend, /BUILD_RELEASE_PINS_ABSENT/);
  const debugBackend = read('../src-tauri/src/debug_backend.rs');
  assert.match(debugBackend, /start_windows_dap/);
  assert.match(debugBackend, /start_linux_dap/);
  assert.match(debugBackend, /fixed_installer_selftest_only_not_project_debugger/);
  assert.match(debugBackend, /MessageDialogButtons::OkCancelCustom/);
  const declared = registered.filter(value =>
    value !== 'ide_list_capabilities' &&
    !nativeLspCommands.includes(value) &&
    !nativeBuildCommands.includes(value) && !nativeDebugCommands.includes(value));
  assert.deepEqual(commands.slice().sort(), declared.slice().sort());
  assert.ok(commands.includes('ide_list_language_servers'));
  assert.match(registry, /descriptions_only: true/);
  assert.match(registry, /connected_to_lu: false/);
  assert.match(registry, /"source.save"/);
  assert.match(registry, /"file_write"/);
  assert.match(registry, /"proposal.stage"/);
  assert.match(tauri, /async fn ide_list_capabilities/);
  assert.match(web, /aria-label="IDE capability directory"/);
  assert.match(web, /This directory describes operations/);
  assert.match(css, /\.capabilityCard/);
});

test('Wave A2 exposes typed request/result/error schemas rather than loose fields', () => {
  const catalog = read('../../host/src/capabilities.rs');
  const schemas = JSON.parse(read('../../host/src/capability_schemas.json'));
  const web = read('../web/main.jsx');
  assert.equal(schemas.schemaVersion, 2);
  assert.equal(Object.keys(schemas.operations).length, 31);
  for (const [id, shapes] of Object.entries(schemas.operations)) {
    assert.equal(shapes.request.type, 'object', id);
    assert.equal(shapes.request.additionalProperties, false, id);
    assert.deepEqual(shapes.request.required.sort(), Object.keys(shapes.request.properties).sort(), id);
    assert.ok(shapes.result.type || shapes.result.$ref, id);
  }
  assert.equal(schemas.definitions.SourceSpan.properties.lines.items.$ref, '#/$defs/SourceLine');
  assert.equal(schemas.definitions.ProposalDetail.properties.reviews.items.$ref, '#/$defs/ChangeReview');
  assert.match(catalog, /pub request_schema: Value/);
  assert.match(catalog, /pub result_schema: Value/);
  assert.match(catalog, /pub error_schema: Value/);
  assert.doesNotMatch(catalog, /pub input_fields:/);
  assert.doesNotMatch(catalog, /pub result_type:/);
  assert.match(web, /item\.requestSchema/);
  assert.match(web, /item\.resultSchema/);
});

test('Wave A3 exposes revisioned, read-only continuity with explicitly unknown external state', () => {
  const native = read('../src-tauri/src/lib.rs');
  const host = read('../../host/src/continuity.rs');
  const web = read('../web/main.jsx');
  const schemas = JSON.parse(read('../../host/src/capability_schemas.json'));
  assert.match(native, /async fn ide_read_project_context/);
  assert.match(native, /development-ledger\.jsonl/);
  assert.match(host, /pub fn context\(/);
  assert.match(host, /fn events\(/);
  assert.match(host, /"untrusted_project_text"/);
  assert.match(host, /remote_git_state: "not_queried"/);
  assert.match(host, /local_artifacts: "bounded_scan"/);
  assert.match(web, /aria-label="Project continuity"/);
  assert.match(web, /invoke\('ide_read_project_context'/);
  assert.equal(schemas.operations['continuity.read'].result.$ref, '#/$defs/ProjectContext');
  assert.equal(schemas.definitions.ProjectContext.properties.events.items.$ref, '#/$defs/DevelopmentEvent');
  assert.doesNotMatch(host, /Command::new|std::process::Command/);
});

test('typed schemas have no dangling nested refs or untyped object properties', () => {
  const schema = JSON.parse(read('../../host/src/capability_schemas.json'));
  const definitions = schema.definitions;
  const visit = (node, path) => {
    if (Array.isArray(node)) {
      node.forEach((child, index) => visit(child, path + '[' + index + ']'));
      return;
    }
    if (!node || typeof node !== 'object') return;
    if (node.$ref) {
      assert.match(node.$ref, /^#\/\$defs\/[A-Za-z]+$/);
      const key = node.$ref.split('/').pop();
      assert.ok(definitions[key], path + ': unknown ' + key);
    }
    if (node.type === 'object') {
      assert.equal(node.additionalProperties, false, path);
      for (const prop of Object.values(node.properties ?? {})) {
        assert.ok(prop.type || prop.$ref || prop.anyOf, path);
      }
    }
    for (const [key, child] of Object.entries(node)) {
      if (key !== '$ref') visit(child, path + '.' + key);
    }
  };
  visit(schema.operations, 'operations');
  visit(definitions, 'definitions');
});

test('Wave A2 typed request keys match actual Rust Tauri argument names', () => {
  const native = read('../src-tauri/src/lib.rs');
  const catalog = read('../../host/src/capabilities.rs');
  const schemas = JSON.parse(read('../../host/src/capability_schemas.json'));
  const capabilities = [...catalog.matchAll(/capability!\(\s*"([^"]+)",\s*"(ide_[a-z_]+)"/g)]
    .map(match => ({ id: match[1], command: match[2] }));
  for (const { id, command } of capabilities) {
    const signature = native.match(new RegExp('async fn ' + command + '\\(([\\s\\S]*?)\\) ->'));
    assert.ok(signature, id + ': missing native function');
    const parameters = [...signature[1].matchAll(/^\s*([a-z_]+):\s*(.+?),?\s*$/gm)]
      .map(match => match[1]).filter(name => !['app', 'state', 'registry', 'inbox', 'ledger', 'snapshots', 'intents'].includes(name))
      .map(name => name.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase()))
      .sort();
    const typed = Object.keys(schemas.operations[id].request.properties).sort();
    assert.deepEqual(typed, parameters, id + ': request schema differs from native command');
  }
});

test('A3 performs a bounded, no-execution local artifact and Git reference probe', () => {
  const inventory = read('../../host/src/inventory.rs');
  const continuity = read('../../host/src/continuity.rs');
  const schema = JSON.parse(read('../../host/src/capability_schemas.json'));
  const web = read('../web/main.jsx');
  assert.match(inventory, /const MAX_CANDIDATES: usize = 80/);
  assert.match(inventory, /const MAX_HASH_BYTES: u64 = 64 \* 1024 \* 1024/);
  assert.match(inventory, /source_capability\(project_id\)/);
  assert.match(inventory, /"Delivered"/);
  assert.match(inventory, /"benchmarks"/);
  assert.match(inventory, /"not_queried"/);
  assert.doesNotMatch(inventory, /std::process::Command|Command::new/);
  assert.match(continuity, /inspect_project_evidence\(project_id\)/);
  assert.equal(schema.definitions.ProjectContext.properties.inventory.$ref, '#/$defs/EvidenceInventory');
  assert.match(web, /projectContext\.inventory\.files/);
});

test('A3 snapshot capture and comparison are separate typed operations', () => {
  const native = read('../src-tauri/src/lib.rs');
  const web = read('../web/main.jsx');
  const store = read('../../host/src/inventory_history.rs');
  const schemas = JSON.parse(read('../../host/src/capability_schemas.json'));
  for (const command of ['ide_capture_project_inventory', 'ide_compare_project_inventory']) {
    assert.ok(native.includes('async fn ' + command), command);
    assert.ok(web.includes("invoke('" + command + "'"), command);
  }
  assert.equal(schemas.operations['inventory.capture'].result.$ref, '#/$defs/InventoryBaseline');
  assert.equal(schemas.operations['inventory.compare'].result.$ref, '#/$defs/InventoryComparison');
  assert.match(store, /"missing_in_scanned_scope"/);
  assert.match(store, /"matched_bytes"/);
  assert.match(store, /"changed_bytes"/);
  assert.match(web, /Record evidence snapshot/);
  assert.doesNotMatch(store, /std::process::Command|Command::new/);
});

test('A3 native Git reader surfaces real dirty tree and no executable Git command', () => {
  const source = read('../../host/src/git_status.rs');
  const context = read('../../host/src/continuity.rs');
  const native = read('../../host/src/lib.rs');
  const ui = read('../web/main.jsx');
  const schemas = JSON.parse(read('../../host/src/capability_schemas.json'));
  assert.match(source, /Repository::open\(&root\)/);
  assert.match(source, /repo\.statuses\(Some\(&mut options\)\)/);
  assert.match(source, /repository_outside_selected_root/);
  assert.match(source, /"not_queried"/);
  assert.doesNotMatch(source, /Command::new|std::process::Command/);
  assert.match(context, /inspect_git_worktree\(project_id\)/);
  assert.match(native, /pub mod git_status/);
  assert.match(ui, /projectContext\.gitWorktree\.worktree/);
  assert.equal(schemas.definitions.ProjectContext.properties.gitWorktree.$ref, '#/$defs/GitWorktreeInspection');
  assert.equal(schemas.definitions.GitWorktreeInspection.properties.liveRemote.const, 'not_queried');
});

test('human source control tab uses the native Git status without shell execution', () => {
  const web = read('../web/main.jsx');
  const native = read('../src-tauri/src/lib.rs');
  const schema = JSON.parse(read('../../host/src/capability_schemas.json'));
  assert.match(web, /aria-label="Read-only source control"/);
  assert.match(web, /invoke\('ide_read_git_status'/);
  assert.match(web, /gitStatus\.displayedChanges\.map/);
  assert.match(native, /async fn ide_read_git_status/);
  assert.equal(schema.operations['git.status'].result.$ref, '#/$defs/GitWorktreeInspection');
  assert.doesNotMatch(web, /invoke\('ide_git_push'|invoke\('ide_git_commit'/);
});

test('live GitHub comparison is a separate explicit network-read action', () => {
  const frontend = read('../web/main.jsx');
  const command = read('../src-tauri/src/lib.rs');
  const native = read('../../host/src/github_live.rs');
  const schema = JSON.parse(read('../../host/src/capability_schemas.json'));
  assert.match(frontend, /Check GitHub/);
  assert.match(frontend, /invoke\('ide_compare_live_github'/);
  assert.match(frontend, /aria-label="Live GitHub comparison"/);
  assert.match(command, /async fn ide_compare_live_github/);
  assert.equal(schema.operations['git.compareLive'].result.$ref, '#/$defs/LiveGitHubComparison');
  assert.equal(schema.definitions.LiveGitHubComparison.properties.responseAuthenticated.const, false);
  assert.match(native, /api\.github\.com\/repos\//);
  assert.match(native, /\.redirects\(0\)/);
  assert.match(native, /MAX_RESPONSE_BYTES/);
  assert.match(native, /branch_not_found_or_private/);
  assert.doesNotMatch(native, /Command::new|std::process::Command/);
});

test('Wave B human-owned Find and Replace preserves explicit Save and undo', () => {
  const web = read('../web/main.jsx');
  const editor = read('../web/code_editor.jsx');
  const planner = read('../web/find_replace.js');
  assert.match(web, /key === 'h'/);
  assert.match(web, /aria-label="Replacement text"/);
  assert.match(web, /aria-label="Replace current match"/);
  assert.match(web, /aria-label="Replace all matches"/);
  assert.match(web, /planReplacement\(activeTab\.draft/);
  assert.match(web, /window\.confirm\(/);
  assert.match(web, /Draft only · Undo available · Save required/);
  assert.match(web, /replaceRequest=\{replaceRequest\?\.tabKey/);
  assert.match(editor, /view\.dispatch\(\{ changes, scrollIntoView: true \}\)/);
  assert.match(editor, /expectedDraft/);
  assert.doesNotMatch(planner, /invoke\(|window\.fetch|\.writeText\(/);
});

test('Wave C native development truth: diff/history/probe use typed read-only host calls', () => {
  const ui = read('../web/main.jsx');
  const native = read('../src-tauri/src/lib.rs');
  const git = read('../../host/src/git_evidence.rs');
  const probe = read('../../host/src/resource_probe.rs');
  const schema = JSON.parse(read('../../host/src/capability_schemas.json'));
  for (const command of ['ide_read_git_diff', 'ide_read_git_history',
    'ide_probe_project_resource']) {
    assert.match(native, new RegExp('async fn ' + command));
    assert.ok(ui.includes("invoke('" + command + "'"), command);
  }
  assert.equal(schema.operations['git.diff'].result.$ref, '#/$defs/GitDiffEvidence');
  assert.equal(schema.operations['git.history'].result.$ref, '#/$defs/GitHistoryEvidence');
  assert.equal(schema.operations['resource.probe'].result.$ref, '#/$defs/ProjectResourceProbe');
  assert.equal(schema.operations['git.diff'].request.properties.scope.enum.length, 2);
  assert.match(ui, /aria-label="Local Git patch"/);
  assert.match(ui, /aria-label="Current resource evidence"/);
  assert.match(git, /registered_git_root\(project_id\)/);
  assert.match(git, /MAX_PATCH_BYTES: usize = 64 \* 1024/);
  assert.match(probe, /source_capability\(project_id\)/);
  assert.match(probe, /executable_usable: "not_tested"/);
  assert.doesNotMatch(git + probe, /std::process::Command|Command::new|\.fetch\(/);
});
