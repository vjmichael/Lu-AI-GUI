import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = relative => readFileSync(new URL(relative, import.meta.url), 'utf8');
const panel = read('../web/ide_workspace.jsx');
const app = read('../web/main.jsx');
const desktop = read('../src-tauri/src/lib.rs');
const adapter = read('../src-tauri/src/ide_workspace.rs');
const schema = read('../../ide/proto/lu/ide/v1/workspace.proto');

test('IDE panel is discoverable without replacing Lu chat', () => {
  assert.match(app, /import \{ IdeWorkspacePanel \}/);
  assert.match(app, /IDE Workspace/);
  assert.match(app, /<IdeWorkspacePanel open=\{ideOpen\}/);
  assert.match(app, /lu_new_chat/);
  assert.match(app, /lu_generate/);
});

test('IDE surface delegates only to scoped lifecycle commands', () => {
  for (const command of [
    'lu_ide_projects', 'lu_ide_select_project',
    'lu_ide_open_project', 'lu_ide_close_project',
  ]) {
    assert.ok(panel.includes(command), command);
    assert.ok(desktop.includes('ide_workspace::' + command), command);
    assert.ok(adapter.includes('fn ' + command + '('), command);
  }
  assert.doesNotMatch(panel, /filesystem|shell_command|execute_tool|invoke\('lu_execute/);
  assert.doesNotMatch(panel, /projectPath|rootPath|absolutePath|directoryPath/);
  assert.match(panel, /projectId: project\.projectId/);
});

test('native picker and protocol never accept a path from JavaScript', () => {
  assert.match(adapter, /blocking_pick_folder\(\)/);
  assert.match(adapter, /NativeDirectoryPicker/);
  assert.match(adapter, /wire::encode_request/);
  assert.match(adapter, /wire::decode_reply/);
  assert.match(desktop, /IdeWorkspaceRegistry::open_with_registry/);
  assert.doesNotMatch(schema, /string (?:root|path|absolute_path|command)\s*=/);
  assert.match(schema, /message ReopenProject \{ string project_id = 1; \}/);
  assert.match(schema, /message CloseProject \{ string project_id = 1; \}/);
});

test('untrusted project execution remains explicitly disabled', () => {
  assert.match(panel, /dependency installation and AI code edits are disabled/);
  assert.match(adapter, /cannot select an arbitrary project path/);
  assert.doesNotMatch(adapter, /Command::new|std::process|spawn_worker|run_command/);
});
