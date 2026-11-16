"""Hostile pure-data P05 owner successor tests; no native acceptance claim."""
import unittest
from verify_p05_owner_successor import EXPECTED, Denied, compare_38_unchanged_and_replacements

class OriginalAndOwnerSuccessor(unittest.TestCase):
    def setUp(self):
        self.base={p:values[0] for p,values in EXPECTED.items()}
        self.base.update({f".github/workflows/ide-original-{n:02d}.yml":f"{n:040x}" for n in range(30)})
        self.now=dict(self.base)
        self.now.update({p:values[1] for p,values in EXPECTED.items()})
    def test_owner_successor_not_native_evidence(self):
        result=compare_38_unchanged_and_replacements(self.base,self.now)
        self.assertEqual(result["historicalExactWorkflows"],38)
        self.assertEqual(result["ownerAuthorizedCurrentReplacements"],8)
        self.assertFalse(result["nativeAccepted"])
        self.assertFalse(result["releaseApproved"])
    def test_unknown_ninth_workflow_source_change_rejected(self):
        self.now[".github/workflows/ide-original-01.yml"]="f"*40
        with self.assertRaisesRegex(Denied,"p05_unreviewed_or_unretained"):
            compare_38_unchanged_and_replacements(self.base,self.now)
    def test_missing_workflow_denied(self):
        self.now.pop(".github/workflows/ide-original-03.yml")
        with self.assertRaisesRegex(Denied,"p05_expected_exact_38"):
            compare_38_unchanged_and_replacements(self.base,self.now)
    def test_owner_authorized_source_identity_change_denied(self):
        p=next(iter(EXPECTED))
        self.now[p]="0"*40
        with self.assertRaisesRegex(Denied,"p05_unreviewed_or_unretained"):
            compare_38_unchanged_and_replacements(self.base,self.now)
    def test_historical_pin_mutation_denied(self):
        p=next(iter(EXPECTED))
        self.base[p]="f"*40
        with self.assertRaisesRegex(Denied,"p05_unreviewed_or_unretained"):
            compare_38_unchanged_and_replacements(self.base,self.now)

if __name__=="__main__":
    unittest.main()
