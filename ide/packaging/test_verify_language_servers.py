import hashlib
import json
import pathlib
import tempfile
import unittest
from verify_language_servers import verify

MANIFEST = pathlib.Path(__file__).with_name("language-servers.manifest.json")


class LanguageServerBundlePolicyTests(unittest.TestCase):
    def test_only_catalogued_server_candidates_are_allowed_without_release(self):
        with tempfile.TemporaryDirectory() as temp:
            self.assertEqual(verify(MANIFEST, pathlib.Path(temp)), [])
            with self.assertRaises(ValueError):
                verify(MANIFEST, pathlib.Path(temp), release=True)

    def test_a_real_pinned_bundle_requires_notice_and_exact_hash(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            assets = root / "bin"
            assets.mkdir()
            (assets / "windows.exe").write_bytes(b"native fixture")
            (assets / "linux").write_bytes(b"linux fixture")
            (root / "LICENSE").write_text("MIT license fixture", encoding="utf-8")
            manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
            row = manifest["servers"][0]
            row["mode"] = "bundled"
            row["releaseVersion"] = "fixture-1"
            row["copyrightReviewed"] = True
            row["dependenciesAudited"] = True
            row["assetByPlatform"] = {
                "windows-x86_64": "bin/windows.exe",
                "linux-x86_64": "bin/linux",
            }
            row["sha256ByPlatform"] = {
                "windows-x86_64": hashlib.sha256(b"native fixture").hexdigest(),
                "linux-x86_64": hashlib.sha256(b"linux fixture").hexdigest(),
            }
            row["licenseNoticeFiles"] = ["LICENSE"]
            row["nativeResourceManifestReviewed"] = True
            row["nativeResourceFilesByPlatform"] = {
                "windows-x86_64": {}, "linux-x86_64": {},
            }
            manifest["releaseApproved"] = True
            temporary = root / "manifest.json"
            temporary.write_text(json.dumps(manifest), encoding="utf-8")
            self.assertEqual(verify(temporary, root, release=True), [row["id"]])
            (assets / "windows.exe").write_bytes(b"replaced")
            with self.assertRaises(ValueError):
                verify(temporary, root, release=True)

    def test_missing_notice_and_traversal_fail_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
            row = manifest["servers"][0]
            row.update({
                "mode": "bundled",
                "releaseVersion": "fixture-1",
                "copyrightReviewed": True,
                "dependenciesAudited": True,
                "assetByPlatform": {
                    "windows-x86_64": "../outside.exe",
                    "linux-x86_64": "bin/linux"
                },
                "sha256ByPlatform": {
                    "windows-x86_64": "0" * 64,
                    "linux-x86_64": "0" * 64,
                },
                "licenseNoticeFiles": ["LICENSE"]
            })
            manifest["releaseApproved"] = True
            path = root / "manifest.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(ValueError):
                verify(path, root, release=True)


    def test_clangd_binary_only_approval_is_denied_even_with_notices(self):
        from verify_language_servers import reviewed_native_resources
        with tempfile.TemporaryDirectory() as temp:
            root=pathlib.Path(temp)
            (root/"clangd.exe").write_bytes(b"candidate")
            (root/"original.dll").write_bytes(b"original sibling")
            row={"id":"clangd","category":"native",
                 "nativeResourceManifestReviewed":True,
                 "nativeResourceFilesByPlatform":{
                   "windows-x86_64":{},
                   "linux-x86_64":{},
                 }}
            with self.assertRaisesRegex(ValueError,"LLVM resources"):
                reviewed_native_resources(row,root,"windows-x86_64")
            row["nativeResourceFilesByPlatform"]["windows-x86_64"]={
                "bin/original.dll":{
                    "path":"original.dll",
                    "sha256":hashlib.sha256(b"original sibling").hexdigest()
                }
            }
            self.assertEqual(len(reviewed_native_resources(
                row,root,"windows-x86_64")),1)
            row["nativeResourceFilesByPlatform"]["windows-x86_64"]={
                "../outside.dll":{
                    "path":"original.dll",
                    "sha256":hashlib.sha256(b"original sibling").hexdigest()
                }
            }
            with self.assertRaisesRegex(ValueError,"unsafe"):
                reviewed_native_resources(row,root,"windows-x86_64")
            row["nativeResourceFilesByPlatform"]["windows-x86_64"]={
                "bin/original.dll":{"path":"original.dll","sha256":"0"*64}
            }
            with self.assertRaisesRegex(ValueError,"SHA mismatch"):
                reviewed_native_resources(row,root,"windows-x86_64")


if __name__ == "__main__":
    unittest.main()
