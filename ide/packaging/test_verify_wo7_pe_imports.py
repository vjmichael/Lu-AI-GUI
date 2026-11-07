"""WO7 genuine import directory parsing, including delay-load library names."""
import struct
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from verify_wo7_pe_imports import pe_imports, audit_staged_pe, Denied


def fake_pe(name=b"KERNEL32.dll", delay=None):
    b=bytearray(0x800)
    b[:2]=b"MZ"
    struct.pack_into("<I",b,0x3c,0x80)
    b[0x80:0x84]=b"PE\0\0"
    struct.pack_into("<H",b,0x84,0x8664)
    struct.pack_into("<H",b,0x86,1)
    struct.pack_into("<H",b,0x94,0xf0)
    opt=0x98
    struct.pack_into("<H",b,opt,0x20b)
    struct.pack_into("<Q",b,opt+24,0x140000000)
    struct.pack_into("<I",b,opt+108,16)
    struct.pack_into("<II",b,opt+120,0x1000,40)
    section=opt+0xf0
    b[section:section+6]=b".rdata"
    struct.pack_into("<IIII",b,section+8,0x400,0x1000,0x400,0x200)
    struct.pack_into("<I",b,0x20c,0x1080)
    b[0x280:0x280+len(name)+1]=name+b"\0"
    if delay:
        struct.pack_into("<II",b,opt+112+13*8,0x1100,64)
        struct.pack_into("<II",b,0x300,1,0x1190)
        b[0x390:0x390+len(delay)+1]=delay+b"\0"
    return b


class PeImportTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path=Path(self.tmp.name)/"test.exe"

    def test_real_format_normal_and_delay_import_names(self):
        self.path.write_bytes(fake_pe(b"KERNEL32.dll", b"PRIVATE.dll"))
        self.assertEqual(pe_imports(self.path), {"kernel32.dll","private.dll"})

    def test_non_pe_rejected(self):
        self.path.write_bytes(b"not an exe")
        with self.assertRaises(Denied):
            pe_imports(self.path)

    def test_path_like_import_rejected(self):
        self.path.write_bytes(fake_pe(b"..\\EVIL.dll"))
        with self.assertRaisesRegex(Denied,"unsafe_import_library_name"):
            pe_imports(self.path)

    def _tree(self, imported: bytes, include_python: bool):
        root = Path(self.tmp.name)
        binpath = root / "resources" / "debugger" / "bin"
        binpath.mkdir(parents=True)
        debugger = binpath / "lldb-dap.exe"
        debugger.write_bytes(fake_pe(imported))
        notice = root / "resources" / "third-party-notices" / "000-000.txt"
        notice.parent.mkdir(parents=True)
        notice.write_text("Test notice fixture only", encoding="utf-8")
        records = []
        for i in range(4):
            item = debugger if i == 0 else root / f"test-{i}.exe"
            if i != 0:
                item.write_bytes(fake_pe())
            records.append({
                "destination": item.relative_to(root).as_posix(),
                "sha256": hashlib.sha256(item.read_bytes()).hexdigest(),
                "license": "Apache-2.0",
                "notices": [{"path": notice.relative_to(root).as_posix(),
                              "sha256": hashlib.sha256(notice.read_bytes()).hexdigest()}],
            })
        if include_python:
            py = binpath / "python310.dll"
            py.write_bytes(fake_pe())
            records.append({
                "destination": py.relative_to(root).as_posix(),
                "sha256": hashlib.sha256(py.read_bytes()).hexdigest(),
                "license": "PSF-2.0",
                "notices": [records[0]["notices"][0]],
            })
        receipt = root / "resources" / "wo7-staged-tree.json"
        receipt.write_text(json.dumps({"schema": "lu.ide.wo7.staged-tree.v1", "files": records}), encoding="utf-8")
        return root

    def test_python310_present_but_unreviewed_is_denied(self):
        root = self._tree(b"python310.dll", include_python=False)
        py = root / "resources" / "debugger" / "bin" / "python310.dll"
        py.write_bytes(fake_pe())
        with self.assertRaisesRegex(Denied, "unreviewed_private_import"):
            audit_staged_pe(root)

    def test_python310_reviewed_and_pinned_private_import_closes(self):
        root = self._tree(b"python310.dll", include_python=True)
        observed = audit_staged_pe(root)
        self.assertTrue(observed["fullReviewedPrivateDllChain"])
        self.assertFalse(observed["releaseAccepted"])
        self.assertTrue(observed["dynamicImportAuditStillRequired"])

    def test_transitive_library_digest_drift_is_denied(self):
        root = self._tree(b"python310.dll", include_python=True)
        py = root / "resources" / "debugger" / "bin" / "python310.dll"
        py.write_bytes(fake_pe(b"USER32.dll"))
        with self.assertRaisesRegex(Denied, "staged_binary_digest_drift"):
            audit_staged_pe(root)

    def test_unreviewed_notice_changes_are_denied(self):
        root = self._tree(b"KERNEL32.dll", include_python=False)
        notice = root / "resources" / "third-party-notices" / "000-000.txt"
        notice.write_text("Modified notice", encoding="utf-8")
        with self.assertRaisesRegex(Denied, "license_notice_digest_drift"):
            audit_staged_pe(root)

    def test_truncated_directory_rejected(self):
        blob=fake_pe()
        struct.pack_into("<I",blob,0x20c,0x7fffffff)
        self.path.write_bytes(blob)
        with self.assertRaises(Denied):
            pe_imports(self.path)


if __name__ == "__main__":
    unittest.main()
