"""Negative tests for exact B4/B5 current-stack source scope."""
import copy
import unittest
from verify_reviewed_native_boundaries import BASE, SCHEMA, SCOPE, Denied, compare


class ReviewedCurrentNativeBoundaries(unittest.TestCase):
    def setUp(self):
        self.review = {
            "schema": SCHEMA, "baseSha": BASE, "sourceScope": list(SCOPE),
            "originalHistoricalB4AndB5GuardsRetained": True,
            "sourceReviewApprovedForExactListOnly": True,
            "nativeWindowsAccepted": False, "nativeLinuxAccepted": False,
            "installedGuiAccepted": False, "releaseApproved": False,
            "mergeApproved": False,
            "files": [{"path": "ide/supervisor/src/linux_lsp_worker.rs",
                       "sha": "a" * 40,
                       "scope": "reviewed debugger diagnostics without new grant"}],
        }
        self.observed = {"ide/supervisor/src/linux_lsp_worker.rs": "a" * 40}

    def test_only_exact_pinned_source_is_reviewable(self):
        self.assertEqual(compare(self.review, self.observed), 1)

    def test_exact_powershell_diagnostic_is_reviewable_but_not_a_grant(self):
        self.review["files"][0]["path"] = "ide/supervisor/fixtures/debug/Probe-WindowsWo6FileDenial.ps1"
        self.assertEqual(compare(self.review, {"ide/supervisor/fixtures/debug/Probe-WindowsWo6FileDenial.ps1": "a" * 40}), 1)
        self.observed = {"ide/supervisor/fixtures/debug/Probe-WindowsWo6FileDenial.ps1": "b" * 40}
        with self.assertRaisesRegex(Denied, "blob_drift"):
            compare(self.review, self.observed)

    def test_only_exact_provenance_readme_is_reviewable(self):
        path = "ide/packaging/upstream-licenses/README.md"
        self.review["files"][0]["path"] = path
        self.assertEqual(compare(self.review, {path: "a" * 40}), 1)
        self.review["files"][0]["path"] = "ide/packaging/arbitrary.md"
        with self.assertRaises(Denied):
            compare(self.review, {"ide/packaging/arbitrary.md": "a" * 40})

    def test_unknown_native_source_denied(self):
        self.observed["ide/supervisor/src/new_permission.rs"] = "b" * 40
        with self.assertRaisesRegex(Denied, "path_set_drift"):
            compare(self.review, self.observed)

    def test_changed_pinned_native_blob_denied(self):
        self.observed["ide/supervisor/src/linux_lsp_worker.rs"] = "b" * 40
        with self.assertRaisesRegex(Denied, "blob_drift"):
            compare(self.review, self.observed)

    def test_source_review_does_not_assert_native_approval(self):
        self.review["nativeWindowsAccepted"] = True
        with self.assertRaisesRegex(Denied, "forged_native"):
            compare(self.review, self.observed)

    def test_historical_b4_b5_guards_must_remain(self):
        self.review["originalHistoricalB4AndB5GuardsRetained"] = False
        with self.assertRaisesRegex(Denied, "historical_source_boundary"):
            compare(self.review, self.observed)

    def test_untrusted_path_and_duplicate_denied(self):
        self.review["files"][0]["path"] = "ide/supervisor/../secrets.rs"
        with self.assertRaisesRegex(Denied, "unsafe_or_duplicate"):
            compare(self.review, self.observed)

    def test_base_cannot_be_relabelled(self):
        self.review["baseSha"] = "b" * 40
        with self.assertRaisesRegex(Denied, "baseline_changed"):
            compare(self.review, self.observed)


if __name__ == "__main__":
    unittest.main()
