"""DEV-04.5C technical candidate intake tests; never enable native execution."""
from __future__ import annotations

import copy
import hashlib
import os
from pathlib import Path
import tempfile
import unittest

from verify_dev045c_debug_bundle import Denied, audit, SCHEMA, SCOPE

SOURCE = "a" * 40
CANDIDATE = {
    "installerSha256": "b" * 64,
    "desktopSha256": "c" * 64,
    "serviceSha256": "d" * 64,
}


def sha(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


class InstalledProjectDebuggerBundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "Lu IDE"
        self.bundle = self.root / "resources/debug-adapters/lldb-dap"
        (self.bundle / "bin").mkdir(parents=True)
        (self.bundle / "debuggee").mkdir()
        (self.root / "THIRD_PARTY_LICENSES").mkdir()
        self.contents = {
            "bin/lldb-dap.exe": b"MZ\0native lldb candidate",
            "bin/libLLVM.dll": b"MZ\0upstream native runtime",
            "debuggee/wo6-selftest.exe": b"MZ\0fixed native acceptance",
        }
        for name, body in self.contents.items():
            (self.bundle / name).write_bytes(body)
        notice = b"Apache-2.0 WITH LLVM-exception original upstream notice"
        (self.root / "THIRD_PARTY_LICENSES/LLVM-LICENSE.txt").write_bytes(notice)
        self.lock = {
            "schema": SCHEMA, "scope": SCOPE, "sourceSha": SOURCE,
            "releaseApproved": False, "redistributionApproved": False,
            "userSelectedExecutable": False, "platform": "windows-x86_64",
            **CANDIDATE,
            "files": [{"path": name, "sha256": sha(body), "bytes": len(body)}
                      for name, body in self.contents.items()],
            "notice": {"path": "THIRD_PARTY_LICENSES/LLVM-LICENSE.txt",
                       "sha256": sha(notice), "bytes": len(notice)}
        }

    def check(self, lock=None, source=SOURCE, platform="windows-x86_64"):
        return audit(self.root, lock if lock is not None else self.lock,
                     source, platform=platform,
                     installer_sha=CANDIDATE["installerSha256"],
                     desktop_sha=CANDIDATE["desktopSha256"],
                     service_sha=CANDIDATE["serviceSha256"])

    def test_exact_file_hash_and_notice_is_technical_intake_not_release(self):
        record = self.check()
        self.assertEqual(record["filesVerified"], 3)
        self.assertTrue(record["candidateIntegrityVerified"])
        self.assertTrue(record["noticeVerified"])
        self.assertFalse(record["releaseApproved"])
        self.assertFalse(record["installedProjectDebuggerAccepted"])
        self.assertFalse(record["nativeHostTreePinVerified"])
        self.assertFalse(record["osContainmentVerified"])

    def test_source_or_unsigned_scope_cannot_be_reclassified(self):
        with self.assertRaises(Denied):
            self.check(source="b" * 40)
        for key, changed in (("scope", "redistributable"),
                             ("releaseApproved", True),
                             ("redistributionApproved", True),
                             ("userSelectedExecutable", True)):
            candidate = copy.deepcopy(self.lock)
            candidate[key] = changed
            with self.subTest(key=key), self.assertRaises(Denied):
                self.check(lock=candidate)

    def test_review_lock_must_match_exact_signed_candidate_binary_triplet(self):
        for key in CANDIDATE:
            candidate = copy.deepcopy(self.lock)
            candidate[key] = "e" * 64
            with self.subTest(role=key), self.assertRaisesRegex(
                Denied, "signed_candidate_binary_identity_mismatch"
            ):
                self.check(lock=candidate)

    def test_binary_or_transitive_dependency_tamper_denied(self):
        for name in ("bin/lldb-dap.exe", "bin/libLLVM.dll",
                     "debuggee/wo6-selftest.exe"):
            path = self.bundle / name
            original = path.read_bytes()
            path.write_bytes(b"evil")
            with self.subTest(name=name), self.assertRaises(Denied):
                self.check()
            path.write_bytes(original)

    def test_extra_payload_or_missing_file_denied(self):
        evil = self.bundle / "bin/curl.exe"
        evil.write_bytes(b"unreviewed")
        with self.assertRaisesRegex(Denied, "extra_or_missing"):
            self.check()
        evil.unlink()
        (self.bundle / "bin/libLLVM.dll").unlink()
        with self.assertRaises(Denied):
            self.check()

    def test_duplicate_casefold_or_parent_traversal_denied(self):
        candidate = copy.deepcopy(self.lock)
        duplicate = dict(candidate["files"][0])
        duplicate["path"] = "BIN/LLDB-DAP.EXE"
        candidate["files"].append(duplicate)
        with self.assertRaisesRegex(Denied, "case_colliding"):
            self.check(lock=candidate)
        candidate = copy.deepcopy(self.lock)
        candidate["files"][0]["path"] = "../bin/lldb-dap.exe"
        with self.assertRaises(Denied):
            self.check(lock=candidate)

    def test_symlink_and_hardlink_denied(self):
        path = self.bundle / "bin/libLLVM.dll"
        path.unlink()
        other = self.root / "outside"
        other.write_bytes(self.contents["bin/libLLVM.dll"])
        try:
            path.symlink_to(other)
        except OSError:
            self.skipTest("native symlink creation unavailable")
        with self.assertRaises(Denied):
            self.check()
        path.unlink()
        try:
            os.link(other, path)
        except OSError:
            self.skipTest("native hardlink creation unavailable")
        with self.assertRaisesRegex(Denied, "hardlinked"):
            self.check()

    def test_notice_must_be_separately_hash_verified(self):
        path = self.root / "THIRD_PARTY_LICENSES/LLVM-LICENSE.txt"
        path.write_bytes(b"replacement notice")
        with self.assertRaisesRegex(Denied, "notice"):
            self.check()
        candidate = copy.deepcopy(self.lock)
        candidate["notice"]["path"] = "bin/LICENSE"
        with self.assertRaisesRegex(Denied, "licenses"):
            self.check(lock=candidate)

    def test_linux_bundle_requires_real_linux_names(self):
        for name in self.contents:
            (self.bundle / name).unlink()
        data = {"bin/lldb-dap": b"ELF adapter", "debuggee/wo6-selftest": b"ELF fixture"}
        for name, body in data.items():
            (self.bundle / name).write_bytes(body)
        candidate = copy.deepcopy(self.lock)
        candidate["platform"] = "linux-x86_64"
        candidate["files"] = [
            {"path": name, "sha256": sha(body), "bytes": len(body)}
            for name, body in data.items()
        ]
        self.assertEqual(self.check(lock=candidate, platform="linux-x86_64")["filesVerified"], 2)
        with self.assertRaises(Denied):
            self.check(lock=candidate, platform="windows-x86_64")


if __name__ == "__main__":
    unittest.main()
