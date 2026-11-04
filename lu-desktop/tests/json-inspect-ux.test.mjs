import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const ui = readFileSync(new URL('../web/main.jsx', import.meta.url), 'utf8');
const host = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8');
const adapter = readFileSync(new URL('../src-tauri/src/native_host_json.rs', import.meta.url), 'utf8');
const native = readFileSync(new URL('../../src-tauri/lu-core/src/native_json_inspect.rs', import.meta.url), 'utf8');

test('Inspect JSON action uses native OS picker, never WebView-selected pathname', () => {
  assert.match(ui, /onClick=\{inspectSelectedJsonFile\}/);
  assert.match(ui, /invoke\('lu_inspect_json_file', \{chatId: targetId\}\)/);
  assert.match(host, /async fn lu_inspect_json_file\(/);
  assert.match(host, /add_filter\("JSON data", &\["json"\]\)/);
  assert.match(host, /blocking_pick_file\(\)/);
  assert.match(host, /lu_inspect_json_file,/);
  assert.doesNotMatch(ui, /invoke\('lu_inspect_json_file',\s*\{[^}]*path/);
});

test('host picks approved handle then opens task-bound native dispatch and revokes', () => {
  assert.match(adapter, /open_verified_selected_file\(selected_path\)/);
  assert.match(adapter, /register_json_inspection_tool_pinned\(/);
  assert.match(adapter, /chats\.append_user_with_turn_id\(/);
  assert.match(adapter, /authority\.open_user_task\(/);
  assert.match(adapter, /tool_id: TOOL_ID\.into\(\), effect: ToolEffect::ReadWorkspace/);
  assert.match(adapter, /authority\.dispatch_bound_durable\(/);
  assert.match(adapter, /authority\.revoke_task\(&task_id\)/);
  assert.match(adapter, /policy\.revoke_scope\(&scope_id\)/);
  assert.match(adapter, /chats\.append_progress\(chat_id, turn_id, &summary\)/);
  assert.doesNotMatch(adapter, /fs::write|write_all|File::create/);
});

test('native inspector checks bounded Protobuf and unique approved handle without writing', () => {
  assert.match(native, /pub const MAX_JSON_INSPECTION_BYTES: u32 = 512 \* 1024/);
  assert.match(native, /request\.encode_to_vec\(\) != call\.input\.bytes/);
  assert.match(native, /request\.path != pinned\.name/);
  assert.match(native, /duplicate object key/);
  assert.match(native, /Sha256::digest\(&data\)/);
  const production = native.split('#[cfg(test)]')[0];
  assert.doesNotMatch(production, /File::create|fs::write|std::process::Command/);
});
