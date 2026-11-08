import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from stage_test_only_lsp_resources import stage,SCHEMA,PLATFORMS


class TechnicalInstalledLspStaging(unittest.TestCase):
    def fixture(self,root:Path):
        manifest=json.loads(Path(__file__).with_name(
            "lsp_upstream_archive_candidates_20261108.json"
        ).read_text(encoding="utf-8"))
        notices=Path(__file__).with_name(
            "lsp_original_upstream_notices_20261108.json"
        )
        archive_cache=root/"archives"
        archive_cache.mkdir()
        for provider in manifest["releases"]:
            provider["version"]="ci-fixture"
            for osname,item in provider["targets"].items():
                item["url"]=f"https://github.com/{provider['repo']}/releases/download/ci-fixture/{item['name']}"
                binary=("fixture native executable "+provider["id"]+" "+osname).encode()
                target=archive_cache/item["name"]
                if item["kind"]=="gzip":
                    with gzip.open(target,"wb") as writer:
                        writer.write(binary)
                elif item["kind"]=="zip":
                    with zipfile.ZipFile(target,"w",zipfile.ZIP_DEFLATED) as writer:
                        writer.writestr("fixed/bin/"+item["binary"],binary)
                elif item["kind"]=="tar.gz":
                    with tarfile.open(target,"w:gz") as writer:
                        entry=tarfile.TarInfo("bin/"+item["binary"])
                        entry.size=len(binary)
                        writer.addfile(entry,io.BytesIO(binary))
                item["sha256"]=hashlib.sha256(target.read_bytes()).hexdigest()
        release=root/"manifest.json"
        release.write_text(json.dumps(manifest),encoding="utf-8")
        return release,notices,archive_cache

    def fake_originals(self,notice_manifest,output,**kwargs):
        for item in notice_manifest["notices"]:
            dest=output/item["destination"]
            dest.parent.mkdir(parents=True,exist_ok=True)
            dest.write_text("fixture original bytes "+item["provider"],encoding="utf-8")
        return {"originalBytesVerified":True,"redistributionApproved":False}

    def test_each_native_os_resource_tree_is_explicitly_unapproved_and_hash_bounded(self):
        for platform in sorted(PLATFORMS):
            with self.subTest(platform=platform),tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp)
                release,notices,cache=self.fixture(root)
                with patch.dict(os.environ,{"GITHUB_ACTIONS":"true","RUNNER_TEMP":tmp}), \
                    patch("stage_test_only_lsp_resources.collect",self.fake_originals):
                    dest=root/"test-only-output"
                    receipt=stage(release,notices,cache,dest,platform,"a"*40)
                self.assertEqual(receipt["schema"],SCHEMA)
                self.assertFalse(receipt["releaseApproved"])
                self.assertFalse(receipt["redistributionReviewed"])
                self.assertFalse(receipt["signedServiceInstalled"])
                self.assertFalse(receipt["installedProtectedSessionAccepted"])
                self.assertFalse(receipt["runLanguageServers"])
                self.assertEqual(set(receipt["resources"]),{"rust-analyzer","clangd","ruff"})
                self.assertTrue((dest/"DEVELOPMENT-ONLY-NOT-FOR-DISTRIBUTION.txt").is_file())
                for provider,value in receipt["resources"].items():
                    self.assertEqual(len(value["bundleSha256"]),64)
                    self.assertEqual(len(value["binarySha256"]),64)
                    self.assertTrue(value["compileTimePin"].startswith("LU_IDE_E2_"))
                    self.assertGreater(value["licenseFiles"],0)
                with self.assertRaises(ValueError):
                    with patch.dict(os.environ,{"GITHUB_ACTIONS":"true","RUNNER_TEMP":tmp}), \
                        patch("stage_test_only_lsp_resources.collect",self.fake_originals):
                        stage(release,notices,cache,dest,platform,"a"*40)

    def test_modified_upstream_archive_does_not_create_installable_resources(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            release,notices,cache=self.fixture(root)
            data=json.loads(release.read_text())
            item=data["releases"][0]["targets"]["linux-x86_64"]
            with (cache/item["name"]).open("ab") as binary:
                binary.write(b"tampered")
            with patch.dict(os.environ,{"GITHUB_ACTIONS":"true","RUNNER_TEMP":tmp}), \
                patch("stage_test_only_lsp_resources.collect",self.fake_originals):
                with self.assertRaisesRegex(ValueError,"archive digest mismatch"):
                    stage(release,notices,cache,root/"out","linux-x86_64","a"*40)
            self.assertFalse((root/"out").exists())

    def test_no_release_approval_and_no_ambient_path_authority(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            release,notices,cache=self.fixture(root)
            data=json.loads(release.read_text())
            data["releaseApproved"]=True
            release.write_text(json.dumps(data))
            with patch.dict(os.environ,{"GITHUB_ACTIONS":"true","RUNNER_TEMP":tmp}), \
                patch("stage_test_only_lsp_resources.collect",self.fake_originals):
                with self.assertRaises(ValueError):
                    stage(release,notices,cache,root/"out","linux-x86_64","a"*40)
            self.assertFalse((root/"out").exists())
            with self.assertRaises(ValueError):
                stage(release,notices,cache,root/"out","invalid-os","a"*40)


if __name__=="__main__":
    unittest.main()
