import hashlib
import json
import pathlib
import tempfile
import unittest

from stage_language_server_release import stage

MANIFEST = pathlib.Path(__file__).with_name("language-servers.manifest.json")


class E2BundleStageTests(unittest.TestCase):
    def setup_assets(self, root):
        upstream = root / "approved"
        (upstream / "w").mkdir(parents=True)
        (upstream / "l").mkdir(parents=True)
        (upstream / "notices").mkdir(parents=True)
        (upstream / "w/clangd.exe").write_bytes(b"reviewed Windows binary")
        (upstream / "l/clangd").write_bytes(b"reviewed Linux binary")
        (upstream / "notices/LICENSE-MIT").write_text("reviewed license notice")
        service = root / "signed-service.exe"
        service.write_bytes(b"simulated signed service fixture")
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        record = next(x for x in manifest["servers"] if x["id"] == "clangd")
        record.update({
            "mode": "bundled", "releaseVersion": "fixture-1",
            "copyrightReviewed": True, "dependenciesAudited": True,
            "assetByPlatform": {
                "windows-x86_64": "w/clangd.exe",
                "linux-x86_64": "l/clangd",
            },
            "sha256ByPlatform": {
                "windows-x86_64": hashlib.sha256(b"reviewed Windows binary").hexdigest(),
                "linux-x86_64": hashlib.sha256(b"reviewed Linux binary").hexdigest(),
            },
            "licenseNoticeFiles": ["notices/LICENSE-MIT"],
        })
        manifest["releaseApproved"] = True
        manifest_file = root / "manifest.json"
        manifest_file.write_text(json.dumps(manifest), encoding="utf-8")
        service_sha = hashlib.sha256(service.read_bytes()).hexdigest()
        return upstream, service, service_sha, manifest_file

    def test_license_notices_and_verified_binary_are_staged_under_tauri_resource_root(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            upstream, service, sha, manifest = self.setup_assets(root)
            dest = root / "app/src-tauri/resources"
            record = stage(manifest, upstream, service, sha, dest,
                           verify_service_signature=False)
            self.assertEqual(record["schema"], "lu.ide.e2.staged-release.v1")
            self.assertEqual((dest / "language-servers/clangd/bin/clangd.exe").read_bytes(),
                             b"reviewed Windows binary")
            self.assertTrue((dest / "language-servers/clangd/NOTICES/000-LICENSE-MIT").is_file())
            self.assertEqual((dest / "recovery/windows_recovery_service.exe").read_bytes(),
                             service.read_bytes())
            with self.assertRaises(ValueError):
                stage(manifest, upstream, service, sha, dest,
                      verify_service_signature=False)

    def test_unreviewed_or_modified_bundle_is_not_staged(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            upstream, service, sha, manifest = self.setup_assets(root)
            (upstream / "w/clangd.exe").write_bytes(b"changed by attacker")
            dest = root / "resources"
            with self.assertRaises(ValueError):
                stage(manifest, upstream, service, sha, dest,
                      verify_service_signature=False)
            self.assertFalse(dest.exists())
            (upstream / "w/clangd.exe").write_bytes(b"reviewed Windows binary")
            (upstream / "notices/LICENSE-MIT").unlink()
            with self.assertRaises(ValueError):
                stage(manifest, upstream, service, sha, dest,
                      verify_service_signature=False)
            self.assertFalse(dest.exists())

    def test_protected_service_requires_independently_pinned_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            upstream, service, sha, manifest = self.setup_assets(root)
            dest = root / "resources"
            with self.assertRaises(ValueError):
                stage(manifest, upstream, service, "0" * 64, dest,
                      verify_service_signature=False)
            self.assertFalse(dest.exists())
