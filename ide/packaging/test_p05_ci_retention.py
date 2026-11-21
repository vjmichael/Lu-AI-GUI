"""Fail closed on unsafe P05 CI reduction and loss of native/signed checks."""
import copy
import json
from pathlib import Path
import unittest
PACK=Path(__file__).resolve().parent

def verify():
    inventory=json.loads((PACK/"p05-workflow-coverage.json").read_text(encoding="utf-8"))
    disposition=json.loads((PACK/"p05-ci-retention-decision.json").read_text(encoding="utf-8"))
    return check(inventory,disposition)

def check(original,disposition):
    if disposition.get("schema")!="lu.ide.p05.ci-retention-decision.v1":
        raise ValueError("invalid_ci_retention_schema")
    if (disposition.get("auditedSnapshotSha")!=original.get("sourceCandidateSha")
            or disposition.get("auditedWorkflowCount")!=38
            or disposition.get("existingWorkflowDecision")!="retain_all_original_until_exact_head_acceptance"):
        raise ValueError("untrusted_ci_retention_snapshot")
    if (disposition.get("sourceOnlyReviewComplete") is not True
            or disposition.get("workflowTriggerReductionAuthorized") is not False
            or disposition.get("releaseApproved") is not False
            or disposition.get("prClosuresAuthorized")!=0
            or disposition.get("workflowFilesRemoved")!=0):
        raise ValueError("unsafe_ci_or_pr_closure")
    expected={x["path"]:x["blobSha"] for x in original["requiredWorkflows"]}
    reviewed=disposition.get("rows")
    if not isinstance(reviewed,list) or len(reviewed)!=38:
        raise ValueError("ci_workflow_coverage_not_complete")
    got={}
    for x in reviewed:
        if (x["path"] in got or x.get("gitBlobSha")!=expected.get(x["path"])
                or x.get("phaseOwner") not in ("P02","P03","P04")
                or x.get("disposition")!="retain_existing_workflow"
                or not x.get("reason")
                or x.get("installedOrNativeReplacementAccepted") is not False
                or x.get("closureEligible") is not False):
            raise ValueError("unaccepted_ci_replacement_or_blob_drift")
        got[x["path"]]=x["gitBlobSha"]
    if got!=expected:
        raise ValueError("missing_original_ci_evidence")
    for area in ("P02","P03","P04"):
        if area not in disposition.get("upstreamGates",{}):
            raise ValueError("missing_upstream_native_gate")
    return True

class RetainAllPendingNativeWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.original=json.loads((PACK/"p05-workflow-coverage.json").read_text(encoding="utf-8"))
        self.current=json.loads((PACK/"p05-ci-retention-decision.json").read_text(encoding="utf-8"))
    def test_all_thirty_eight_existing_workflows_reviewed_and_retained(self):
        self.assertTrue(check(self.original,self.current))
    def test_no_forged_installed_acceptance(self):
        x=copy.deepcopy(self.current);x["rows"][0]["installedOrNativeReplacementAccepted"]=True
        with self.assertRaisesRegex(ValueError,"unaccepted_ci_replacement"):
            check(self.original,x)
    def test_no_workflow_removed_without_native_parity(self):
        x=copy.deepcopy(self.current);x["workflowFilesRemoved"]=1
        with self.assertRaisesRegex(ValueError,"unsafe_ci_or_pr_closure"):
            check(self.original,x)
    def test_source_only_green_cannot_authorize_closure(self):
        x=copy.deepcopy(self.current);x["prClosuresAuthorized"]=3
        with self.assertRaisesRegex(ValueError,"unsafe_ci_or_pr_closure"):
            check(self.original,x)
    def test_workflow_sha_immutability(self):
        x=copy.deepcopy(self.current);x["rows"][-1]["gitBlobSha"]="f"*40
        with self.assertRaisesRegex(ValueError,"unaccepted_ci_replacement"):
            check(self.original,x)

if __name__=="__main__":
    unittest.main()
