"""Ensure the P05 current-stack diagnostic NEVER authorizes replacement."""
import unittest
from verify_p05_merge02_pending_native_boundaries import EXPECTED, Denied, classify_rows

class PendingP05Tests(unittest.TestCase):
    def setUp(self):
        self.approved = {p: x[0] for p, x in EXPECTED.items()}
        self.approved.update({
            ".github/workflows/ide-original-%02d.yml" % i: "%040x" % i
            for i in range(31)
        })
        self.observed = self.approved.copy()
        self.observed.update({p: x[1] for p, x in EXPECTED.items()})
        self.pending = EXPECTED.copy()

    def test_seven_differences_remain_unapproved(self):
        result = classify_rows(self.approved,self.observed,self.pending)
        self.assertEqual(result["unauthorizedChangedWorkflows"], 7)
        self.assertFalse(result["p05HistoricallyAccepted"])
        self.assertFalse(result["publicationAuthorized"])

    def test_eighth_rewrite_is_denied(self):
        self.observed[".github/workflows/ide-original-01.yml"] = "b" * 40
        with self.assertRaisesRegex(Denied,"p05_unreviewed_historical_source_drift"):
            classify_rows(self.approved,self.observed,self.pending)

    def test_forged_historical_identity_is_denied(self):
        path = next(iter(EXPECTED))
        self.approved[path] = "a" * 40
        with self.assertRaisesRegex(Denied,"p05_unreviewed_historical_source_drift"):
            classify_rows(self.approved,self.observed,self.pending)

    def test_restoration_requires_fresh_review_not_silent_approval(self):
        path = next(iter(EXPECTED))
        self.observed[path] = self.approved[path]
        with self.assertRaisesRegex(Denied,"p05_unreviewed_historical_source_drift"):
            classify_rows(self.approved,self.observed,self.pending)

    def test_missing_or_extra_workflows_denied(self):
        self.observed.pop(next(iter(EXPECTED)))
        with self.assertRaisesRegex(Denied,"p05_missing_original_38_native_workflows"):
            classify_rows(self.approved,self.observed,self.pending)

if __name__ == "__main__":
    unittest.main()
