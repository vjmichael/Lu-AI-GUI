import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const core=readFileSync(new URL('../../src-tauri/lu-core/src/ai_operations.rs',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const workspace=readFileSync(new URL('../src-tauri/src/workspace_desktop.rs',import.meta.url),'utf8');
test('P03 routes from current user and already authorized read catalog only',()=>{
  assert.match(core,/pub fn recommended_read_tools/);
  assert.match(core,/effect=="read_workspace"/);
  assert.match(core,/user_declines_tools/);
  assert.match(workspace,/recommended_read_tools\(&prompt,&available\)/);
  assert.match(workspace,/let mut turn=if let Some\(exe\)/);
  assert.match(workspace,/run_journaled_observed/);
  assert.doesNotMatch(core,/std::process::Command/);
});
test('P03 source reuse requires fresh exact chat session epoch and hash',()=>{
  assert.match(core,/struct SourceReuseIndex/);
  assert.match(core,/retrieve_if_fresh/);
  assert.match(core,/source_epoch!=fresh_epoch/);
  assert.match(core,/source_sha256!=freshly_verified_sha256/);
  assert.match(core,/MAX_SOURCES:usize=64/);
});
test('P03 model inspection does not certify compatibility or invent GPU proof',()=>{
  assert.match(host,/fn lu_model_preflight\(/);
  assert.match(host,/fn inspect_registered_model\(/);
  assert.match(host,/ModelAdmissionManager::new\(\)/);
  assert.match(host,/projector\.recheck\(\)/);
  assert.match(host,/runtime_compatible:false/);
  assert.match(host,/gguf_metadata_summary::inspect_metadata/);
  assert.match(ui,/modelInspection\.reportedContextLength/);
  assert.match(host,/provider_download_enabled:false/);
  assert.match(host,/if !inspection\.load_allowed/);
  assert.match(host,/lu_model_preflight,/);
  assert.match(ui,/invoke\('lu_model_preflight'/);
  assert.match(ui,/modelSearch/);
  assert.match(ui,/modelInspection\?\.loadAllowed/);
  assert.match(ui,/Remote provider search\/download is not enabled/);
});
test('P03 provider review requires immutable revision and verified digest and approval',()=>{
  assert.match(core,/c\.provider!="hugging_face"/);
  assert.match(core,/c\.revision\.len\(\)!=40/);
  assert.match(core,/valid_sha\(c\.sha256\)/);
  assert.match(core,/c\.license_accepted/);
  assert.match(core,/c\.access_approved/);
});

test('P03 Reader reidentification requires a new native picked snapshot and completed tool receipt',()=>{
  const reader=readFileSync(new URL('../src-tauri/src/reader_desktop.rs',import.meta.url),'utf8');
  assert.match(reader,/pick_reader_verified\(&pick_app,8\*1024\*1024\)/);
  assert.match(reader,/reader source changed/i);
  assert.match(reader,/source_reuse\.retrieve_if_fresh/);
  assert.match(reader,/result\.tool_calls==0/);
  assert.match(reader,/host\.source_reuse\.remember_verified/);
  assert.match(reader,/You MUST still call the current Reader tool/);
});
test('P03 public provider metadata search is an explicit read-only Settings action',()=>{
  const provider=readFileSync(new URL('../../src-tauri/lu-core/src/model_provider_catalog.rs',import.meta.url),'utf8');
  const researcher=readFileSync(new URL('../src-tauri/src/research_desktop.rs',import.meta.url),'utf8');
  assert.match(host,/fn lu_model_provider_search\(/);
  assert.match(host,/fn lu_model_provider_review\(/);
  assert.match(host,/fn lu_model_backend_options\(/);
  assert.match(provider,/parse_repo_license/);
  assert.match(provider,/parse_tree/);
  assert.match(provider,/download_available:false/);
  assert.match(provider,/lfs\/sha256/);
  assert.match(researcher,/SecureHttpFetcher/);
  assert.match(researcher,/if !self\.available\(\)/);
  assert.match(ui,/lu_model_provider_search/);
  assert.match(ui,/lu_model_provider_review/);
  assert.match(ui,/In-app downloads are blocked/);
  assert.match(ui,/Model backend availability/);
});
