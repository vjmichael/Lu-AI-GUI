import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname,resolve} from 'node:path';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const source=path=>readFileSync(resolve(root,path),'utf8');
const contains=(path,checks)=>{
  const text=source(path);
  for(const needle of checks)assert.ok(text.includes(needle),path+': expected original or stronger contract '+needle);
};

test('C1 Observe source retains consent privacy and independent native custody contracts',()=>{
  contains('lu-desktop/src-tauri/src/observe_desktop.rs',[
    'verified_reaped_frame_receipt','deny_observation_network_egress',
    'reconcile_revoked','release_guardian_when_reaped'
  ]);
  contains('src-tauri/lu-core/src/supervised_tool_generation.rs',[
    'malformed_model_reply_receives_host_owned_nonexecuting_feedback'
  ]);
});
test('C1 Reader retains native picker and bounded UTF8 context contracts',()=>{
  contains('lu-desktop/tests/reader-desktop.test.mjs',[
    'native Reader file selection is host-mediated and file path never supplied by WebView',
    'Reader attachments remain in originating chat and respect the 16k prompt cap',
    'Reader attachment context uses the model UTF-8 byte budget, not JS code-unit length',
    'Ask Reader routes exactly one native-picker document through the supervised read-only model turn'
  ]);
  contains('src-tauri/lu-core/src/native_reader_tool.rs',[
    'register_reader_with_host_observer',
    'workspace_reader_does_not_follow_nested_symlink_directories'
  ]);
});
test('C1 Browser keeps reviewed origin custody human challenge and no WebSocket egress',()=>{
  contains('src-tauri/lu-core/src/browser_worker_supervisor.rs',[
    'require_joint_os_owned_node_scope','sec06c_reconcile_previous'
  ]);
  contains('tests/lu-browser-worker.test.mjs',[
    'visible password form triggers human handoff even with neutral page text',
    'browser blocks page WebSockets without connecting to any server',
    'missing WebSocket interception fails before any page read'
  ]);
  contains('src-tauri/lu-core/src/browser_model_codec.rs',[
    'model_read_catalog_excludes_screenshot_bytes_and_interactive_controls'
  ]);
  const manifest=JSON.parse(source('lu-desktop/src-tauri/resources/lu-browser/manifest.json'));
  assert.equal(manifest.status,'not-packaged');
  assert.deepEqual(manifest.files,[]);
  assert.equal(manifest.platform,'none');
});
