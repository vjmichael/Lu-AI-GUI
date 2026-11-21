import copy
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

from review_upstream_lsp_archives import (
    checked_manifest, extract_one, review_archives,
)


class UpstreamNativeLspCandidates(unittest.TestCase):
    def create_fixture(self, root: Path):
        manifest = json.loads(Path(__file__).with_name(
            "lsp_upstream_archive_candidates_20261108.json"
        ).read_text(encoding="utf-8"))
        bundle = root / "downloads"
        bundle.mkdir()
        for entry in manifest["releases"]:
            entry["version"] = "test-1"
            for osname, item in entry["targets"].items():
                item["url"] = (f"https://github.com/{entry['repo']}/releases/"
                               f"download/test-1/{item['name']}")
                dest = bundle / item["name"]
                payload = ("verified-fixture:" + entry["id"] + ":" + osname).encode()
                if item["kind"] == "gzip":
                    with gzip.open(dest,"wb") as stream:
                        stream.write(payload)
                elif item["kind"] == "zip":
                    with zipfile.ZipFile(dest,"w",zipfile.ZIP_DEFLATED) as stream:
                        stream.writestr("release/bin/"+item["binary"], payload)
                        stream.writestr("release/NOTICES/LICENSE", "candidate notice")
                elif item["kind"] == "tar.gz":
                    with tarfile.open(dest,"w:gz") as stream:
                        name="release/bin/"+item["binary"]
                        entry_info=tarfile.TarInfo(name=name)
                        entry_info.size=len(payload)
                        stream.addfile(entry_info,io.BytesIO(payload))
                item["sha256"]=hashlib.sha256(dest.read_bytes()).hexdigest()
        return manifest, bundle

    def test_six_pinned_archive_intake_measures_binary_and_never_approves(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            manifest,store=self.create_fixture(root)
            receipt=review_archives(manifest,store,root/"receipt.json")
            self.assertEqual(len(receipt["sources"]),6)
            self.assertFalse(receipt["releaseApproved"])
            self.assertFalse(receipt["redistributionApproved"])
            self.assertFalse(receipt["installedOrExecuted"])
            self.assertFalse(receipt["originalNoticesVerified"])
            self.assertFalse(receipt["nativeContainmentAccepted"])
            for row in receipt["sources"]:
                self.assertEqual(len(row["archiveSha256"]),64)
                self.assertEqual(len(row["executableSha256"]),64)
                self.assertGreater(row["executableBytes"],0)
            self.assertEqual(json.loads((root/"receipt.json").read_text()),receipt)
            with self.assertRaises(ValueError):
                review_archives(manifest,store,root/"receipt.json")

    def test_upstream_digest_tamper_fails_without_approving_or_writing_receipt(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            manifest,store=self.create_fixture(root)
            data=manifest["releases"][0]["targets"]["windows-x86_64"]
            with (store/data["name"]).open("ab") as stream:
                stream.write(b"corrupt")
            with self.assertRaisesRegex(ValueError,"digest mismatch"):
                review_archives(manifest,store,root/"receipt.json")
            self.assertFalse((root/"receipt.json").exists())

    def test_reject_latest_mutable_urls_foreign_repo_missing_platforms_and_approval(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            original,store=self.create_fixture(root)
            changed=copy.deepcopy(original)
            changed["releaseApproved"]=True
            with self.assertRaises(ValueError):
                checked_manifest(changed)
            changed=copy.deepcopy(original)
            changed["releases"][0]["targets"]["windows-x86_64"]["url"] += "?latest=true"
            with self.assertRaises(ValueError):
                checked_manifest(changed)
            changed=copy.deepcopy(original)
            changed["releases"][0]["targets"].pop("linux-x86_64")
            with self.assertRaises(ValueError):
                checked_manifest(changed)
            changed=copy.deepcopy(original)
            changed["releases"][0]["repo"]="someone-else/clangd"
            with self.assertRaises(ValueError):
                checked_manifest(changed)
            changed=copy.deepcopy(original)
            changed["releases"].pop()
            with self.assertRaises(ValueError):
                checked_manifest(changed)

    def test_zip_member_traversal_duplicate_binary_and_gzip_bomb_bounds(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            path=root/"bad.zip"
            with zipfile.ZipFile(path,"w") as writer:
                writer.writestr("../escape/clangd.exe",b"unsafe")
            with self.assertRaises(ValueError):
                extract_one(path,"zip","clangd.exe",root/"out1")
            with zipfile.ZipFile(path,"w") as writer:
                writer.writestr("a/clangd.exe",b"a")
                writer.writestr("b/clangd.exe",b"b")
            with self.assertRaisesRegex(ValueError,"exactly one"):
                extract_one(path,"zip","clangd.exe",root/"out2")
            # Ensure no source archive may smuggle symlinks as executables.
            with zipfile.ZipFile(path,"w") as writer:
                info=zipfile.ZipInfo("bin/clangd.exe")
                info.create_system=3
                info.external_attr=(0o120777 << 16)
                writer.writestr(info,"../../outside")
            with self.assertRaisesRegex(ValueError,"symlink"):
                extract_one(path,"zip","clangd.exe",root/"out3")


if __name__ == "__main__":
    unittest.main()
