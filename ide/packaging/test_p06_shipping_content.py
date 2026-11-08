"""P06-R2 hostile candidate shipment tree hash tests; no native release claims."""
import copy
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest

from p06_shipping_content import (
    Denied, SCHEMA, capture, verify, _safe_relative, _category,
)

SHA = "d" * 40


class TestPayloadObservation(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="p06-r2-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / "candidate"
        self.root.mkdir()
        (self.root / "bin").mkdir()
        (self.root / "THIRD_PARTY_LICENSES").mkdir()
        (self.root / "bin" / "lu-ide.exe").write_bytes(b"MZ" + b"x" * 900)
        (self.root / "THIRD_PARTY_LICENSES" / "LICENSE.txt").write_bytes(
            b"Original complete notice bytes\n" + b"A" * 180
        )
        (self.root / "README.txt").write_bytes(b"Readme" * 75)
        self.args = (self.root, SHA, "windows-2022", "installed_candidate")

    def test_source_bound_snapshot_has_no_release_or_legal_authority(self):
        data = capture(*self.args)
        self.assertEqual(data["schema"], SCHEMA)
        self.assertEqual(data["sourceSha"], SHA)
        self.assertEqual(data["fileCount"], 3)
        self.assertEqual(data["categories"]["native_binary_candidate"], 1)
        self.assertEqual(data["categories"]["legal_evidence_candidate"], 1)
        self.assertEqual(data["categories"]["other_payload_file"], 1)
        self.assertEqual(len(data["contentTreeSha256"]), 64)
        self.assertFalse(data["nativeInstalledVerified"])
        self.assertFalse(data["packageOriginVerified"])
        self.assertFalse(data["licenseLegalReviewApproved"])
        self.assertFalse(data["protectedExecutionAccepted"])
        self.assertFalse(data["releaseApproved"])
        self.assertTrue(data["publisherReviewDeferredUntilFinalShipSet"])
        self.assertEqual(verify(*self.args[:1], data, *self.args[1:])["fileCount"], 3)

    def test_single_modified_byte_or_new_file_invalidates_snapshot(self):
        before = capture(*self.args)
        (self.root / "README.txt").write_bytes(b"X" + b"Readme" * 75)
        with self.assertRaisesRegex(Denied, "shipment_snapshot_differs"):
            verify(self.root, before, SHA, "windows-2022", "installed_candidate")
        (self.root / "README.txt").write_bytes(b"Readme" * 75)
        (self.root / "rogue.bin").write_bytes(b"executable payload")
        with self.assertRaisesRegex(Denied, "shipment_snapshot_differs"):
            verify(self.root, before, SHA, "windows-2022", "installed_candidate")

    def test_removed_file_invalidates_snapshot(self):
        before = capture(*self.args)
        (self.root / "README.txt").unlink()
        with self.assertRaisesRegex(Denied, "shipment_snapshot_differs"):
            verify(self.root, before, SHA, "windows-2022", "installed_candidate")

    def test_forged_approval_invalidates_claim(self):
        data = capture(*self.args)
        for field in ("releaseApproved", "nativeInstalledVerified",
                      "packageOriginVerified", "licenseLegalReviewApproved",
                      "dependencyShipmentVerified", "protectedExecutionAccepted"):
            candidate = copy.deepcopy(data)
            candidate[field] = True
            with self.subTest(field=field), self.assertRaisesRegex(
                    Denied, "shipment_snapshot_differs"):
                verify(self.root, candidate, SHA, "windows-2022",
                       "installed_candidate")

    def test_fake_sha_platform_or_tree_type_denied(self):
        for sha in ("HEAD", "a" * 39, "A" * 40, ""):
            with self.subTest(sha=sha), self.assertRaisesRegex(Denied, "source_sha"):
                capture(self.root, sha, "windows-2022", "installed_candidate")
        with self.assertRaisesRegex(Denied, "unsupported_native_witness_platform"):
            capture(self.root, SHA, "win-custom", "installed_candidate")
        with self.assertRaisesRegex(Denied, "unsupported_payload_observation"):
            capture(self.root, SHA, "windows-2022", "release_verified")

    def test_rejects_symlink_to_external_directory_and_file(self):
        ext = Path(self.tmp.name) / "secret"
        ext.mkdir()
        (ext / "hidden.txt").write_text("private file", encoding="utf-8")
        link = self.root / "outside"
        try:
            link.symlink_to(ext, target_is_directory=True)
        except (OSError, NotImplementedError):
            self.skipTest("runner cannot create a symlink")
        with self.assertRaisesRegex(Denied, "symlink_or_reparse_entry_denied"):
            capture(*self.args)
        link.unlink()
        link.symlink_to(ext / "hidden.txt")
        with self.assertRaisesRegex(Denied, "symlink_or_reparse_entry_denied"):
            capture(*self.args)
        link.unlink()
        newroot = Path(self.tmp.name) / "symlink-root"
        newroot.symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(Denied, "symlink_or_reparse_entry_denied"):
            capture(newroot, SHA, "windows-2022", "installed_candidate")

    def test_special_files_and_casefold_collisions_fail(self):
        (self.root / "Readme.TXT").write_bytes(b"collision")
        with self.assertRaisesRegex(Denied, "cross_platform_case_collision"):
            capture(*self.args)
        (self.root / "Readme.TXT").unlink()
        (self.root / "EMPTY.txt").touch()
        observed = capture(*self.args)
        self.assertEqual(observed["fileCount"], 4)
        self.assertEqual(next(x for x in observed["files"]
                              if x["path"] == "EMPTY.txt")["bytes"], 0)

    def test_forbid_traversal_control_characters(self):
        for bad in ("", "../etc/passwd", "./bin/lu", "foo//bar",
                    "file\\other", "a/\0b", "a/\nb", "x/" * 26 + "t"):
            with self.subTest(bad=repr(bad)), self.assertRaises(Denied):
                _safe_relative(bad)

    def test_unix_executable_binary_class(self):
        self.assertEqual(_category("usr/bin/lu-ide", b"\x7fELF", os.stat(
            self.root / "bin" / "lu-ide.exe")), "native_binary_candidate")

    def test_canonical_file_list_independent_of_file_creation_order(self):
        before = capture(*self.args)
        test = self.root / "README.txt"
        raw = test.read_bytes()
        test.unlink()
        test.write_bytes(raw)
        after = capture(*self.args)
        self.assertEqual(before["contentTreeSha256"], after["contentTreeSha256"])
        self.assertEqual(before["files"], after["files"])


if __name__ == "__main__":
    unittest.main()
