"""WO7 candidate testing may precede release approval, but cannot promote it."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from verify_wo7_candidate import audit_candidate, Denied


class CandidateTests(unittest.TestCase):
    def setUp(self):
        tmp=tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root=Path(tmp.name)
        self.manifest=self.root/"candidate.json"
        self.source="a"*40
        self.data={"schema":"lu.ide.wo7.signed-nsis.v1","releaseApproved":False,
                   "sourceSha":self.source,"licensesReviewed":True}
        for name in ("installer","service"):
            target=self.root/(name+".exe")
            target.write_bytes(name.encode())
            self.data[name]={"path":target.name,
                             "sha256":hashlib.sha256(target.read_bytes()).hexdigest(),
                             "signerThumbprint":"b"*40}
        self.save()

    def save(self):
        self.manifest.write_text(json.dumps(self.data),encoding="utf-8")

    def checked(self,signature=None):
        return audit_candidate(self.manifest,self.root,self.source,
                               signature=signature or (lambda path:("Valid","b"*40)))

    def test_real_release_not_implicitly_accepted(self):
        report=self.checked()
        self.assertEqual(report["stage"],"signed_candidate_for_native_installed_testing")
        self.assertFalse(report["releaseAccepted"])
        self.assertFalse(report["protectedRunTestDebugAccepted"])

    def test_rejects_mismatch(self):
        self.data["service"]["sha256"]="0"*64
        self.save()
        with self.assertRaisesRegex(Denied,"candidate_changed_service"):
            self.checked()

    def test_rejects_signer_mismatch(self):
        with self.assertRaisesRegex(Denied,"candidate_signer_or_chain_invalid"):
            self.checked(lambda path:("Valid","c"*40))

    def test_rejects_wrong_source(self):
        self.data["sourceSha"]="f"*40
        self.save()
        with self.assertRaisesRegex(Denied,"candidate_wrong_source_sha"):
            self.checked()

    def test_rejects_missing_notice_review(self):
        self.data["licensesReviewed"]=False
        self.save()
        with self.assertRaisesRegex(Denied,"candidate_licenses_unreviewed"):
            self.checked()


if __name__=="__main__":
    unittest.main()
