import hashlib
import json
import pathlib
import tempfile
import unittest

from stage_linux_language_server_release import stage_linux, bundle_digest

ORIGINAL = pathlib.Path(__file__).with_name("language-servers.manifest.json")


class LinuxProtectedLspStageTests(unittest.TestCase):
    def fixture(self, temp):
        root = pathlib.Path(temp)
        source = root / "supplied"
        for name in ["win", "linux", "notices"]:
            (source / name).mkdir(parents=True)
        (source / "win/clangd.exe").write_bytes(b"reviewed win clangd")
        (source / "linux/clangd").write_bytes(b"reviewed linux clangd")
        (source / "notices/LICENSE-LLVM").write_text("reviewed SPDX license")
        (source / "win/original-runtime.dll").write_bytes(b"reviewed DLL")
        (source / "linux/lib/clang/23/include").mkdir(parents=True)
        (source / "linux/lib/clang/23/include/stddef.h").write_bytes(b"reviewed original Clang header")
        data = json.loads(ORIGINAL.read_text(encoding="utf-8"))
        row = next(item for item in data["servers"] if item["id"] == "clangd")
        row.update({
            "mode": "bundled",
            "releaseVersion": "ci-fixture-not-a-release",
            "copyrightReviewed": True,
            "dependenciesAudited": True,
            "assetByPlatform": {
                "windows-x86_64": "win/clangd.exe",
                "linux-x86_64": "linux/clangd",
            },
            "sha256ByPlatform": {
                "windows-x86_64": hashlib.sha256(b"reviewed win clangd").hexdigest(),
                "linux-x86_64": hashlib.sha256(b"reviewed linux clangd").hexdigest(),
            },
            "licenseNoticeFiles": ["notices/LICENSE-LLVM"],
            "nativeResourceManifestReviewed": True,
            "nativeResourceFilesByPlatform": {
                "windows-x86_64": {
                    "bin/original-runtime.dll": {
                        "path":"win/original-runtime.dll",
                        "sha256":hashlib.sha256(b"reviewed DLL").hexdigest(),
                    },
                },
                "linux-x86_64": {
                    "lib/clang/23/include/stddef.h": {
                        "path":"linux/lib/clang/23/include/stddef.h",
                        "sha256":hashlib.sha256(b"reviewed original Clang header").hexdigest(),
                    },
                },
            },
        })
        data["releaseApproved"] = True
        manifest = root / "fixture-manifest.json"
        manifest.write_text(json.dumps(data), encoding="utf-8")
        return source, manifest

    def test_original_unreviewed_manifest_cannot_stage_released_bins(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "assets").mkdir()
            with self.assertRaises(ValueError):
                stage_linux(ORIGINAL, root / "assets", root / "installed")

    def test_linux_staging_preserves_exact_notice_hash_and_native_tree_digest(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            source, manifest = self.fixture(tmp)
            destination = root / "output"
            evidence = stage_linux(manifest, source, destination)
            self.assertEqual(evidence["schema"], "lu.ide.linux-lsp-staging.v1")
            self.assertEqual(evidence["platform"], "linux-x86_64")
            self.assertFalse(evidence["installedRootOwned"])
            self.assertFalse(evidence["nativeWorkerVerified"])
            pin = evidence["servers"]["clangd"]["bundleSha256"]
            self.assertEqual(pin, bundle_digest(destination / "language-servers/clangd"))
            self.assertEqual(len(pin), 64)
            self.assertEqual(evidence["servers"]["clangd"]["rustEnvPin"],
                             "LU_IDE_E2_CLANGD_BUNDLE_SHA256")
            self.assertEqual(
                (destination / "language-servers/clangd/bin/clangd").read_bytes(),
                b"reviewed linux clangd",
            )
            self.assertEqual(
                (destination / "language-servers/clangd/NOTICES/000-LICENSE-LLVM").read_text(),
                "reviewed SPDX license",
            )
            self.assertEqual(
                (destination / "language-servers/clangd/lib/clang/23/include/stddef.h").read_bytes(),
                b"reviewed original Clang header",
            )
            self.assertEqual(evidence["servers"]["clangd"]["originalNativeResourceFiles"],1)
            receipt = json.loads((destination / "linux-lsp-staging-evidence.json").read_text())
            self.assertEqual(receipt["servers"]["clangd"]["bundleSha256"], pin)
            with self.assertRaises(ValueError):
                stage_linux(manifest, source, destination)

    def test_binary_or_license_tamper_is_refused_without_partial_destination(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            source, manifest = self.fixture(tmp)
            (source / "linux/clangd").write_bytes(b"tampered")
            with self.assertRaises(ValueError):
                stage_linux(manifest, source, root / "destination1")
            self.assertFalse((root / "destination1").exists())
            (source / "linux/clangd").write_bytes(b"reviewed linux clangd")
            (source / "notices/LICENSE-LLVM").unlink()
            with self.assertRaises(ValueError):
                stage_linux(manifest, source, root / "destination2")
            self.assertFalse((root / "destination2").exists())

    def test_unreviewed_python_node_language_server_cannot_be_staged(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            source, manifest = self.fixture(tmp)
            data = json.loads(manifest.read_text(encoding="utf-8"))
            row = next(item for item in data["servers"] if item["id"] == "pyright")
            row.update({
                "mode": "bundled",
                "releaseVersion": "test-only",
                "copyrightReviewed": True,
                "dependenciesAudited": True,
                "assetByPlatform": {
                    "windows-x86_64": "win/clangd.exe",
                    "linux-x86_64": "linux/clangd",
                },
                "sha256ByPlatform": {
                    "windows-x86_64": hashlib.sha256(b"reviewed win clangd").hexdigest(),
                    "linux-x86_64": hashlib.sha256(b"reviewed linux clangd").hexdigest(),
                },
                "licenseNoticeFiles": ["notices/LICENSE-LLVM"],
            })
            manifest.write_text(json.dumps(data), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "unsupported"):
                stage_linux(manifest, source, root / "destination")
            self.assertFalse((root / "destination").exists())

    def test_symlinked_root_or_notice_does_not_escape_approved_asset_store(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            source, manifest = self.fixture(tmp)
            (source / "notices/LICENSE-LLVM").unlink()
            try:
                (source / "notices/LICENSE-LLVM").symlink_to(root / "target")
            except (NotImplementedError, OSError):
                return
            with self.assertRaises(ValueError):
                stage_linux(manifest, source, root / "destination")


if __name__ == "__main__":
    unittest.main()
