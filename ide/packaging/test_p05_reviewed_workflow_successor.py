"""Original P05 stays red on eight current changes; successor is witness only."""
import copy
import json
from pathlib import Path
import unittest
from audit_p05_reviewed_workflow_successor import REVIEWED, Denied, classify, checkout
from verify_p05_pr_inventory import check_checkout, Denied as HistoricalDenied

ROOT = Path(__file__).resolve().parents[2]
PACK = Path(__file__).resolve().parent


def fixture():
    manifest = json.loads((PACK / "p05-workflow-coverage.json").read_text(encoding="utf-8"))
    observed = {
        row["path"]: REVIEWED[row["path"]][1]
        if row["path"] in REVIEWED else row["blobSha"]
        for row in manifest["requiredWorkflows"]
    }
    return manifest, observed


class OriginalP05VsCurrentSourceTests(unittest.TestCase):
    def test_all_38_original_pins_covered_and_only_eight_scoped_deltas(self):
        original, blobs = fixture()
        report = classify(original, blobs)
        self.assertEqual(report["historicalPinnedWorkflows"], 38)
        self.assertEqual(report["identicalHistoricalWorkflows"], 30)
        self.assertEqual(report["historicallyDivergentWorkflows"], 8)
        self.assertEqual(len(report["pushTriggerReductionAuthorizationRequired"]), 2)
        for name in ("historicalP05ImmutableCheckoutPass", "originalNativeAcceptanceClosed",
                     "signedInstalledReleaseAccepted", "legalAndLicenseReviewApproved",
                     "mainMergeAuthorized", "successorHasAuthorityToOverrideHistoricalP05",
                     "releaseApproved"):
            self.assertIs(report[name], False, name)

    def test_exact_checked_out_source_matches_successor_not_historical(self):
        checked = checkout(ROOT)
        self.assertTrue(checked["reviewedCurrentGitSourceIdentitiesVerified"])
        prs = json.loads((PACK / "p05-pr-source-audit.json").read_text(encoding="utf-8"))
        original, _ = fixture()
        with self.assertRaisesRegex(HistoricalDenied,
                                    "reviewed_native_ci_workflow_changed"):
            check_checkout(ROOT, prs, original)

    def test_unreviewed_ninth_workflow_change_or_missing_workflow_denied(self):
        historical, current = fixture()
        unchanged = next(x for x in current if x not in REVIEWED)
        current[unchanged] = "f" * 40
        with self.assertRaisesRegex(Denied, "unreviewed_successor_workflow_drift"):
            classify(historical, current)
        historical, current = fixture()
        current.pop(unchanged)
        with self.assertRaisesRegex(Denied, "unreviewed_successor_workflow_drift"):
            classify(historical, current)

    def test_changed_successor_blob_or_old_original_pin_denied(self):
        old, now = fixture()
        path = next(iter(REVIEWED))
        now[path] = REVIEWED[path][0]
        with self.assertRaisesRegex(Denied, "unreviewed_successor_workflow_drift"):
            classify(old, now)
        old, now = fixture()
        for row in old["requiredWorkflows"]:
            if row["path"] == path:
                row["blobSha"] = REVIEWED[path][1]
        with self.assertRaisesRegex(Denied, "original_blob_not_immutable"):
            classify(old, now)

    def test_original_historical_pruning_or_release_cannot_self_authorize(self):
        old, now = fixture()
        for field, changed in (
            ("releaseApproved", True), ("nativeReleaseSecurityClosure", "passed")):
            tampered = copy.deepcopy(old)
            tampered[field] = changed
            with self.assertRaisesRegex(Denied, "historical_p05_inventory_scope"):
                classify(tampered, now)
        tampered = copy.deepcopy(old)
        tampered["requiredWorkflows"][0]["prTriggerPruningAuthorized"] = True
        with self.assertRaisesRegex(Denied, "historical_p05_pin_or_policy_changed"):
            classify(tampered, now)


if __name__ == "__main__":
    unittest.main()
