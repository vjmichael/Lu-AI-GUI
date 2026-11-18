import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const core=readFileSync(new URL('../../src-tauri/lu-core/src/native_file_discovery.rs',
  import.meta.url),'utf8');
const bridge=readFileSync(new URL('../../src-tauri/lu-core/src/native_workspace_read_turn.rs',
  import.meta.url),'utf8');
const registry=readFileSync(new URL('../../src-tauri/lu-core/src/lib.rs',
  import.meta.url),'utf8');
const desktop=readFileSync(new URL('../src-tauri/src/workspace_desktop.rs',
  import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');

test('P01 discovery is registered only inside a human-picked read-only turn',()=>{
  assert.match(registry,/pub mod native_file_discovery;/);
  assert.match(bridge,/native_file_discovery::register_discovery_tool/);
  assert.match(bridge,/register_discovery_tool\(&registry,&mut catalog,&root\)/);
  assert.match(bridge,/AuthorizedCapability/);
  assert.match(core,/pub const ID:&str="lu.file_discovery.discover"/);
  assert.match(core,/effect:ToolEffect::ReadWorkspace/);
  assert.match(core,/NativeToolRegistry/);
  assert.match(desktop,/blocking_pick_folder\(\)/);
  assert.match(desktop,/WorkspaceReadTurnSession::new_with_reader_observer\(/);
  assert.match(desktop,/blocking_pick_folder\(\)/);
});

test('P01 discovery index stays bounded, nonpersistent and source-rechecked',()=>{
  assert.match(core,/const MAX_ENTRIES:usize=12_000/);
  assert.match(core,/const MAX_DIRECTORIES:usize=512/);
  assert.match(core,/const MAX_CACHE_ROOTS:usize=4/);
  assert.match(core,/const REFRESH_AFTER:Duration=Duration::from_secs\(15\)/);
  assert.match(core,/const MAX_DISCOVERY_RUNTIME:Duration=Duration::from_secs\(4\)/);
  assert.match(core,/fn safe_components\(/);
  assert.match(core,/fn fresh\(/);
  assert.match(core,/root\.symlink_metadata\(/);
  assert.match(core,/content:Arc<Mutex<TextCache>>/);
  assert.match(core,/Never globally shared or persisted/);
  assert.doesNotMatch(core,/Connection::open\(/);
  assert.match(core,/index_incomplete/);
  assert.match(core,/index_refreshed/);
});

test('P01 query supports filters, case-insensitive names and paged content',()=>{
  for(const prop of ['query','directory','file_suffix','kind',
    'min_bytes','max_bytes','modified_since','limit','refresh',
    'content_query','content_after']){
    assert.ok(core.includes('pub '+prop+':'),prop);
  }
  assert.match(core,/next_content_after/);
  assert.match(core,/MAX_CONTENT_FILES:usize=96/);
  assert.match(core,/MAX_CONTENT_BYTES:u64=4\*1024\*1024/);
  assert.match(core,/content_files_examined/);
  assert.match(core,/filename/);
  assert.match(core,/exact_name/);
  assert.match(desktop,/indexed filename discovery/);
  assert.match(ui,/async function askWorkspaceDirectory\(\)/);
  assert.match(desktop,/explicit_read_workspace_request\(/);
});

test('P01 does not convert cached discovery into read or write authorization',()=>{
  assert.match(core,/A cache entry is never a grant/);
  assert.match(core,/A discovered path is NOT permission to open or modify it/);
  assert.match(core,/DISCOVERY_INVALID_SCOPE/);
  assert.match(core,/source revalidation/);
  assert.match(bridge,/ReadOnlyPolicy/);
});
