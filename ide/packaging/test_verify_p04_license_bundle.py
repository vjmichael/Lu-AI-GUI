"""P04 complete source notice inventory rehash and exact native binary binding."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from collect_verification_licenses import bundle, UPSTREAM
from verify_p04_license_bundle import audit, Denied

HEAD="a"*40

class VerifyPackageNoticesTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)
        node=self.root/"ide/app/node_modules/react"
        node.mkdir(parents=True)
        (self.root/"ide/app/package.json").write_text('{"name":"test"}',encoding="utf-8")
        (node/"package.json").write_text('{"name":"react","version":"19.0","license":"MIT"}',encoding="utf-8")
        (node/"LICENSE").write_text("MIT license\n"+"x"*300,encoding="utf-8")
        (self.root/"ide/app/src-tauri").mkdir()
        (self.root/"ide/app/src-tauri/Cargo.toml").write_text("[package]\nname='test'\n",encoding="utf-8")
        lic=self.root/"ide/packaging/upstream-licenses"
        lic.mkdir(parents=True)
        for name in UPSTREAM:
            (lic/name).write_text("License text\n"+"x"*300,encoding="utf-8")
        (self.root/"ide/app/package-lock.json").write_text('{"lockfileVersion":3}'+" "*120,encoding="utf-8")
        (self.root/"ide/app/src-tauri/Cargo.lock").write_text("# auto-generated Cargo.lock\n"+"a"*120,encoding="utf-8")
        self.out=self.root/"notices"
        with patch("collect_verification_licenses.cargo_packages",return_value=([],[])):
            bundle(self.root,self.out,HEAD)
    def test_real_files_match_manifest_and_never_grant_release(self):
        report=audit(self.out,HEAD)
        self.assertTrue(report["actualBytesVerified"])
        self.assertFalse(report["signedOrLegallyApproved"])
        self.assertEqual(report["unresolved"],0)
    def test_modification_of_license_caught(self):
        target=next((self.out/"resolved/npm").glob("*.txt"))
        target.write_text("tampered",encoding="utf-8")
        with self.assertRaisesRegex(Denied,"license_file_digest_drift"):
            audit(self.out,HEAD)
    def test_unlisted_file_denied(self):
        (self.out/"rogue.dll").write_bytes(b"unlisted")
        with self.assertRaisesRegex(Denied,"unlisted_or_missing_legal_bytes"):
            audit(self.out,HEAD)
    def test_wrong_exact_head_denied(self):
        with self.assertRaisesRegex(Denied,"untrusted_notice_inventory"):
            audit(self.out,"b"*40)
    def test_release_flag_tampering_denied(self):
        path=self.out/"inventory.json"
        data=json.loads(path.read_text(encoding="utf-8"))
        data["releaseApproved"]=True
        path.write_text(json.dumps(data),encoding="utf-8")
        with self.assertRaisesRegex(Denied,"untrusted_notice_inventory"):
            audit(self.out,HEAD)
if __name__=="__main__":
    unittest.main()
