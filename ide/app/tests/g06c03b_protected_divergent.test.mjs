import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const host = readFileSync('src-tauri/src/lib.rs', 'utf8');
const ui = readFileSync('web/main.jsx', 'utf8');
const native = readFileSync('../host/src/git_protected_divergent_merge.rs', 'utf8');
const verifier = readFileSync('../host/src/git_divergent_verify.rs', 'utf8');

test('6C03B exact merge uses separate read-only verification and OS authorization', () => {
  assert.match(host, /async fn ide_git_verify_divergent_merge\(/);
  assert.match(host, /async fn ide_git_execute_divergent_merge\(/);
  assert.match(host, /app\.dialog\(\)\.message\(format!\(/);
  assert.match(host, /\.blocking_show\(\)/);
  assert.match(host, /registry\.revalidate_protected_divergent_merge\(/);
  assert.match(host, /registry\.execute_os_approved_divergent_merge\(/);
  assert.match(ui, /invoke\('ide_git_verify_divergent_merge'/);
  assert.match(ui, /invoke\('ide_git_execute_divergent_merge'/);
  assert.match(ui, /conflictFreePreview === true/);
  assert.match(ui, /authorityGranted !== false/);
  assert.match(ui, /resultSha256: candidate\.resultSha256/);
});

test('6C03B merge construction is two-parent, exact CAS and journal-first', () => {
  assert.match(native, /\&\[\&ours, \&theirs\]/);
  assert.match(native, /c\.parent_count\(\) == 2/);
  assert.match(native, /repo\.reference_matching\(/);
  assert.match(native, /append\(\&journal_file, \&j\)\?/);
  assert.match(native, /"checkout_intent"/);
  assert.match(native, /"publishing"/);
  assert.match(native, /copy_object\(\&src_odb/);
  assert.match(native, /automatic_retry: false/);
  assert.match(native, /automatic_rollback: false/);
  assert.match(native, /automatic_lock_deletion: false/);
});

test('6C03B quarantines the merged tree and never authorizes from a preview', () => {
  assert.match(verifier, /ReplaySandbox::open\(\&repo\)/);
  assert.match(verifier, /quarantine\s*\.merge_trees\(/);
  assert.match(verifier, /merged\s*\.write_tree_to\(quarantine\)/);
  assert.match(verifier, /authority_granted: false/);
  assert.match(verifier, /source_objects_written: false/);
  assert.match(native, /"G06C03B_NEW_CONFLICTS"/);
});
