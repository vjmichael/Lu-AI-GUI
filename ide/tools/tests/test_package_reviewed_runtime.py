"""Reproducible DEV-03 runtime package proofs. All binaries are inert fixtures."""
import importlib.util
import json
from pathlib import Path
import stat
import tempfile
import unittest
import zipfile

SCRIPT = Path(__file__).resolve().parents[1] / "package_reviewed_runtime.py"
spec = importlib.util.spec_from_file_location("package_reviewed_runtime", SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class RuntimePackageTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="dev03-bundle-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / "runtime"
        (self.root / "bin").mkdir(parents=True)
        (self.root / "LICENSES").mkdir()
        self.bin = self.root / "bin/node"
        self.bin.write_bytes(b"FIXTURE: never execute this fake node")
        self.bin.chmod(0o755)
        (self.root / "LICENSES/node.txt").write_text("Fixture attribution\n")

    def package(self, basename="node.zip", expected=None, candidate=True):
        return module.package(
            self.root, Path(self.tmp.name) / basename, "js_tests",
            "linux-x64", expected, candidate,
        )

    def test_exact_candidate_and_approved_pin_are_stable(self):
        files, sha, size = module.stable_files(self.root)
        self.assertEqual(len(files), 2)
        self.assertEqual(len(sha), 64)
        self.assertGreater(size, 0)
        result = self.package()
        self.assertEqual(result["runtimeTreeSha256"], sha)
        self.assertFalse(result["runtimeExecutionEnabled"])
        self.assertEqual(result["approval"], "candidate_not_approved")
        with zipfile.ZipFile(Path(self.tmp.name) / "node.zip") as z:
            receipt = json.loads(z.read("runtime-manifest.json"))
            self.assertEqual(receipt, result)
            self.assertEqual(z.read("runtime/bin/node"), self.bin.read_bytes())
            self.assertIn("runtime/LICENSES/node.txt", z.namelist())
        approved = self.package("approved.zip", sha, False)
        self.assertEqual(approved["approval"], "pin_matched_not_os_accepted")
        self.assertFalse(approved["installedAppAccepted"])

    def test_bad_pin_is_denied_before_any_output(self):
        with self.assertRaises(module.Refused):
            self.package("bad.zip", "a"*64, False)
        self.assertFalse((Path(self.tmp.name) / "bad.zip").exists())
        with self.assertRaises(module.Refused):
            self.package("zero.zip", "0"*64, False)
        with self.assertRaises(module.Refused):
            self.package("implicit.zip", None, False)

    def test_missing_license_and_fake_profile_are_denied(self):
        (self.root / "LICENSES/node.txt").unlink()
        with self.assertRaises(module.Refused):
            self.package()
        with self.assertRaises(module.Refused):
            module.package(self.root,Path(self.tmp.name)/"forge.zip",
                "npm -c curl", "linux-x64",None,True)

    def test_runtime_symlink_and_hardlink_denied(self):
        external=Path(self.tmp.name)/"outside"
        external.write_text("outside secret")
        link=self.root/"bin/escape"
        try:
            link.symlink_to(external)
        except OSError:
            self.skipTest("symlink unsupported")
        with self.assertRaises(module.Refused):
            self.package()
        link.unlink()
        alias=self.root/"bin/alias"
        try:
            import os
            os.link(self.bin,alias)
        except OSError:
            self.skipTest("hardlink unsupported")
        with self.assertRaises(module.Refused):
            self.package()

    def test_refuses_symlink_staging_root(self):
        alias=Path(self.tmp.name)/"alias"
        try:
            alias.symlink_to(self.root, target_is_directory=True)
        except OSError:
            self.skipTest("symlink unsupported")
        with self.assertRaises(module.Refused):
            module.package(alias,Path(self.tmp.name)/"alias.zip",
                "js_tests","linux-x64",None,True)

    def test_path_traversal_controls_and_file_budget(self):
        self.assertFalse(module.safe_part("../secret"))
        self.assertFalse(module.safe_part("bad\\name"))
        self.assertFalse(module.safe_part("bad:name"))
        self.assertFalse(module.safe_part("bad\nname"))
        self.assertFalse(module.safe_part(".."))
        self.assertTrue(module.safe_part("über.py"))
        too_large=self.root/"bin/huge"
        with too_large.open("wb") as file:
            file.truncate(module.MAX_FILE+1)
        with self.assertRaises(module.Refused):
            self.package()

    def test_windows_reserved_paths_denied_on_linux_too(self):
        for part in ("CON", "con.txt", "Lpt1.log", "NUL", "AUX.dat", "com9"):
            self.assertFalse(module.safe_part(part), part)
        self.assertTrue(module.safe_part("console.txt"))
        reserved = self.root / "bin" / "CON.txt"
        reserved.write_bytes(b"not portable")
        with self.assertRaises(module.Refused):
            self.package("reserved.zip")
        self.assertFalse((Path(self.tmp.name) / "reserved.zip").exists())

    def test_case_and_unicode_aliases_are_refused(self):
        path = self.root / "bin" / "Node"
        path.write_bytes(b"alias")
        with self.assertRaises(module.Refused):
            self.package("case.zip")
        path.unlink()
        composed = self.root / "bin" / "caf\\u00e9"
        decomposed = self.root / "bin" / "cafe\\u0301"
        if composed == decomposed:
            self.skipTest("filesystem normalizes unicode filenames")
        composed.write_bytes(b"composed")
        decomposed.write_bytes(b"decomposed")
        with self.assertRaises(module.Refused):
            self.package("unicode.zip")

    def test_does_not_overwrite_archive_or_partial(self):
        self.package()
        with self.assertRaises(module.Refused):
            self.package()
        self.assertTrue((Path(self.tmp.name)/"node.zip").exists())
        partial=Path(self.tmp.name)/"other.zip.partial"
        partial.write_bytes(b"existing")
        with self.assertRaises(module.Refused):
            self.package("other.zip")
        self.assertEqual(partial.read_bytes(),b"existing")


if __name__ == "__main__":
    unittest.main()
