"""Pinned graph equivalence: CRLF difference is not version drift."""
import copy
import hashlib
import json
from pathlib import Path
import unittest
from verify_p06_lock_parity import verify,Denied,sha
REVIEW=json.loads(Path(__file__).with_name("p06-reviewed-lock-graphs.json").read_text(encoding="utf-8"))
class ParityTests(unittest.TestCase):
    def setUp(self):
        self.cargo=b"Cargo locked graph test bytes"
        self.doc={"packages":{"a":{"version":"1.0.0"}}}
        self.npm=json.dumps(self.doc).encode()
        self.review=copy.deepcopy(REVIEW)
        self.review["cargoRawSha256"]=sha(self.cargo)
        self.review["npmCanonicalJsonSha256"]=sha(json.dumps(self.doc,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode())
        self.review["npmPlatformRawSha256"]={"windows-2022":sha(self.npm),"ubuntu-24.04":sha(self.npm)}
        self.evidence={"schema":"lu.ide.p06.resolved-dependency-evidence.v1","rustTauri":"=2.12.5",
            "javascriptTauri":"2.12.4","upstreamSourcesVerified":True,
            "releaseApproved":False,"lockfilesCommittedAndReviewed":True,
            "unresolvedLicenseEntries":4,
            "cargoLockSha256":sha(self.cargo),"npmLockSha256":sha(self.npm),
            "cargoPackages":500,"npmPackages":62,"sourceSha":"a"*40}
    def test_hash_bound_graph_not_release(self):
        self.assertFalse(verify(self.review,self.evidence,self.npm,self.cargo,"windows-2022")["releaseApproved"])
    def test_cargo_drift_denied(self):
        with self.assertRaisesRegex(Denied,"resolved_cargo_graph_differs"):
            verify(self.review,self.evidence,self.npm,self.cargo+b"x","windows-2022")
    def test_semantic_npm_drift_denied(self):
        changed=b'{"packages":{"a":{"version":"2.0.0"}}}'
        self.review["npmPlatformRawSha256"]["ubuntu-24.04"]=sha(changed)
        self.evidence["npmLockSha256"]=sha(changed)
        with self.assertRaisesRegex(Denied,"npm_semantic_graph_differs"):
            verify(self.review,self.evidence,changed,self.cargo,"ubuntu-24.04")
    def test_unsupported_platform_denied(self):
        with self.assertRaisesRegex(Denied,"unsupported_native_platform"):
            verify(self.review,self.evidence,self.npm,self.cargo,"macos-14")
    def test_laundered_approval_denied(self):
        self.review["releaseApproved"]=True
        with self.assertRaisesRegex(Denied,"untrusted_native_lock_review"):
            verify(self.review,self.evidence,self.npm,self.cargo,"windows-2022")
if __name__=="__main__":unittest.main()
