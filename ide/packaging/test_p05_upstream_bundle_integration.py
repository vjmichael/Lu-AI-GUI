"""Full upstream notice receipt -> bundled installer resource audit, offline fixture."""
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

from collect_verification_licenses import bundle, UPSTREAM
from upstream_notice_receipt import verify_sources, SOURCES
from verify_p04_license_bundle import audit, Denied

HEAD = "a" * 40

class UpstreamBundlingTests(unittest.TestCase):
    def setUp(self):
        self.tmp = TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        app = self.root / "ide/app"
        (app / "node_modules/example").mkdir(parents=True)
        (app / "src-tauri").mkdir()
        (app / "package.json").write_text('{"name":"lu-ide-test"}', encoding="utf-8")
        (app / "node_modules/example/package.json").write_text(
            '{"name":"example","version":"1.0.0","license":"MIT"}', encoding="utf-8")
        (app / "node_modules/example/LICENSE").write_text("MIT License\n" + "A"*220, encoding="utf-8")
        (app / "src-tauri/Cargo.toml").write_text("[package]\nname='example'\n", encoding="utf-8")
        (app / "package-lock.json").write_text("npm lock source " + "n"*150, encoding="utf-8")
        (app / "src-tauri/Cargo.lock").write_text("Cargo.lock source " + "c"*150, encoding="utf-8")
        cargo = self.root / "rust-dep"
        cargo.mkdir()
        (cargo / "LICENSE").write_text("MIT License\n" + "C"*220, encoding="utf-8")
        self.cargo = cargo
        lic = self.root / "ide/packaging/upstream-licenses"
        lic.mkdir(parents=True)
        for name in UPSTREAM:
            (lic / name).write_text("Original " + name + "\n" + "U"*230, encoding="utf-8")
        self.proof = self.root / "upstream-source-receipt.json"
        def fetch(url):
            name = next(name for name, (_, pinned) in SOURCES.items() if pinned == url)
            return (lic / name).read_bytes()
        self.proof.write_text(json.dumps(verify_sources(self.root, HEAD, fetch)), encoding="utf-8")
        self.out = self.root / "notices"

    def collect(self, proof=True):
        with patch("collect_verification_licenses.cargo_packages",return_value=(
                [("cargo","rust-dep","1.0.0","MIT",self.cargo)], [])):
            bundle(self.root, self.out, HEAD, upstream_receipt=self.proof if proof else None)

    def test_actual_upstream_retrieval_and_every_notice_byte_bound(self):
        self.collect()
        report = audit(self.out, HEAD, require_upstream=True)
        self.assertTrue(report["upstreamRetrievalVerified"])
        self.assertTrue(report["actualBytesVerified"])
        self.assertFalse(report["signedOrLegallyApproved"])
        self.assertEqual(report["unresolved"], 0)
        self.assertTrue((self.out / "upstream-fetch.json").is_file())

    def test_missing_upstream_proof_fails_package_submission(self):
        self.collect(proof=False)
        with self.assertRaisesRegex(Denied, "upstream_license_sources_not_retrieved"):
            audit(self.out, HEAD, require_upstream=True)

    def test_modified_bundled_retrieval_receipt_is_denied(self):
        self.collect()
        target = self.out / "upstream-fetch.json"
        target.write_text(target.read_text(encoding="utf-8") + "\n", encoding="utf-8")
        with self.assertRaisesRegex(Denied, "upstream_fetch_receipt_digest_drift"):
            audit(self.out, HEAD, require_upstream=True)

    def test_untrusted_receipt_never_staged_into_license_package(self):
        proof = json.loads(self.proof.read_text(encoding="utf-8"))
        proof["sources"][0]["sourceRevision"] = "moving-main"
        self.proof.write_text(json.dumps(proof), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unverified_upstream_notice_source"):
            self.collect()
        self.assertFalse(self.out.exists())

if __name__ == "__main__":
    unittest.main()
