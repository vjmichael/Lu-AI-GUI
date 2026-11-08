import copy
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile
from unittest.mock import patch

from collect_original_lsp_notices import git_blob_sha1
from stage_native_lsp_disposable_test_install import (
    SCHEMA, allowed_temp_destination, install_verified, make_package,
)


class DisposableNativeLspPayloadTests(unittest.TestCase):
    def fixture(self, root: Path, platform: str):
        pack = Path(__file__).with_name(
            "lsp_upstream_archive_candidates_20261108.json")
        manifest = json.loads(pack.read_text(encoding="utf-8"))
        original = json.loads(Path(__file__).with_name(
            "lsp_original_upstream_notices_20261108.json"
        ).read_text(encoding="utf-8"))
        source = root / "upstream"
        source.mkdir()
        for entry in manifest["releases"]:
            entry["version"] = "test-1"
            for target, info in entry["targets"].items():
                info["url"] = ("https://github.com/" + entry["repo"] +
                    "/releases/download/test-1/" + info["name"])
                if target != platform:
                    continue
                archive = source / info["name"]
                payload = (entry["id"] + "-" + target).encode("ascii")
                if info["kind"] == "gzip":
                    with gzip.open(archive, "wb") as writer:
                        writer.write(payload)
                elif info["kind"] == "zip":
                    with zipfile.ZipFile(archive, "w") as writer:
                        writer.writestr("release/bin/" + info["binary"], payload)
                else:
                    with tarfile.open(archive, "w:gz") as writer:
                        member = tarfile.TarInfo("release/bin/" + info["binary"])
                        member.size = len(payload)
                        writer.addfile(member, io.BytesIO(payload))
                info["sha256"] = hashlib.sha256(archive.read_bytes()).hexdigest()
        notice_root = root / "notices-original"
        notice_root.mkdir()
        for row in original["notices"]:
            payload = (row["provider"] + ":" + row["path"]).encode("utf-8")
            row["gitBlobSha1"] = git_blob_sha1(payload)
            target = notice_root / row["destination"]
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(payload)
        return manifest, original, source, notice_root

    def test_real_archive_unpack_zip_package_and_disposable_install_both_os(self):
        for platform in ("windows-x86_64", "linux-x86_64"):
            with self.subTest(platform=platform), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                manifest, notices, archives, original = self.fixture(root, platform)
                result = make_package(
                    manifest, notices, archives, original, root / "test-output",
                    platform
                )
                self.assertTrue(result["hashReadbackPassed"])
                self.assertEqual(result["payloadFilesVerified"], 7)
                self.assertEqual(result["archiveDigestsVerified"], 3)
                self.assertEqual(result["originalNoticesVerified"], 4)
                for key in ("releaseApproved", "dependencyClosureVerified",
                            "nativeWorkerContainmentAccepted", "executedInsideLu",
                            "genuineNsisOrDebInstalled"):
                    self.assertFalse(result[key])
                out = root / "test-output"
                self.assertEqual(
                    json.loads((out/"verification-receipt.json").read_text()),
                    result
                )
                with zipfile.ZipFile(out/"native-lsp-disposable-test-payload.zip") as z:
                    seal = json.loads(z.read("bundle-manifest.json"))
                    self.assertEqual(seal["schema"], SCHEMA)
                    self.assertFalse(seal["releaseApproved"])
                    self.assertEqual(len(seal["files"]), 7)
                with self.assertRaises(ValueError):
                    make_package(manifest, notices, archives, original,
                                 out, platform)

    def test_reject_wrong_upstream_archive_and_forged_notice(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest, notices, archives, original = self.fixture(root, "linux-x86_64")
            item = manifest["releases"][0]["targets"]["linux-x86_64"]
            with (archives / item["name"]).open("ab") as writer:
                writer.write(b"archive-tamper")
            with self.assertRaisesRegex(ValueError, "SHA256 mismatch"):
                make_package(manifest, notices, archives, original,
                             root/"test-output", "linux-x86_64")
            self.assertFalse((root/"test-output").exists())
            # Original upstream notice Git blob SHA must reject altered content.
            manifest, notices, archives, original = self.fixture(
                root / "second", "linux-x86_64"
            ) if False else (manifest, notices, archives, original)
            notice = original / notices["notices"][0]["destination"]
            notice.write_bytes(b"forged")
            with self.assertRaisesRegex(ValueError, "Git blob byte identity"):
                make_package(manifest, notices, archives, original,
                             root/"test-output", "linux-x86_64")

    def test_reject_unexpected_zip_members_and_corrupt_install(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            manifest, notices, archives, original=self.fixture(root, "linux-x86_64")
            make_package(manifest, notices, archives, original,
                         root/"bundle", "linux-x86_64")
            zip_path=root/"bundle/native-lsp-disposable-test-payload.zip"
            with zipfile.ZipFile(zip_path) as z:
                seal=json.loads(z.read("bundle-manifest.json"))
            with zipfile.ZipFile(zip_path, "a") as z:
                z.writestr("../../escaped", b"bad")
            with self.assertRaisesRegex(ValueError, "unexpected"):
                install_verified(zip_path, root/"bad-install", seal)
            with self.assertRaises(ValueError):
                install_verified(zip_path, root/"bundle/installed", seal)
            changed=copy.deepcopy(seal)
            changed["files"][next(iter(changed["files"]))]["sha256"]="0"*64
            with self.assertRaises(ValueError):
                install_verified(zip_path,root/"bad-install-two",changed)

    def test_disposable_destination_never_leaves_ci_and_no_symlinks(self):
        with tempfile.TemporaryDirectory() as tmp:
            runner=Path(tmp)/"runner"
            runner.mkdir()
            with patch.dict(os.environ, {"GITHUB_ACTIONS":"true",
                                         "RUNNER_TEMP":str(runner)}):
                allowed_temp_destination(runner/"new")
                with self.assertRaisesRegex(ValueError,"outside"):
                    allowed_temp_destination(Path(tmp)/"elsewhere")
                (runner/"already").mkdir()
                with self.assertRaisesRegex(ValueError,"absent"):
                    allowed_temp_destination(runner/"already")
                (runner/"linked").symlink_to(runner/"already",target_is_directory=True)
                with self.assertRaises(ValueError):
                    allowed_temp_destination(runner/"linked/new")
            with patch.dict(os.environ, {"GITHUB_ACTIONS":"false",
                                         "RUNNER_TEMP":str(runner)}):
                with self.assertRaises(ValueError):
                    allowed_temp_destination(runner/"new")


if __name__ == "__main__":
    unittest.main()
