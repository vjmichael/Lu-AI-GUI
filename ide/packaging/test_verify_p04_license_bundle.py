"""P04 complete source notice inventory rehash and exact native binary binding."""
import json
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from collect_verification_licenses import bundle, UPSTREAM
from upstream_notice_receipt import SOURCES, verify_sources
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
    def test_forged_remote_source_sha_denied_at_collect_and_bundle_audit(self):
        # A Windows CRLF checkout must not claim to be the byte-exact remote
        # original simply because the semantically normalized text matches.
        by_url = {
            url: (self.root/"ide/packaging/upstream-licenses"/name).read_bytes()
            for name, (_, url) in SOURCES.items()
        }
        record = verify_sources(self.root, HEAD, lambda url: by_url[url])
        receipt = self.root/"verified-upstream.json"
        receipt.write_text(json.dumps(record),encoding="utf-8")
        target = self.root/"strict-notices"
        with patch("collect_verification_licenses.cargo_packages",return_value=([],[])):
            bundle(self.root,target,HEAD,upstream_receipt=receipt)
        self.assertTrue(audit(target,HEAD,require_upstream=True)["actualBytesVerified"])
        name=next(iter(SOURCES))
        bad = json.loads(receipt.read_text(encoding="utf-8"))
        first=next(row for row in bad["sources"] if row["name"]==name)
        first["sourceSha256"] = "b"*64
        receipt.write_text(json.dumps(bad),encoding="utf-8")
        with patch("collect_verification_licenses.cargo_packages",return_value=([],[])):
            with self.assertRaisesRegex(ValueError,"unverified_upstream_notice_source"):
                bundle(self.root,self.root/"strict-forged",HEAD,upstream_receipt=receipt)
        embedded=target/"upstream-fetch.json"
        embedded.write_text(json.dumps(bad),encoding="utf-8")
        inventory_path=target/"inventory.json"
        inventory=json.loads(inventory_path.read_text(encoding="utf-8"))
        inventory["upstreamRetrieval"]["sha256"] = hashlib.sha256(embedded.read_bytes()).hexdigest()
        inventory_path.write_text(json.dumps(inventory),encoding="utf-8")
        with self.assertRaisesRegex(Denied,"upstream_fetch_notice_mismatch"):
            audit(target,HEAD,require_upstream=True)

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
