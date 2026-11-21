#!/usr/bin/env python3
"""Adversarial real-Git oracle self-tests; no installed UI/human evidence claimed."""
import tempfile
from pathlib import Path
import unittest
from g06b05_git_oracle import init_fixture, snapshot, verify, git, selected, TARGET

class OracleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='g06b05-')
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)

    def setup_repo(self, mode='create', checkpoint='initial', lock=False):
        repo, manifest = self.base / 'repo', self.base / 'baseline.json'
        init_fixture(repo, mode)
        if checkpoint == 'selected':
            oid = git(repo, 'hash-object', '-w', '--stdin', input=selected(mode)).decode().strip()
            git(repo, 'update-index', '--cacheinfo', '100644,' + oid + ',' + TARGET)
        if lock:
            (repo / '.git/index.lock').write_bytes(b'exclusive foreign lock\n')
        snapshot(repo, manifest, mode, checkpoint, lock)
        return repo, manifest

    def test_denial_cancellation_and_interruption_preserve_state(self):
        _, manifest = self.setup_repo()
        for outcome in ('denied', 'cancelled', 'interrupted_prepublication'):
            verify(manifest, outcome)

    def test_foreign_index_lock_preserved(self):
        _, manifest = self.setup_repo(lock=True)
        verify(manifest, 'denied')

    def test_actual_git_commit_exact_selected_hunk(self):
        repo, manifest = self.setup_repo(checkpoint='selected')
        git(repo, '-c', 'commit.gpgsign=false', 'commit', '-qm', 'reviewed commit')
        verify(manifest, 'created')

    def test_actual_git_amend_retains_old_objects_and_parent(self):
        repo, manifest = self.setup_repo(mode='amend', checkpoint='selected')
        git(repo, '-c', 'commit.gpgsign=false', 'commit', '--amend', '-qm', 'reviewed amend')
        verify(manifest, 'amended')

    def test_orphan_object_does_not_falsely_fail_cancel(self):
        repo, manifest = self.setup_repo(checkpoint='selected')
        git(repo, 'hash-object', '-w', '--stdin', input=b'unreachable cancellation candidate')
        verify(manifest, 'cancelled')

    def test_unrelated_worktree_change_denied(self):
        repo, manifest = self.setup_repo(checkpoint='selected')
        (repo / 'unrelated.txt').write_bytes(b'tampered unrelated\n')
        with self.assertRaisesRegex(ValueError, 'worktree_modified'):
            verify(manifest, 'created')

    def test_unrelated_index_change_denied(self):
        repo, manifest = self.setup_repo(checkpoint='selected')
        oid = git(repo, 'hash-object', '-w', '--stdin', input=b'tampered index only\n').decode().strip()
        git(repo, 'update-index', '--cacheinfo', '100644,' + oid + ',unrelated.txt')
        with self.assertRaisesRegex(ValueError, 'index_modified'):
            verify(manifest, 'cancelled')

    def test_unselected_second_hunk_not_accepted(self):
        repo, manifest = self.setup_repo(checkpoint='selected')
        git(repo, 'add', TARGET)
        git(repo, '-c', 'commit.gpgsign=false', 'commit', '-qm', 'unreviewed both hunks')
        with self.assertRaisesRegex(ValueError, 'index_modified'):
            verify(manifest, 'created')

    def test_unexpected_second_commit_denied(self):
        repo, manifest = self.setup_repo(checkpoint='selected')
        git(repo, '-c', 'commit.gpgsign=false', 'commit', '-qm', 'reviewed commit')
        git(repo, '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-qm', 'extra commit')
        with self.assertRaisesRegex(ValueError, 'parent_or_merge_changed'):
            verify(manifest, 'created')

    def test_foreign_lock_deletion_denied(self):
        repo, manifest = self.setup_repo(lock=True)
        (repo / '.git/index.lock').unlink()
        with self.assertRaisesRegex(ValueError, 'foreign_lock_created_removed_or_modified'):
            verify(manifest, 'denied')

    def test_nonfixture_denied(self):
        repo = self.base / 'not-fixture'
        repo.mkdir()
        git(repo, 'init', '--initial-branch=main')
        with self.assertRaisesRegex(ValueError, 'missing_disposable_marker'):
            snapshot(repo, self.base / 'x.json', 'create', 'initial')

    def test_unexpected_new_file_denied(self):
        repo, manifest = self.setup_repo()
        (repo / 'foreign.txt').write_text('unexpected')
        with self.assertRaisesRegex(ValueError, 'worktree_modified'):
            verify(manifest, 'denied')

if __name__ == '__main__':
    unittest.main(verbosity=2)
