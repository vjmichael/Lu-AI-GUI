"""P05 hostile GitHub PR consolidation, workflow coverage and WO10 tests."""
import copy
import json
from pathlib import Path
import unittest
from verify_p05_pr_inventory import check_snapshot, check_checkout, Denied

ROOT=Path(__file__).resolve().parents[2]
PACK=Path(__file__).resolve().parent

def documents():
    return (
        json.loads((PACK/"p05-pr-source-audit.json").read_text(encoding="utf-8")),
        json.loads((PACK/"p05-workflow-coverage.json").read_text(encoding="utf-8")),
    )

class CloseoutInventoryTests(unittest.TestCase):
    def setUp(self):
        self.prs,self.wfs=documents()
    def denied(self,mutation,label):
        a=copy.deepcopy(self.prs)
        b=copy.deepcopy(self.wfs)
        mutation(a,b)
        with self.assertRaisesRegex(Denied,label):
            check_snapshot(a,b)
    def test_all_open_pr_and_ci_gate_evidence_is_retained(self):
        actual=check_snapshot(self.prs,self.wfs)
        self.assertEqual(actual["prCount"],30)
        self.assertEqual(actual["workflowCount"],38)
        self.assertEqual(actual["approvedForClosure"],0)
        self.assertFalse(actual["releaseAccepted"])
    def test_current_checkout_proves_original_ci_blobs_and_absent_wo10(self):
        self.assertTrue(check_checkout(ROOT,self.prs,self.wfs))
    def test_cannot_claim_historical_pr_merged_or_closed(self):
        self.denied(lambda a,b:a["prEntries"][0].update(
            disposition="close_superseded",closureAuthorized=True),
            "premature_pr_close")
    def test_b2_buffer_exception_cannot_be_dropped(self):
        self.denied(lambda a,b:a["prEntries"][0].update(missingCandidatePaths=[]),
            "unported_or_deferred_source")
    def test_b3_project_search_source_pin_must_survive(self):
        self.denied(lambda a,b:a["uniqueUnportedBlobPins"].pop("ide/host/src/project_search.rs"),
            "unported_b2_b3_blob")
    def test_wo10_bridge_remains_deferred(self):
        self.denied(lambda a,b:a.update(deferredWo10Pr=75),
            "incomplete_ide_pr_or_deferred_wo10")
    def test_pending_native_ci_not_unilaterally_removed(self):
        self.denied(lambda a,b:b["requiredWorkflows"].pop(),
            "workflow_gate_coverage_incomplete")
    def test_cannot_falsely_assert_native_wo6_is_replaced(self):
        self.denied(lambda a,b:b["requiredWorkflows"][0].update(replacementAccepted=True),
            "missing_or_prematurely_disabled_ci_gate")
    def test_cannot_skip_p03_security_or_signed_release(self):
        self.denied(lambda a,b:b.update(nativeReleaseSecurityClosure="passed"),
            "native_security_workflow_replacement_not_accepted")
    def test_same_manifest_cannot_authorize_release(self):
        self.denied(lambda a,b:a.update(releaseApproved=True),
            "unexpected_release_promotion")
    def test_original_workflow_source_sha_locked(self):
        self.denied(lambda a,b:b.update(sourceCandidateSha="f"*40),
            "cross_inventory_source_sha_drift")
    def test_unrecognized_future_pr_cannot_be_silently_marked_reviewed(self):
        self.denied(lambda a,b:a["prEntries"][0].update(number=999),
            "unknown_or_duplicate_pr")

if __name__=="__main__":
    unittest.main()
