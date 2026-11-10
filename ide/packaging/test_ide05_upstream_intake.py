#!/usr/bin/env python3
"""IDE-05.6 security regression tests; no network or real operator required."""
import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
import zipfile

SPEC = importlib.util.spec_from_file_location(
    "ide056", Path(__file__).with_name("verify_ide05_upstream_uv.py"))
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class UpstreamIntakeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.wheel = self.root / "uv-0.9.9-py3-none-manylinux_2_17_x86_64.whl"
        self.binary = self.root / "uv"
        self.license = self.root / "LICENSE-MIT"
        self.binary.write_bytes(b"\x7fELFnative first-party fixture")
        self.license.write_bytes(b"MIT\nCI fixture only\n")
        self.build()
    def build(self, extra=None, link=False):
        with zipfile.ZipFile(self.wheel, "w", compression=zipfile.ZIP_STORED) as z:
            z.writestr("uv/uv", self.binary.read_bytes())
            z.writestr("uv-0.9.9.dist-info/METADATA", "Name: uv\nVersion: 0.9.9\n\n")
            z.writestr("uv-0.9.9.dist-info/licenses/LICENSE-MIT", self.license.read_bytes())
            if extra:
                entry = zipfile.ZipInfo(extra)
                if link:
                    entry.create_system = 3
                    entry.external_attr = (0o120777 << 16)
                z.writestr(entry, "bad")
        buf = self.wheel.read_bytes()
        self.metadata = {
            "info": {"name": "uv", "version": "0.9.9"},
            "urls": [{
                "filename": self.wheel.name, "packagetype": "bdist_wheel",
                "yanked": False, "size": len(buf),
                "url": "https://files.pythonhosted.org/packages/test/" + self.wheel.name,
                "digests": {"sha256": hashlib.sha256(buf).hexdigest()},
            }],
        }
    def check(self):
        return MODULE.verify_pypi_uv_wheel(
            self.metadata, self.wheel, self.binary, self.license)
    def test_known_wheel_archive_binary_license_verified_but_not_promoted(self):
        result = self.check()
        self.assertTrue(result["registryWheelHashAndInstalledBinaryMatched"])
        self.assertFalse(result["upstreamPublisherSignatureVerified"])
        self.assertFalse(result["releaseApproved"])
    def test_original_binary_changed(self):
        self.binary.write_bytes(b"impostor")
        with self.assertRaises(MODULE.Denied):
            self.check()
    def test_archive_hash_changed(self):
        with self.wheel.open("ab") as file:
            file.write(b"x")
        with self.assertRaises(MODULE.Denied):
            self.check()
    def test_missing_registry_or_mismatched_filename(self):
        self.metadata["urls"][0]["filename"] = "other.whl"
        with self.assertRaises(MODULE.Denied):
            self.check()
    def test_yanked_wheel_denied(self):
        self.metadata["urls"][0]["yanked"] = True
        with self.assertRaises(MODULE.Denied):
            self.check()
    def test_source_url_must_be_exact_pypi_origin(self):
        self.metadata["urls"][0]["url"] = "https://example.invalid/" + self.wheel.name
        with self.assertRaises(MODULE.Denied):
            self.check()
    def test_license_bytes_changed(self):
        self.license.write_bytes(b"counterfeit")
        with self.assertRaises(MODULE.Denied):
            self.check()
    def test_zip_traversal_denied(self):
        self.build(extra="../escape")
        with self.assertRaises(MODULE.Denied):
            self.check()
    def test_symlink_denied(self):
        self.build(extra="evil-link", link=True)
        with self.assertRaises(MODULE.Denied):
            self.check()
    def test_installed_binary_and_license_aliases_are_denied(self):
        original_binary = self.root / "native-elf"
        original_binary.write_bytes(self.binary.read_bytes())
        self.binary.unlink()
        self.binary.symlink_to(original_binary)
        with self.assertRaises(MODULE.Denied):
            self.check()
        self.binary.unlink()
        self.binary.write_bytes(original_binary.read_bytes())
        original_license = self.root / "original-license"
        original_license.write_bytes(self.license.read_bytes())
        self.license.unlink()
        self.license.symlink_to(original_license)
        with self.assertRaises(MODULE.Denied):
            self.check()

    def test_original_file_handle_replacement_is_denied(self):
        # A path swapped while the read is underway cannot supply bytes
        # from a different inode even if its length and content match.
        from unittest import mock
        original_open = MODULE.os.open
        observed = []
        def replaced(path, flags, *args, **kwargs):
            fd = original_open(path, flags, *args, **kwargs)
            if Path(path) == self.binary and not observed:
                observed.append(True)
                replacement = self.root / "replacement"
                replacement.write_bytes(self.binary.read_bytes())
                replacement.replace(self.binary)
            return fd
        with mock.patch.object(MODULE.os, "open", side_effect=replaced):
            with self.assertRaises(MODULE.Denied):
                self.check()

    def test_wheel_path_symlink_denied(self):
        alias = self.root / "alias.whl"
        alias.symlink_to(self.wheel)
        # Wrong filename and alias both deny.
        with self.assertRaises(MODULE.Denied):
            MODULE.verify_pypi_uv_wheel(
                self.metadata, alias, self.binary, self.license)


if __name__ == "__main__":
    unittest.main()
