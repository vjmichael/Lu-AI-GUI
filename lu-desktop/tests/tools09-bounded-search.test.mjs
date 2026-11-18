// TOOLS-09 source contract tests. Native Rust/installed OS acceptance separate.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const core=readFileSync(new URL('../../src-tauri/lu-core/src/native_file_extra.rs',import.meta.url),'utf8');
const proto=readFileSync(new URL('../../src-tauri/lu-core/proto/lu/v1/core.proto',import.meta.url),'utf8');
const fixture=readFileSync(new URL('../../src-tauri/lu-core/src/tools09_search_adversarial_tests.rs',import.meta.url),'utf8');

test('TOOLS09 extends the original authenticated native tool not a duplicate raw shell',()=>{
  assert.match(core,/const SEARCH: &str = "lu\.inspect\.search_text"/);
  assert.match(core,/name: "lu_files_search_text"/);
  assert.match(core,/effect: ToolEffect::ReadWorkspace/);
  assert.match(core,/Dir::open_ambient_dir\(&canonical, ambient_authority\(\)\)/);
  assert.match(core,/search_with_cancel\(&root, args,/);
  assert.doesNotMatch(core,/Command::new\(|std::process::Command/);
});
test('TOOLS09 implements strict original-directory and nofollow leaf scope',()=>{
  assert.match(core,/fn open_search_directory\(root: &Dir, folder: &Path\)/);
  assert.match(core,/current\.symlink_metadata\(text\)/);
  assert.match(core,/current\.canonicalize\(text\)/);
  assert.match(core,/O_NOFOLLOW/);
  assert.match(core,/O_DIRECTORY/);
  assert.match(core,/dir\.open_with\(&name,&opts\)/);
  assert.match(core,/same_file_identity\(&meta,&opened\)/);
  assert.match(core,/same_mount\(&mount, &meta\)/);
  assert.match(core,/is_reparse\(&meta\)/);
});
test('TOOLS09 scan remains bounded and cancellation-aware despite spawn_blocking',()=>{
  assert.match(core,/const MAX_DIRECTORIES: u32 = 128/);
  assert.match(core,/const MAX_ENTRIES: usize = 1_024/);
  assert.match(core,/const MAX_SEARCH_BYTES: u64 = 8 \* 1024 \* 1024/);
  assert.match(core,/const MAX_FILE_BYTES: u64 = 64 \* 1024/);
  assert.match(core,/const MAX_SEARCH_OUTPUT_BYTES: usize = 48 \* 1024/);
  assert.match(core,/const MAX_SEARCH_MILLIS: u64 = 3_000/);
  assert.match(core,/struct SearchDropGuard/);
  assert.match(core,/fn drop\(&mut self\)/);
  assert.match(core,/cancelled\(\) \{ return Err\(bad\("CANCELLED"/);
});
test('TOOLS09 output distinguishes real no match from incomplete search',()=>{
  assert.match(proto,/repeated string limitations = 5/);
  assert.match(proto,/uint64 read_bytes = 6/);
  assert.match(proto,/uint32 skipped_entries = 7/);
  assert.match(core,/"status":if result\.truncated \{"PARTIAL"\} else \{"COMPLETE"\}/);
  for(const code of ['FILE_LIMIT','MATCH_LIMIT','RESULT_BYTE_LIMIT','FILE_SIZE_LIMIT',
     'TOTAL_BYTE_LIMIT','DIRECTORY_LIMIT','ENTRY_LIMIT','TIME_BUDGET',
     'INVALID_UTF8','FILE_UNREADABLE','FILE_CHANGED','LINK_EXCLUDED',
     'MOUNT_EXCLUDED','RESULT_PATH_LIMIT']){
    assert.ok(core.includes('"'+code+'"'),'missing explicit partial reason '+code);
  }
});
test('TOOLS09 complete source hits are file-bound SHA-256, not model-authorized paths',()=>{
  assert.match(proto,/string sha256_hex = 4/);
  assert.match(proto,/uint64 size_bytes = 5/);
  assert.match(core,/Sha256::digest\(&bytes\)/);
  assert.match(core,/bytes\.len\(\) as u64 != opened\.len\(\)/);
  assert.match(core,/same_file_identity\(&opened,&after\)/);
  assert.match(core,/content\.chars\(\)\.skip\(start_char\)\.take\(320\)/);
});
test('TOOLS09 has actual native hostile coverage rather than self-attestation',()=>{
  for(const n of [
    'tools09_nested_utf8_filter_and_original_digest_are_stable',
    'tools09_binary_large_and_changed_data_never_produce_complete_no_match',
    'tools09_scope_filter_and_path_attack_are_denied_without_ambient_open',
    'tools09_limit_reasons_and_cooperative_cancel_are_explicit',
    'tools09_entry_and_match_ceiling_honest_partial_markers',
    'tools09_symlinked_external_directory_and_file_are_never_read',
    'tools09_open_directory_handle_survives_ambient_root_replacement',
    'tools09_entry_limit_is_marked_even_when_every_candidate_is_filtered',
    'tools09_long_relative_result_path_is_not_emitted_or_silently_omitted',
    'tools09_late_line_match_contains_the_provenance_phrase'
  ])assert.ok(fixture.includes('fn '+n),'missing native negative test '+n);
});
