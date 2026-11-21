"""Negative tests for exact source-pinned standalone Cargo lock receipts."""
import json
from pathlib import Path
import unittest
from verify_ide_merge02c_native_locks import Denied, PINS, verify_bytes

ROOT = Path(__file__).resolve().parents[2]

class ReviewedNativeLockTests(unittest.TestCase):
    def test_original_windows_linux_exact_lock_bytes_and_package_counts(self):
        for rel, (_, sha256, size, package_count) in PINS.items():
            with self.subTest(path=rel):
                self.assertTrue(verify_bytes((ROOT / rel).read_bytes(), sha256,
                                             size, package_count))

    def test_hostile_byte_mutation_stays_denied(self):
        for rel, (_, sha256, size, package_count) in PINS.items():
            data = (ROOT / rel).read_bytes()
            for tampered in (data + b" ", data.replace(b"\n", b"\r\n", 1),
                             data[:-1]):
                with self.assertRaises(Denied):
                    verify_bytes(tampered, sha256, size, package_count)

    def test_release_flags_remain_explicitly_false(self):
        review = json.loads((ROOT / "ide/packaging/ide-merge02c-native-cargo-lock-receipts.json")
                            .read_text(encoding="utf-8"))
        for key in ("releaseApproved", "legalReviewed", "physicalOsAccepted",
                    "mainMergeAuthorized"):
            self.assertIs(review[key], False)

if __name__ == "__main__":
    unittest.main()
