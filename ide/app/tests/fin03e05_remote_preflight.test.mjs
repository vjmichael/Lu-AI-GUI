import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const native = read('../../host/src/github_remote_publication.rs');
const host = read('../../host/src/lib.rs');
const tauri = read('../src-tauri/src/lib.rs');

test('FIN03 6E05A remains native registered-project preview only', () => {
  assert.match(host, /pub mod github_remote_publication;/);
  assert.match(native, /pub fn review_github_remote_publication\(/);
  assert.match(native, /exact_repo\(self,project_id\)/);
  assert.match(native, /github_repo/);
  assert.match(native, /preview_evidence_sha256/);
  assert.match(native, /tracking_source:"local_stored_ref_only_not_live_remote"/);
  assert.match(native, /source_rechecked:true/);
  assert.doesNotMatch(tauri, /ide_github_remote_publication_execute/);
  assert.doesNotMatch(tauri, /ide_github_publish_branch/);
});

test('FIN03 6E05A never converts a cached tracking ref or CI into authority', () => {
  for (const field of [
    'live_remote_ref_verified:false',
    'branch_rules_complete:false',
    'remote_credentials_verified:false',
    'native_one_use_approval_granted:false',
    'fsynced_remote_action_journal:false',
    'remote_write_route_available:false',
    'remote_mutation_performed:false',
    'git_process_started:false',
    'network_queried:false',
    'release_accepted:false',
  ]) {
    assert.ok(native.includes(field), 'missing hardcoded deny: '+field);
  }
  assert.match(native, /live_remote_ref_compare_and_swap_missing/);
  assert.match(native, /origin_push_url_override_denied/);
  assert.match(native, /foreign_git_lock_present/);
  assert.match(native, /local_descendant_of_stored_tracking/);
  assert.doesNotMatch(native, /\.push\s*\(/);
  assert.doesNotMatch(native, /\.push_to\s*\(/);
  assert.doesNotMatch(native, /\.set_target\s*\(/);
  assert.doesNotMatch(native, /\.remote_set_url\s*\(/);
  assert.doesNotMatch(native, /Command\s*::\s*new\s*\(/);
  assert.doesNotMatch(native, /ureq\s*::/);
});

test('FIN03 native unit fixtures reject dirty, drift, spoofing and write assertions', () => {
  for (const testName of [
    'immutable_local_publication_preview_cannot_mint_remote_authority',
    'uncommitted_and_foreign_lock_are_never_publishable',
    'different_local_commit_is_only_cached_fast_forward_not_live_cas',
    'spoofed_origin_and_push_url_override_are_denied',
  ]) {
    assert.ok(native.includes('fn '+testName+'('),testName);
  }
  assert.match(native,/FIN03_REMOTE_SOURCE_DRIFT_DURING_PREVIEW/);
  assert.match(native,/FIN03_REMOTE_ORIGIN_NOT_CANONICAL_GITHUB/);
  assert.match(native,/FIN03_REMOTE_TRACKING_UNVERIFIED/);
  assert.match(native,/remote_write_route_available:false/);
});
