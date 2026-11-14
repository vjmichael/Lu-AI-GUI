import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {
  isHostCpuWorkspaceLookup,isHostCpuChatCountsQuestion,
  isHostCpuChatLookup,isHostModelStatusQuestion
} from '../web/native_intent.mjs';

const read=(rel)=>readFileSync(new URL(rel,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs');
const workspace=read('../src-tauri/src/workspace_desktop.rs');
const router=read('../src-tauri/src/perf01_router.rs');
const scanner=read('../../src-tauri/lu-core/src/native_file_extra.rs');
const nativeFixture=read('../../src-tauri/lu-core/src/tools09_search_adversarial_tests.rs');
const perf=read('../src-tauri/src/perf01_telemetry.rs');
const ui=read('../web/main.jsx');

test('PERF01 P0D strict CPU intents reject model, filesystem root, or broad search by mere text',()=>{
  assert.equal(isHostCpuWorkspaceLookup('Search selected workspace for: needle'),true);
  assert.equal(isHostCpuChatCountsQuestion('Show current chat counts'),true);
  assert.equal(isHostCpuChatLookup('Search this chat for: needle'),true);
  assert.equal(isHostModelStatusQuestion('Show local model status'),true);
  for(const prompt of [
    'Search selected workspace for: ',
    'Search selected workspace for: x',
    'Please search the selected workspace for: needle',
    'Search selected workspace for: <script>',
    'Search selected workspace for: a\nb',
    'Search selected workspace for: '+ 'x'.repeat(129),
    'Search selected workspace for: needle [Attached text file: x]',
  ])assert.equal(isHostCpuWorkspaceLookup(prompt),false,prompt);
  assert.equal(isHostCpuChatCountsQuestion('Show all other chats counts'),false);
});
test('PERF01 P0D native host independently reclassifies current human text first',()=>{
  assert.match(router,/ApprovedWorkspaceLiteral\(String\)/);
  assert.match(router,/CurrentChatCounts/);
  assert.match(router,/input\.strip_prefix\("Search selected workspace for: "\)/);
  assert.match(host,/perf01_router::Route::ApprovedWorkspaceLiteral\(needle\)/);
  assert.match(host,/workspace_desktop::cpu_search_workspace_for_chat\(/);
  assert.match(host,/perf01_router::count_current_chat\(&host\.chats,&chat_id\)/);
  assert.match(ui,/isHostCpuWorkspaceLookup\(prompt\)/);
  assert.match(ui,/isHostCpuChatCountsQuestion\(prompt\)/);
});
test('PERF01 P0D CPU workspace only reads via fresh OS folder picker and same-session fence',()=>{
  assert.match(workspace,/blocking_pick_folder\(\)/);
  assert.match(workspace,/selected\.into_path\(\)/);
  assert.match(workspace,/if !is_real_directory\(&root\)/);
  assert.match(workspace,/now\.session_id!=expected_session\|\|now\.model_ref!=expected_model/);
  assert.match(workspace,/search_host_approved_workspace_cpu\(&root,&literal\)/);
  assert.match(workspace,/host\.chats\.append_user_with_turn_id\(&chat_id,&gid,&prompt\)/);
  assert.match(workspace,/host\.chats\.complete\(&chat_id,&gid\)/);
  assert.match(workspace,/"APPROVED_WORKSPACE_LITERAL"/);
  assert.doesNotMatch(workspace,/std::process::Command::new\(/);
});
test('PERF01 native CPU scanner is the original bounded TOOLS-09 code',()=>{
  assert.match(scanner,/pub fn search_host_approved_workspace_cpu\(/);
  assert.match(scanner,/let result=search\(&root,request\)\?/);
  assert.match(scanner,/const MAX_FILE_BYTES: u64 = 64 \* 1024/);
  assert.match(scanner,/O_DIRECTORY\|O_NOFOLLOW/);
  assert.match(scanner,/selected directory changed during admission/);
  for(const f of [
    'perf01_cpu_workspace_reuses_original_no_follow_search_and_hash',
    'perf01_cpu_workspace_marks_capped_results_partial',
    'perf01_cpu_workspace_rejects_direct_symlink_as_approved_root'
  ])assert.ok(nativeFixture.includes('fn '+f),'missing native fixture '+f);
});
test('PERF01 indexed chat counts are scoped, bounded and do not inspect content',()=>{
  assert.match(router,/chats\.history_window\(chat_id,before,"older",64\)/);
  assert.match(router,/if !page\.has_more/);
  assert.match(router,/PARTIAL_CURSOR_UNAVAILABLE/);
  assert.match(router,/fn bounded_current_chat_count_only_reads_original_active_ANSF_index\(/);
  const countBody=router.split('pub(crate) fn count_current_chat(')[1]
    .split('#[cfg(test)]')[0];
  assert.ok(countBody);
  assert.doesNotMatch(countBody,/chats\.history\(/);
  assert.doesNotMatch(countBody,/original_source_text_page\(/);
});
test('PERF01 model-start events are original host callback deltas, not GPU guesses',()=>{
  assert.match(host,/phase=="engine_spawned"&&pid\.is_some\(\)/);
  assert.match(host,/engine_start_for_load\.fetch_add\(1,Ordering::Relaxed\)/);
  assert.match(host,/observed_engine_starts_before/);
  assert.match(host,/observed_engine_starts_after/);
  assert.match(perf,/observed_engine_starts_during_turn:Option<u64>/);
  assert.match(perf,/observed_engine_starts_since_host_start:Option<u64>/);
  assert.match(perf,/gpu_used_percent:None,cache_hit:None/);
  assert.match(perf,/prefill_tokens_per_second:None,decode_tokens_per_second:None/);
  assert.match(perf,/source_head:source_head\(\)/);
});
