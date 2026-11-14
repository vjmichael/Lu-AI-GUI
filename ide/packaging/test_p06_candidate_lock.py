"""Unsigned installer cannot silently replace reviewed native Cargo/npm graphs."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from verify_p06_candidate_lock import check,sha,LockDenied
BASE=Path(__file__).with_name("p06-reviewed-lock-graphs.json")
class CandidateLockTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)
        self.r=copy.deepcopy(json.loads(BASE.read_text(encoding="utf-8")))
        self.cargo=(self.root/"ide/app/src-tauri/Cargo.lock")
        self.npm=(self.root/"ide/app/package-lock.json")
        self.cargo.parent.mkdir(parents=True)
        self.cargo_bytes=('version = 4\n'+''.join(
            f'[[package]]\nname = "{x}"\nversion = "2.12.5"\n'
            for x in ("tauri","tauri-runtime","tauri-runtime-wry"))+
            ''.join('[[package]]\nname="crate-%d"\nversion="1.0.0"\n'%i for i in range(497))).encode()
        self.npm_doc={"lockfileVersion":3,"packages":{"":{"name":"fixture"},
                       **{"node_modules/pkg-%d"%i:{"version":"1.0.0"} for i in range(62)}}}
        self.npm_bytes=json.dumps(self.npm_doc).encode()
        self.cargo.write_bytes(self.cargo_bytes)
        self.npm.write_bytes(self.npm_bytes)
        self.r["cargoRawSha256"]=sha(self.cargo_bytes)
        self.r["npmPlatformRawSha256"]={"windows-2022":sha(self.npm_bytes),"ubuntu-24.04":sha(self.npm_bytes)}
        self.r["npmCanonicalJsonSha256"]=sha(json.dumps(self.npm_doc,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode())
    def test_matching_native_hashes_only_unsigned(self):
        a=check(self.root,"windows-2022",self.r)
        self.assertTrue(a["resolvedGraphEqualsCrossOSReview"])
        self.assertTrue(a["sourceLockfilesCommitted"])
        self.assertFalse(a["releaseApproved"])
    def test_cargo_runtime_drift_denied(self):
        self.cargo.write_bytes(self.cargo_bytes.replace(b"2.12.5",b"2.13.0",1))
        with self.assertRaisesRegex(LockDenied,"candidate_cargo_lock_not_reviewed"):
            check(self.root,"windows-2022",self.r)
    def test_package_integrity_hash_drift_denied(self):
        self.npm.write_bytes(self.npm_bytes+b"\n")
        with self.assertRaisesRegex(LockDenied,"candidate_os_npm_lock_not_reviewed"):
            check(self.root,"windows-2022",self.r)
    def test_unreviewed_other_os_denied(self):
        with self.assertRaisesRegex(LockDenied,"unsupported_reviewed_os"):
            check(self.root,"macos-14",self.r)
    def test_manifest_cannot_launder_release(self):
        self.r["releaseApproved"]=True
        with self.assertRaisesRegex(LockDenied,"unreviewed_candidate_graph_manifest"):
            check(self.root,"windows-2022",self.r)
if __name__=="__main__":unittest.main()
