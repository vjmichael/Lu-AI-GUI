"""P05 source/semantic audit guards: historic receipts cannot confer release."""
import copy
import json
from pathlib import Path
import unittest
PACK=Path(__file__).resolve().parent
ROOT=PACK.parents[1]

def load():
    return (json.loads((PACK/"p05-pr-source-audit.json").read_text(encoding="utf-8")),
            json.loads((PACK/"p05-semantic-reconciliation.json").read_text(encoding="utf-8")))

def verify(a,s):
    if s.get("schema")!="lu.ide.p05.semantic-reconciliation.v1" or s.get("sourceCandidateSha")!=a.get("sourceCandidateSha"):
        raise ValueError("semantic_snapshot_mismatch")
    groups=(s["ancestral"],s["divergent"],s["parallelUnported"],[s["excludedBridge"]],s["phaseChain"])
    if list(map(len,groups))!=[15,7,3,1,4]:
        raise ValueError("semantic_group_count_mismatch")
    audited={x["number"]:x["headSha"] for x in a["prEntries"]}
    seen=set()
    for group in groups:
        for item in group:
            n=item["pr"]
            if n in seen or item.get("headSha")!=audited.get(n):
                raise ValueError("semantic_pr_identity_or_duplicate")
            seen.add(n)
    if seen!=set(audited):
        raise ValueError("semantic_pr_unclassified")
    if any(x["aheadBy"]!=0 for x in s["ancestral"]):
        raise ValueError("ancestral_pr_unique_commit_fabrication")
    if any(x.get("nativeEquivalenceAccepted") is not False or x["uniqueCommitsBeyondWo9"]<=0 for x in s["divergent"]):
        raise ValueError("divergent_branch_falsely_accepted")
    if any(x.get("portAccepted") is not False or x.get("disposition")!="retain_for_separate_feature_review" for x in s["parallelUnported"]):
        raise ValueError("unported_parallel_feature_falsely_shipped")
    if s["excludedBridge"]["disposition"]!="do_not_merge_before_lu_core_v1":
        raise ValueError("wo10_falsely_enabled")
    for flag in ("nativeReleaseAccepted","prClosureAuthorized","ciRemovalAuthorized"):
        if s[flag] is not False:
            raise ValueError("premature_release_or_pr_cleanup")
    if len(s["witnesses"])!=8:
        raise ValueError("missing_patch_witnesses")
    for row in s["witnesses"]:
        path=row["path"]
        if not (path.startswith("ide/") or path.startswith(".github/workflows/ide-")):
            raise ValueError("invalid_semantic_witness_path")
        file=ROOT/path
        if file.is_symlink() or not file.is_file():
            raise ValueError("missing_or_linked_semantic_witness")
        body=file.read_text(encoding="utf-8")
        if not all(term in body for term in row["terms"]):
            raise ValueError("missing_semantic_witness")
    return True

class P05SemanticTests(unittest.TestCase):
    def setUp(self):
        self.a,self.s=load()
    def test_complete_exact_historical_classification(self):
        self.assertTrue(verify(self.a,self.s))
    def test_no_forged_ancestry(self):
        changed=copy.deepcopy(self.s);changed["ancestral"][0]["aheadBy"]=2
        with self.assertRaisesRegex(ValueError,"ancestral_pr_unique_commit"):
            verify(self.a,changed)
    def test_unported_find_in_project_not_relabelled_shipped(self):
        changed=copy.deepcopy(self.s);changed["parallelUnported"][-1]["portAccepted"]=True
        with self.assertRaisesRegex(ValueError,"unported_parallel_feature"):
            verify(self.a,changed)
    def test_wo10_cannot_be_unlocked(self):
        changed=copy.deepcopy(self.s);changed["excludedBridge"]["disposition"]="enabled"
        with self.assertRaisesRegex(ValueError,"wo10_falsely_enabled"):
            verify(self.a,changed)
    def test_cannot_delete_native_ci_on_source_evidence(self):
        changed=copy.deepcopy(self.s);changed["ciRemovalAuthorized"]=True
        with self.assertRaisesRegex(ValueError,"premature_release_or_pr_cleanup"):
            verify(self.a,changed)
    def test_missing_real_source_marker_fails(self):
        changed=copy.deepcopy(self.s);changed["witnesses"][0]["terms"]=["not_a_real_capability_assertion"]
        with self.assertRaisesRegex(ValueError,"missing_semantic_witness"):
            verify(self.a,changed)

if __name__=="__main__":
    unittest.main()
