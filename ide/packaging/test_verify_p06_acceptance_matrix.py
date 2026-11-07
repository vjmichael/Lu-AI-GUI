import copy
import json
from pathlib import Path
import unittest
from verify_p06_acceptance_matrix import check,Denied
DATA=json.loads(Path(__file__).with_name("p06-native-acceptance-matrix.json").read_text(encoding="utf-8"))
class MatrixTests(unittest.TestCase):
    def test_only_source_receipts_accepted(self):
        self.assertFalse(check(DATA)["releaseApproved"])
    def test_forged_release_flag_denied(self):
        v=copy.deepcopy(DATA);v["releaseApproved"]=True
        with self.assertRaisesRegex(Denied,"unverified_release_promotion"):check(v)
    def test_forged_native_pass_denied(self):
        v=copy.deepcopy(DATA);v["phases"][1]["state"]="pass"
        with self.assertRaisesRegex(Denied,"unverified_gate_accepted"):check(v)
    def test_deferred_wo10_scope_denied(self):
        v=copy.deepcopy(DATA);v["phases"][-1]["state"]="accepted"
        with self.assertRaisesRegex(Denied,"unverified_gate_accepted"):check(v)
    def test_unpinned_historical_sha_denied(self):
        v=copy.deepcopy(DATA);v["phases"][3]["receipt"]["headSha"]="0"*40
        with self.assertRaisesRegex(Denied,"historical_source_evidence_missing"):check(v)
if __name__=="__main__":unittest.main()
