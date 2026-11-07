"""Canonical upstream source revisions must agree across text, receipt and bundle."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from collect_verification_licenses import UPSTREAM,bundle
from upstream_notice_receipt import SOURCES,verify_sources
from verify_p04_license_bundle import audit,Denied
SHA="a"*40
class SourceRevisionTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)
        app=self.root/"ide/app"
        (app/"src-tauri").mkdir(parents=True)
        (app/"package.json").write_text('{"name":"fixture"}')
        (app/"src-tauri/Cargo.toml").write_text('[package]\nname="fixture"\n')
        (app/"package-lock.json").write_text("x"*150)
        (app/"src-tauri/Cargo.lock").write_text("c"*150)
        folder=self.root/"ide/packaging/upstream-licenses"
        folder.mkdir(parents=True)
        for name in UPSTREAM:
            (folder/name).write_text(name+"\n"+"X"*220)
        data=verify_sources(self.root,SHA,lambda url:(folder/next(name for name,(_,src) in SOURCES.items() if src==url)).read_bytes())
        self.receipt=self.root/"reviewed-receipt.json"
        self.receipt.write_text(json.dumps(data))
        self.out=self.root/"notices"
        with patch("collect_verification_licenses.cargo_packages",return_value=([],[])):
            bundle(self.root,self.out,SHA,upstream_receipt=self.receipt)
    def test_exact_upstream_revisions_trusted(self):
        self.assertTrue(audit(self.out,SHA,require_upstream=True)["actualBytesVerified"])
        self.assertEqual(UPSTREAM,{name:rev for name,(rev,url) in SOURCES.items()})
    def test_altered_inventory_revision_denied(self):
        inventory=self.out/"inventory.json"
        data=json.loads(inventory.read_text())
        data["upstreamLicenses"][0]["sourceRevision"]="moving-main"
        inventory.write_text(json.dumps(data))
        with self.assertRaisesRegex(Denied,"upstream_fetch_notice_mismatch"):
            audit(self.out,SHA,require_upstream=True)
    def test_altered_fetch_source_url_denied(self):
        receipt=self.out/"upstream-fetch.json"
        data=json.loads(receipt.read_text())
        data["sources"][0]["sourceUrl"]="https://example.test/malicious"
        receipt.write_text(json.dumps(data))
        with self.assertRaisesRegex(Denied,"upstream_fetch_receipt_digest_drift"):
            audit(self.out,SHA,require_upstream=True)
if __name__=="__main__":unittest.main()
