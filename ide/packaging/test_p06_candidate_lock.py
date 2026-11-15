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


class Merge02ScopedCargoTests(unittest.TestCase):
    """Original P06 pins stay immutable; only the exact reviewed local delta passes."""

    def setUp(self):
        self.source_root = Path(__file__).resolve().parents[2]
        self.review = json.loads(BASE.read_text(encoding="utf-8"))
        self.successor = json.loads(
            Path(__file__).with_name("ide-merge02-reviewed-lock-successor.json")
            .read_text(encoding="utf-8")
        )
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        for rel in ("ide/app/src-tauri/Cargo.lock", "ide/app/package-lock.json",
                    "ide/app/src-tauri/Cargo.toml", "ide/supervisor/Cargo.toml",
                    "ide/packaging/ide05-reviewed-lock-successor.json"):
            dest = self.root / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes((self.source_root / rel).read_bytes())

    def test_original_baseline_remains_denied_and_scoped_successor_has_no_release(self):
        with self.assertRaisesRegex(LockDenied, "candidate_cargo_lock_not_reviewed"):
            check(self.root, "windows-2022", self.review)
        for os_name in ("windows-2022", "ubuntu-24.04"):
            result = check(self.root, os_name, self.review,
                           ide_merge02_successor=self.successor)
            self.assertTrue(result["ideMerge02ScopedSuccessorAccepted"])
            self.assertFalse(result["releaseApproved"])
            self.assertFalse(result["signedOrLegallyApproved"])

    def test_recomputed_desktop_block_digest_and_65_digit_typo_denial(self):
        import hashlib
        import re
        data = (self.source_root / "ide/app/src-tauri/Cargo.lock").read_bytes()
        blocks = list(re.finditer(
            rb'\n\[\[package\]\]\nname = "lu-ide-desktop"\n.*?(?=\n\[\[package\]\]|\Z)',
            data, re.DOTALL))
        self.assertEqual(len(blocks), 1)
        match = blocks[0]
        desktop = match.group()
        outside = data[:match.start()] + data[match.end():]
        self.assertEqual(len(self.successor["newDesktopBlockSha256"]), 64)
        self.assertEqual(hashlib.sha256(desktop).hexdigest(),
                         self.successor["newDesktopBlockSha256"])
        self.assertEqual(hashlib.sha256(outside).hexdigest(),
                         self.successor["unchangedOutsideDesktopSha256"])
        poisoned = copy.deepcopy(self.successor)
        poisoned["newDesktopBlockSha256"] += "c"
        with self.assertRaisesRegex(LockDenied, "not_independently_pinned"):
            check(self.root, "windows-2022", self.review,
                  ide_merge02_successor=poisoned)

    def test_approval_flip_and_other_source_bytes_remain_denied(self):
        altered = copy.deepcopy(self.successor)
        altered["releaseApproved"] = True
        with self.assertRaisesRegex(LockDenied, "not_independently_pinned"):
            check(self.root, "windows-2022", self.review,
                  ide_merge02_successor=altered)
        target = self.root / "ide/app/src-tauri/Cargo.lock"
        current = target.read_bytes()
        self.assertIn(b'\n "rand",', current)
        target.write_bytes(current.replace(b'\n "rand",', b'', 1))
        with self.assertRaises(LockDenied):
            check(self.root, "windows-2022", self.review,
                  ide_merge02_successor=self.successor)

    def test_manifest_or_npm_mutation_denied(self):
        manifest = self.root / "ide/app/src-tauri/Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace('rand = "0.8"',
                                                           'rand = "1.0"', 1),
            encoding="utf-8")
        with self.assertRaisesRegex(LockDenied, "dependency_unreviewed"):
            check(self.root, "ubuntu-24.04", self.review,
                  ide_merge02_successor=self.successor)
        manifest.write_bytes((self.source_root / "ide/app/src-tauri/Cargo.toml").read_bytes())
        npm = self.root / "ide/app/package-lock.json"
        npm.write_bytes(npm.read_bytes() + b'\n')
        with self.assertRaisesRegex(LockDenied, "candidate_os_npm_lock_not_reviewed"):
            check(self.root, "ubuntu-24.04", self.review,
                  ide_merge02_successor=self.successor)

if __name__=="__main__":unittest.main()
