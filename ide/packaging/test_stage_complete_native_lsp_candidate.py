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

from collect_original_lsp_notices import git_blob_sha1
from stage_complete_native_lsp_candidate import (
    SCHEMA, candidate, normalize_original_tree, safe_parts, stage_provider,
)


class OriginalFullNativeBundleTests(unittest.TestCase):
    def fixture(self, root:Path, platform:str):
        archives=root/"original-archives"
        archives.mkdir()
        notice_root=root/"upstream-notices"
        notice_root.mkdir()
        manifest=json.loads(Path(__file__).with_name(
            "lsp_upstream_archive_candidates_20261108.json").read_text())
        originals=json.loads(Path(__file__).with_name(
            "lsp_original_upstream_notices_20261108.json").read_text())
        for row in originals["notices"]:
            raw=("original-notice-"+row["provider"]+"-"+row["path"]).encode()
            row["gitBlobSha1"]=git_blob_sha1(raw)
            notice=notice_root/row["destination"]
            notice.parent.mkdir(parents=True,exist_ok=True)
            notice.write_bytes(raw)
        for record in manifest["releases"]:
            record["version"]="test-1"
            for target,item in record["targets"].items():
                item["url"]=("https://github.com/"+record["repo"]+
                             "/releases/download/test-1/"+item["name"])
                if target != platform:
                    continue
                file=archives/item["name"]
                payload=("original "+record["id"]).encode()
                if item["kind"]=="gzip":
                    with gzip.open(file,"wb") as output:
                        output.write(payload)
                elif item["kind"]=="zip":
                    with zipfile.ZipFile(file,"w") as output:
                        prefix="clangd-release/" if record["id"]=="clangd" else "upstream/"
                        binary_path=(prefix+"bin/"+item["binary"] if record["id"]=="clangd"
                                    else prefix+item["binary"])
                        output.writestr(binary_path,payload)
                        if record["id"]=="clangd":
                            output.writestr(prefix+"bin/original-runtime.dll",b"original-dll")
                            output.writestr(prefix+"lib/clang/23/include/stddef.h",b"header-original")
                        else:
                            output.writestr(prefix+"README.md",b"original readme")
                else:
                    with tarfile.open(file,"w:gz") as output:
                        raw_name="upstream/"+item["binary"]
                        mem=tarfile.TarInfo(raw_name)
                        mem.size=len(payload)
                        output.addfile(mem,io.BytesIO(payload))
                item["sha256"]=hashlib.sha256(file.read_bytes()).hexdigest()
        return manifest,originals,archives,notice_root

    def test_complete_original_release_resource_tree_on_each_platform(self):
        for platform in ("windows-x86_64","linux-x86_64"):
            with self.subTest(platform=platform),tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp)
                manifest,notices,archives,original=self.fixture(root,platform)
                result=candidate(manifest,notices,archives,original,root/"stage",platform)
                self.assertEqual(result["schema"],SCHEMA)
                self.assertEqual(set(result["providers"]),{"clangd","rust-analyzer","ruff"})
                for key in ("releaseApproved","redistributionApproved",
                            "legalReviewComplete","protectedLuProtocolAccepted",
                            "realInstalledNsisDebAccepted","dynamicDependencyClosureApproved"):
                    self.assertFalse(result[key])
                self.assertTrue(result["allArchiveFileBytesRetained"])
                for name,row in result["providers"].items():
                    self.assertEqual(row["stagedOriginalEntries"],row["originalArchiveEntries"])
                    self.assertTrue((root/"stage"/name/
                        ("bin/"+("clangd.exe" if platform.startswith("windows") else "clangd")
                         if name=="clangd" else "bin/"+(
                          name+".exe" if platform.startswith("windows") else name))).exists())
                    for rel,record in row["files"].items():
                        raw=(root/"stage"/name).joinpath(*rel.split("/")).read_bytes()
                        self.assertEqual(record["sha256"],hashlib.sha256(raw).hexdigest())
                    self.assertGreaterEqual(len([v for v in row["files"] if v.startswith("NOTICES/")]),1)
                if platform.startswith("windows"):
                    self.assertTrue((root/"stage/clangd/bin/original-runtime.dll").exists())
                    self.assertTrue((root/"stage/clangd/lib/clang/23/include/stddef.h").exists())
                    self.assertEqual(result["providers"]["clangd"]["resourceFileCount"],1)
                self.assertEqual(json.loads((root/"stage/candidate-full-resource-receipt.json").read_text()), result)
                with self.assertRaises(ValueError):
                    candidate(manifest,notices,archives,original,root/"stage",platform)

    def test_no_path_alias_traversal_or_cross_root_resource_acceptance(self):
        for raw in ("../evil","/abs","x\\y","C:/windows","a//b","a/./b",
                    "a/../b","a/file name",""):
            with self.subTest(name=raw):
                with self.assertRaises(ValueError):
                    safe_parts(raw)
        with self.assertRaises(ValueError):
            normalize_original_tree(["release/bin/clangd.exe",
                                     "foreign/lib/clang/23/include/x.h"],"clangd.exe")
        with self.assertRaises(ValueError):
            normalize_original_tree(["release/bin/clangd.exe",
                                     "release/bin/CLANGD.exe"],"clangd.exe")
        with self.assertRaises(ValueError):
            normalize_original_tree(["release/bin/clangd.exe",
                                     "release/bin/x.dll",
                                     "release/bin/X.dll"],"clangd.exe")

    def test_original_archive_and_notices_are_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            m,n,archives,notices=self.fixture(root,"windows-x86_64")
            file=archives/m["releases"][0]["targets"]["windows-x86_64"]["name"]
            with file.open("ab") as out:
                out.write(b"tamper")
            with self.assertRaisesRegex(ValueError,"digest mismatch"):
                candidate(m,n,archives,notices,root/"not-created","windows-x86_64")
            self.assertFalse((root/"not-created").exists())
            original=notices/n["notices"][0]["destination"]
            original.write_bytes(b"forged-notice")
            with self.assertRaises(ValueError):
                candidate(m,n,archives,notices,root/"not-created","windows-x86_64")

    def test_archive_links_unknown_files_and_duplicate_native_binary_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            zip_path=root/"original.zip"
            with zipfile.ZipFile(zip_path,"w") as z:
                z.writestr("build/bin/clangd.exe",b"real")
                z.writestr("build/lib/clang/23/include/x.h",b"resource")
            report=stage_provider(zip_path,"zip","clangd.exe",root/"safe","windows-x86_64")
            self.assertEqual(report["stagedOriginalEntries"],2)
            with zipfile.ZipFile(zip_path,"w") as z:
                z.writestr("build/bin/clangd.exe",b"real")
                z.writestr("../../outside",b"evil")
            with self.assertRaises(ValueError):
                stage_provider(zip_path,"zip","clangd.exe",root/"bad","windows-x86_64")
            with zipfile.ZipFile(zip_path,"w") as z:
                z.writestr("build/bin/clangd.exe",b"real")
                z.writestr("build/bin/CLANGD.exe",b"alias")
            with self.assertRaises(ValueError):
                stage_provider(zip_path,"zip","clangd.exe",root/"bad2","windows-x86_64")
            with zipfile.ZipFile(zip_path,"w") as z:
                info=zipfile.ZipInfo("build/bin/clangd.exe")
                info.create_system=3
                info.external_attr=(0o120777 << 16)
                z.writestr(info,b"link")
            with self.assertRaises(ValueError):
                stage_provider(zip_path,"zip","clangd.exe",root/"bad3","windows-x86_64")


if __name__=="__main__":
    unittest.main()
