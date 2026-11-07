"""Real OS filesystem rename, durable journal and subprocess-crash P04 tests."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from p04_transactional_upgrade import (
    MARKER, Denied, identity, read_journal, recover, upgrade,
)

PROGRAM=Path(__file__).with_name("p04_transactional_upgrade.py")

class NativeUpgradeTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix="lu-p04-native-")
        self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)/"transaction"
        self.root.mkdir()
        (self.root/".p04-fixture").write_text(MARKER,encoding="utf-8")
        (self.root/"workers.json").write_text(
            json.dumps({"activeWorkers":0,"allReconciled":True,"osTreeEmpty":True}),
            encoding="utf-8")
        (self.root/"user-state").mkdir()
        (self.root/"user-state/projects.json").write_bytes(
            b'{"projects":[{"id":"registered-original","source":"untouched"}]}')
        self.user_sha=self.sha(self.root/"user-state/projects.json")
        self.package("current","0.1.0-alpha.1",b"old signed-equivalent fixture")
        self.package("incoming","0.2.0-alpha.1",b"new signed-equivalent fixture")
        self.old_sha=self.sha(self.root/"current/app.bin")
        self.new_sha=self.sha(self.root/"incoming/app.bin")

    @staticmethod
    def sha(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()

    def package(self,slot,version,bytes_):
        folder=self.root/slot
        folder.mkdir()
        (folder/"app.bin").write_bytes(bytes_)
        (folder/"version.json").write_text(json.dumps({
            "schema":"lu.ide.p04.fixture-version.v1",
            "version":version,"sha256":self.sha(folder/"app.bin")
        }),encoding="utf-8")

    def invoke(self,operation,inject=None):
        args=[sys.executable,str(PROGRAM),"--root",str(self.root),
              "--operation",operation]
        if inject:
            args+=["--inject",inject]
        return subprocess.run(args,text=True,capture_output=True,timeout=20)

    def preserved(self,expected_sha):
        self.assertEqual(self.sha(self.root/"current/app.bin"),expected_sha)
        self.assertEqual(self.sha(self.root/"user-state/projects.json"),self.user_sha)

    def test_successful_atomic_version_activation_keeps_prior_for_redundancy(self):
        result=self.invoke("upgrade")
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(read_journal(self.root)["state"],"committed")
        self.preserved(self.new_sha)
        self.assertEqual(self.sha(self.root/"previous/app.bin"),self.old_sha)
        self.assertEqual(self.invoke("recover").returncode,0)
        self.preserved(self.new_sha)

    def test_explicit_revert_of_committed_candidate_restores_previous(self):
        self.assertEqual(self.invoke("upgrade").returncode,0)
        recovered=self.invoke("rollback")
        self.assertEqual(recovered.returncode,0,recovered.stderr)
        self.preserved(self.old_sha)
        self.assertEqual(self.sha(self.root/"rejected/app.bin"),self.new_sha)
        self.assertEqual(read_journal(self.root)["state"],"rolled_back")

    def test_actual_process_crashes_are_recovered_without_python_finally(self):
        for point in ("after_prepare","after_backup_rename",
                      "after_activation_rename","after_validation"):
            with self.subTest(point=point):
                # Test a genuinely terminated child process on a separate
                # disposable directory, not a mocked exception or journal.
                fixture=NativeUpgradeTests("runTest")
                fixture.setUp()
                try:
                    failed=fixture.invoke("upgrade",point)
                    self.assertEqual(failed.returncode,73,(point,failed.stdout,failed.stderr))
                    self.assertEqual(fixture.sha(fixture.root/"user-state/projects.json"),fixture.user_sha)
                    result=fixture.invoke("recover")
                    self.assertEqual(result.returncode,0,(point,result.stdout,result.stderr))
                    fixture.preserved(fixture.old_sha)
                    self.assertEqual(read_journal(fixture.root)["state"],"rolled_back")
                    self.assertNotEqual(fixture.invoke("upgrade").returncode,0)
                finally:
                    fixture.temp.cleanup()

    def test_actual_process_death_during_rollback_resumes_on_recover(self):
        for point in ("after_rollback_journal","after_reject_rename",
                      "after_restore_previous"):
            with self.subTest(point=point):
                fixture=NativeUpgradeTests("runTest")
                fixture.setUp()
                try:
                    self.assertEqual(fixture.invoke("upgrade").returncode,0)
                    self.assertEqual(fixture.sha(fixture.root/"current/app.bin"),fixture.new_sha)
                    killed=fixture.invoke("rollback",point)
                    self.assertEqual(killed.returncode,73,(point,killed.stderr))
                    self.assertEqual(read_journal(fixture.root)["state"],"rolling_back")
                    fixture.preserved(fixture.old_sha) if point=="after_restore_previous" else self.assertEqual(
                        fixture.sha(fixture.root/"user-state/projects.json"),fixture.user_sha)
                    completed=fixture.invoke("recover")
                    self.assertEqual(completed.returncode,0,(point,completed.stdout,completed.stderr))
                    fixture.preserved(fixture.old_sha)
                    self.assertEqual(read_journal(fixture.root)["state"],"rolled_back")
                    self.assertEqual(fixture.invoke("recover").returncode,0)
                    self.assertNotEqual(fixture.invoke("upgrade").returncode,0)
                finally:
                    fixture.temp.cleanup()

    def test_rollback_never_runs_with_active_or_unreconciled_workers(self):
        self.assertEqual(self.invoke("upgrade").returncode,0)
        (self.root/"workers.json").write_text(
            json.dumps({"activeWorkers":1,"allReconciled":False,"osTreeEmpty":False}))
        attempt=self.invoke("rollback","after_rollback_journal")
        self.assertEqual(attempt.returncode,2)
        self.assertIn("workers_not_quiescent_for_upgrade",attempt.stderr)
        self.assertEqual(read_journal(self.root)["state"],"committed")
        self.preserved(self.new_sha)

    def test_normal_recover_denies_injected_rollback_fault(self):
        result=self.invoke("recover","after_restore_previous")
        self.assertEqual(result.returncode,2)
        self.assertIn("recovery_never_accepts_failure_injection",result.stderr)
        self.preserved(self.old_sha)

    def test_crash_after_durable_commit_never_silently_reverts_success(self):
        self.assertEqual(self.invoke("upgrade","after_commit").returncode,73)
        self.assertEqual(self.invoke("recover").returncode,0)
        self.preserved(self.new_sha)

    def test_unreconciled_workers_block_rename(self):
        (self.root/"workers.json").write_text(
            json.dumps({"activeWorkers":1,"allReconciled":False,"osTreeEmpty":False}))
        res=self.invoke("upgrade")
        self.assertEqual(res.returncode,2)
        self.assertIn("workers_not_quiescent",res.stderr)
        self.assertFalse((self.root/"journal.json").exists())
        self.preserved(self.old_sha)

    def test_payload_tamper_blocks_transaction_before_any_mutation(self):
        (self.root/"incoming/app.bin").write_bytes(b"tampered")
        result=self.invoke("upgrade")
        self.assertEqual(result.returncode,2)
        self.assertIn("version_binary_digest_changed",result.stderr)
        self.assertFalse((self.root/"journal.json").exists())
        self.preserved(self.old_sha)

    def test_incomplete_state_missing_previous_does_not_claim_recovery(self):
        self.assertEqual(self.invoke("upgrade","after_activation_rename").returncode,73)
        (self.root/"previous/app.bin").unlink()
        result=self.invoke("recover")
        self.assertEqual(result.returncode,2)
        self.assertTrue("unexpected_version_directory_contents" in result.stderr
                        or "version_directory_missing_or_linked" in result.stderr,
                        result.stderr)
        self.assertNotEqual(read_journal(self.root)["state"],"rolled_back")

    def test_disallows_missing_disposable_root_authorization(self):
        (self.root/".p04-fixture").unlink()
        result=self.invoke("upgrade")
        self.assertEqual(result.returncode,2)
        self.assertIn("missing_explicit_disposable_root_marker",result.stderr)
        self.preserved(self.old_sha)

if __name__=="__main__":
    unittest.main()
