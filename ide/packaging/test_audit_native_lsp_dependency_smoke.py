import copy
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from audit_native_lsp_dependency_smoke import (
    SCHEMA, checked_relative, inspect, pe_imports, stage_archive,
    static_pe_report, version_probe,
)


def synthetic_pe_with_import(library: bytes = b"KERNEL32.dll") -> bytes:
    raw = bytearray(0x600)
    raw[:2] = b"MZ"
    struct.pack_into("<I", raw, 0x3c, 0x80)
    raw[0x80:0x84] = b"PE\x00\x00"
    # COFF: one section, optional header size 0xe0
    struct.pack_into("<H", raw, 0x86, 1)
    struct.pack_into("<H", raw, 0x94, 0xe0)
    optional = 0x98
    struct.pack_into("<H", raw, optional, 0x10b)
    # IMAGE_DIRECTORY_ENTRY_IMPORT is index 1 of the optional data dir.
    struct.pack_into("<II", raw, optional+96+8, 0x1000, 40)
    section = optional+0xe0
    struct.pack_into("<I", raw, section+8, 0x300)
    struct.pack_into("<I", raw, section+12, 0x1000)
    struct.pack_into("<I", raw, section+16, 0x300)
    struct.pack_into("<I", raw, section+20, 0x200)
    struct.pack_into("<I", raw, 0x200+12, 0x1040)
    raw[0x240:0x240+len(library)+1] = library+b"\x00"
    return bytes(raw)


