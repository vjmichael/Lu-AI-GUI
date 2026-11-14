import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const native = read('../../host/src/github_remote_publication.rs');
const production = native.split('#[cfg(test)]')[0];
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
  // A Vec::push of blocker labels is not a Git remote push. Reject an
  // execution call on native Git/HTTP handles rather than every method push.
  assert.doesNotMatch(production, /(?:remote|repository|repo|git_remote)\.push\s*\(/);
  assert.doesNotMatch(production, /\.push_to\s*\(/);
  assert.doesNotMatch(production, /\.set_target\s*\(/);
  assert.doesNotMatch(production, /\.remote_set_url\s*\(/);
  assert.doesNotMatch(production, /Command\s*::\s*new\s*\(/);
  assert.doesNotMatch(production, /ureq\s*::/);
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

test('FIN03 6E05B uses OS native credential read only, not WebView secrets', () => {
  const live = read('../../host/src/github_remote_live_policy.rs');
  const code = live.split('#[cfg(test)]')[0];
  assert.match(host, /pub mod github_remote_live_policy;/);
  assert.match(live, /NativeOsGitHubReadVault/);
  assert.match(live, /keyring::Entry::new\(/);
  assert.match(live, /entry\.get_password\(\)/);
  assert.match(live, /zeroize::Zeroizing::new/);
  assert.match(live, /fin03-native-vault/);
  assert.doesNotMatch(code, /\.set_password\s*\(/);
  assert.doesNotMatch(code, /\.set_secret\s*\(/);
  assert.doesNotMatch(code, /\.delete_credential\s*\(/);
  assert.doesNotMatch(code, /std::env::var\s*\(/);
  assert.doesNotMatch(tauri, /ide_github_read_token|ide_github_publish_branch/);
  for (const fence of [
    'live_ref_compare_and_swap_authorized:false',
    'native_approval_verified:false',
    'remote_mutation_performed:false',
    'remote_write_route_available:false',
  ]) assert.ok(code.includes(fence), fence);
});

test('FIN03 6E05C offline intent remains host-only and never remotely executes', () => {
  const intent = read('../../host/src/github_remote_publication_intent.rs');
  const code = intent.split('#[cfg(test)]')[0];
  assert.match(host, /mod github_remote_publication_intent;/);
  assert.match(code, /pub\(crate\) fn prepare_github_remote_publication_intent\(/);
  assert.match(code, /graph_descendant_of\(to,from\)/);
  assert.match(code, /create_new\(true\)/);
  assert.match(code, /file\.sync_all\(\)/);
  assert.match(code, /reconciliation_required_no_remote_retry/);
  for (const forbidden of [
    /Command\s*::\s*new\(/, /ureq\s*::/,
    /\.push_to\s*\(/, /\.set_target\s*\(/,
    /\.push\s*\(/, /\.set_password\s*\(/,
  ]) assert.doesNotMatch(code, forbidden);
  assert.doesNotMatch(tauri, /ide_github_remote_publish_execute|ide_github_read_token/);
  for (const testName of [
    'offline_intent_requires_real_local_ancestry_without_write_authority',
    'stale_or_moved_snapshot_denies_even_with_claimed_policy',
    'journal_one_use_fsync_then_restart_never_retries',
    'foreign_remote_or_dirty_project_never_prepares_intent',
  ]) assert.ok(intent.includes('fn '+testName+'('), testName);
});

test('Group 6F aggregates four original native recovery inspectors without publishing or retry', () => {
  const code=read('../../host/src/git_security_recovery_matrix.rs');
  const production=code.split('#[cfg(test)]')[0];
  assert.match(host, /pub mod git_security_recovery_matrix;/);
  for (const method of [
    'inspect_reviewed_commit_recovery',
    'inspect_protected_merge_recovery',
    'inspect_protected_divergent_recovery',
    'inspect_protected_replay_recovery',
  ]) assert.ok(production.includes(method),method);
  for (const marker of [
    'G06F_SOURCE_DRIFT_DURING_RECONCILIATION',
    'foreign_git_lock_present:',
    'remote_publication_authorized:false',
    'os_human_approval_granted:false',
    'automatic_retry_authorized:false',
    'automatic_lock_removal_authorized:false',
  ]) assert.ok(production.includes(marker),marker);
  for (const forbidden of [/Command\s*::\s*new\(/,/ureq\s*::/,
      /\.set_target\s*\(/,/\.push_to\s*\(/,
      /\.remove_file\s*\(/,/\.remove_dir\s*\(/]) {
    assert.doesNotMatch(production,forbidden);
  }
});
