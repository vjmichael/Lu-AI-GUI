import copy
import unittest
from verify_p04_transition_evidence import evaluate, Denied, REQUIRED

HEAD = "a" * 40
def record(gate, i):
    os_name, workflow, changing = REQUIRED[gate]
    return {
        "gate": gate, "runnerOs": os_name, "workflow": workflow,
        "sourceSha": HEAD, "result": "passed", "skipped": False,
        "runId": 500+i, "jobId": 800+i,
        "witnessSha256": "1"*64, "oldInstallerSha256": "2"*64,
        "newInstallerSha256": ("3" if changing else "2")*64,
        "catalogBeforeSha256": "4"*64, "catalogAfterSha256": "4"*64,
        "sourceBeforeSha256": "5"*64, "sourceAfterSha256": "5"*64,
        "beforeVersion": "0.1.0",
        "attemptedVersion": ("0.2.0" if changing else "0.1.0"),
        "afterVersion": ("0.2.0" if changing and "rollback" not in gate else "0.1.0"),
        "restoredInstallerSha256": ("2"*64 if "rollback" in gate else None),
        "signedAndReviewed": True, "processTreeEmpty": True,
        "noAutomaticReplay": True, "protectedServiceRemoved": True,
        "failedUpgradeObserved": "rollback" in gate,
        "previousVersionRestored": "rollback" in gate,
    }

def document():
    return {"schema": "lu.ide.p04.transition.v1", "sourceSha": HEAD,
            "releaseApproved": False,
            "observations": [record(gate, i) for i, gate in enumerate(REQUIRED)]}

class TransitionEvidenceTests(unittest.TestCase):
    def assertDenied(self, mutate):
        data = document()
        mutate(data)
        with self.assertRaises(Denied):
            evaluate(data, HEAD)

    def test_complete_metadata_still_does_not_grant_release(self):
        result = evaluate(document(), HEAD)
        self.assertTrue(result["metadataCoverageComplete"])
        self.assertFalse(result["signedReleaseApproved"])
        self.assertFalse(result["independentlyFetchedNativeEvidence"])

    def test_empty_checked_in_evidence_denied(self):
        self.assertDenied(lambda d: d["observations"].clear())

    def test_no_release_promotion_from_json(self):
        self.assertDenied(lambda d: d.update(releaseApproved=True))

    def test_wrong_head_denied(self):
        self.assertDenied(lambda d: d["observations"][0].update(sourceSha="b"*40))

    def test_skipped_job_denied(self):
        self.assertDenied(lambda d: d["observations"][0].update(skipped=True))

    def test_wrong_os_denied(self):
        self.assertDenied(lambda d: d["observations"][0].update(runnerOs="ubuntu-24.04"))

    def test_same_version_falsely_claimed_upgrade_denied(self):
        self.assertDenied(lambda d: d["observations"][1].update(
            newInstallerSha256="2"*64, afterVersion="0.1.0"))

    def test_changed_catalog_denied(self):
        self.assertDenied(lambda d: d["observations"][0].update(catalogAfterSha256="9"*64))

    def test_worker_replay_not_verified_denied(self):
        self.assertDenied(lambda d: d["observations"][0].update(noAutomaticReplay=False))

    def test_rollback_without_actual_failure_denied(self):
        gate = next(i for i, row in enumerate(document()["observations"]) if "rollback" in row["gate"])
        self.assertDenied(lambda d: d["observations"][gate].update(failedUpgradeObserved=False))

    def test_duplicate_gate_denied(self):
        self.assertDenied(lambda d: d["observations"].__setitem__(1, copy.deepcopy(d["observations"][0])))

    def test_rollback_did_not_restore_original_version_denied(self):
        index = next(i for i, row in enumerate(document()["observations"]) if "rollback" in row["gate"])
        self.assertDenied(lambda d: d["observations"][index].update(afterVersion="0.2.0"))

    def test_rollback_signed_candidate_mismatch_denied(self):
        index = next(i for i, row in enumerate(document()["observations"]) if "rollback" in row["gate"])
        self.assertDenied(lambda d: d["observations"][index].update(restoredInstallerSha256="9"*64))

    def test_unreviewed_artifact_denied(self):
        self.assertDenied(lambda d: d["observations"][0].update(signedAndReviewed=False))

if __name__ == "__main__":
    unittest.main()