class Dev026NativeClosureSourceTests(unittest.TestCase):
    def fixture(self, root: Path, platform: str):
        original = json.loads(Path(__file__).with_name(
            "lsp_upstream_archive_candidates_20261108.json"
        ).read_text(encoding="utf-8"))
        store = root/"archives"
        store.mkdir()
        for provider in original["releases"]:
            provider["version"]="test-1"
            # The production pin checker validates BOTH OS URLs, even in a
            # single-native-host fixture; update every target release URL.
            for target in provider["targets"].values():
                target["url"]=("https://github.com/"+provider["repo"]+
                               "/releases/download/test-1/"+target["name"])
            row=provider["targets"][platform]
            archive=store/row["name"]
            payload=(b"\x7fELF"+provider["id"].encode() if platform.startswith("linux")
                     else synthetic_pe_with_import())
            if row["kind"]=="zip":
                with zipfile.ZipFile(archive,"w") as z:
                    z.writestr("source/bin/"+row["binary"], payload)
                    if provider["id"]=="clangd":
                        z.writestr("source/bin/original-companion.dll", synthetic_pe_with_import())
                        z.writestr("source/lib/clang/23/include/stddef.h", b"resource")
            elif row["kind"]=="tar.gz":
                with tarfile.open(archive,"w:gz") as z:
                    info=tarfile.TarInfo("source/bin/"+row["binary"])
                    info.size=len(payload)
                    z.addfile(info,io.BytesIO(payload))
            elif row["kind"]=="gzip":
                with gzip.open(archive,"wb") as out:
                    out.write(payload)
            row["sha256"]=hashlib.sha256(archive.read_bytes()).hexdigest()
        return original,store

    def test_parse_actual_pe_header_import_and_fail_closed_on_truncation(self):
        self.assertEqual(pe_imports(synthetic_pe_with_import()), ["kernel32.dll"])
        self.assertEqual(pe_imports(synthetic_pe_with_import(b"custom.dll")),
                         ["custom.dll"])
        for malformed in (b"", b"MZ", synthetic_pe_with_import()[:140]):
            with self.assertRaises(ValueError):
                pe_imports(malformed)
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/"main.exe"
            path.write_bytes(synthetic_pe_with_import())
            report=static_pe_report(path,Path(tmp))
            self.assertEqual(report["osCandidateUnverified"],["kernel32.dll"])
            self.assertEqual(report["externalUnresolved"],[])
            path.write_bytes(synthetic_pe_with_import(b"vcruntime140.dll"))
            report=static_pe_report(path,Path(tmp))
            self.assertEqual(report["externalUnresolved"], ["vcruntime140.dll"])
            self.assertNotIn("vcruntime140.dll", report["osCandidateUnverified"])

    def test_reject_archive_traversal_duplicate_and_symlink(self):
        for path in ("../escape/clangd.exe", "/absolute/clangd.exe",
                     "a\\b/clangd.exe", "C:/evil/clangd.exe"):
            with self.assertRaises(ValueError):
                checked_relative(path)
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            archive=root/"malicious.zip"
            with zipfile.ZipFile(archive,"w") as z:
                z.writestr("../escape/clangd.exe",b"bad")
            with self.assertRaises(ValueError):
                stage_archive(archive,"zip","clangd.exe",root/"installed1")
            with zipfile.ZipFile(archive,"w") as z:
                z.writestr("a/clangd.exe",b"a")
                z.writestr("a/clangd.exe",b"b")
            with self.assertRaises(ValueError):
                stage_archive(archive,"zip","clangd.exe",root/"installed2")
            with zipfile.ZipFile(archive,"w") as z:
                info=zipfile.ZipInfo("main/clangd.exe")
                info.create_system=3
                info.external_attr=(0o120777<<16)
                z.writestr(info,b"../../outside")
            with self.assertRaises(ValueError):
                stage_archive(archive,"zip","clangd.exe",root/"installed3")

    def test_preserve_original_sibling_library_layout_and_unbundled_resources(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            manifest,archives=self.fixture(root,"windows-x86_64")
            row=manifest["releases"][0]["targets"]["windows-x86_64"]
            staged=stage_archive(archives/row["name"],row["kind"],
                                 row["binary"],root/"stage")
            self.assertTrue(staged["main"].exists())
            self.assertTrue((root/"stage/source/bin/original-companion.dll").exists())
            self.assertFalse((root/"stage/source/lib/clang/23/include/stddef.h").exists())
            self.assertEqual(staged["resourceEntriesNotBundled"],1)
            self.assertEqual(len(staged["files"]),2)
            with self.assertRaises(ValueError):
                stage_archive(archives/row["name"],row["kind"],
                              row["binary"],root/"stage")

    def test_independent_three_original_archives_and_fixed_version_receipt(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            manifest,archives=self.fixture(root,"linux-x86_64")
            with patch("audit_native_lsp_dependency_smoke.static_elf_report",
                       return_value={"neededNames":["libc.so.6"]}), patch(
                       "audit_native_lsp_dependency_smoke.version_probe",
                       return_value={"versionProbePassed":True, "reaped":True,
                                     "exitedZero":True}):
                result=inspect(manifest,archives,root/"stage","linux-x86_64")
            self.assertTrue(result["originalPinnedAssetsVerified"])
            self.assertTrue(result["allVersionProbesPassed"])
            self.assertEqual(len(result["providers"]),3)
            self.assertEqual(result["schema"],SCHEMA)
            for key in ("releaseApproved","redistributionApproved",
                        "protectedLuWorkerTested","genuineInstalledNsisOrDeb",
                        "fullDependencyClosureApproved","fullLLVMResourcesReviewed"):
                self.assertFalse(result[key])
            self.assertEqual(
                json.loads((root/"stage/candidate-dependency-smoke.json").read_text()),
                result
            )
            with self.assertRaises(ValueError):
                inspect(manifest,archives,root/"stage","linux-x86_64")

    def test_do_not_convert_archive_tamper_or_missing_executable_to_success(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            manifest,archives=self.fixture(root,"linux-x86_64")
            row=manifest["releases"][0]["targets"]["linux-x86_64"]
            with (archives/row["name"]).open("ab") as out:
                out.write(b"tamper")
            with patch("audit_native_lsp_dependency_smoke.static_elf_report",
                       return_value={"neededNames":[]}), patch(
                       "audit_native_lsp_dependency_smoke.version_probe",
                       return_value={"versionProbePassed":True}):
                result=inspect(manifest,archives,root/"stage","linux-x86_64")
            self.assertFalse(result["originalPinnedAssetsVerified"])
            self.assertFalse(result["allVersionProbesPassed"])
            clangd=next(p for p in result["providers"] if p["provider"]=="clangd")
            self.assertEqual(clangd["failureClass"],"ValueError")
            self.assertFalse(clangd["versionProbePassed"])


if __name__ == "__main__":
    unittest.main()
