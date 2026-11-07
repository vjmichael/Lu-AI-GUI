"""Reviewed version-specific original license sources, never generic SPDX guesses."""
import tempfile
import unittest
from pathlib import Path
from collect_verification_licenses import (
    CURATED_MISSING_NOTICES,PROTOC_UPSTREAM_FAMILY,WEBVIEW_EXACT_CRATES,OBJC2_UPSTREAM_POLICY,EXTERNAL_NATIVE_SDK_SOURCE_RULES,verified_curated_notices,git_blob_sha,Denied
)
ROOT=Path(__file__).resolve().parents[2]
class CuratedOriginalNotices(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo=Path(self.tmp.name)
        self.folder=self.repo/"ide/packaging/upstream-crate-notices"
        self.folder.mkdir(parents=True)
    def load(self,rule):
        for item in rule["files"]:
            source=ROOT/item["checkedIn"]
            dest=self.repo/item["checkedIn"]
            dest.write_bytes(source.read_bytes())
    def test_only_exact_immutable_upstream_full_license_texts(self):
        self.assertEqual(set(CURATED_MISSING_NOTICES),
                         {"alloc-stdlib:0.3.0","tauri-plugin:2.7.2"} |
                         {name+":3.3.0" for name in PROTOC_UPSTREAM_FAMILY} |
                         {name+":"+version for name,version in WEBVIEW_EXACT_CRATES} |
                         {r["name"]+":"+r["version"] for r in OBJC2_UPSTREAM_POLICY} |
                         {r["name"]+":"+r["version"] for r in EXTERNAL_NATIVE_SDK_SOURCE_RULES})
        for name_version,rule in CURATED_MISSING_NOTICES.items():
            with self.subTest(name_version=name_version):
                self.load(rule)
                name,version=name_version.split(":",1)
                rows=verified_curated_notices(self.repo,"cargo",name,version,
                                              rule["declaredLicense"])
                self.assertEqual(len(rows),len(rule["files"]))
                for filename,source in rows:
                    self.assertEqual(git_blob_sha(filename.read_bytes()),source["blobSha"])
                    self.assertNotIn(b"\r",filename.read_bytes())
                    self.assertTrue(source["url"].startswith("https://raw.githubusercontent.com/"))
    def test_entire_exact_vendored_protoc_family_has_complete_original_mit(self):
        for name in PROTOC_UPSTREAM_FAMILY:
            rule=CURATED_MISSING_NOTICES[name+":3.3.0"]
            self.assertEqual(rule["declaredLicense"],"MIT")
            self.load(rule)
            result=verified_curated_notices(self.repo,"cargo",name,"3.3.0","MIT")
            self.assertEqual(len(result),1)
            self.assertEqual(git_blob_sha(result[0][0].read_bytes()),
                             result[0][1]["blobSha"])
    def test_three_exact_webview_bindings_use_pinned_original_mit(self):
        for name,version in WEBVIEW_EXACT_CRATES:
            rule=CURATED_MISSING_NOTICES[name+":"+version]
            self.assertEqual(rule["declaredLicense"],"MIT")
            self.load(rule)
            rows=verified_curated_notices(self.repo,"cargo",name,version,"MIT")
            self.assertEqual(len(rows),1)
            self.assertEqual(git_blob_sha(rows[0][0].read_bytes()),rows[0][1]["blobSha"])
    def test_upstream_markdown_notice_survives_windows_checkout(self):
        attr=(ROOT/".gitattributes").read_text(encoding="utf-8")
        self.assertIn("ide/packaging/upstream-crate-notices/*.md text eol=lf",attr)
        src=(ROOT/"ide/packaging/upstream-crate-notices/objc2-family-LICENSE.md").read_bytes()
        self.assertNotIn(b"\r",src)
        self.assertEqual(git_blob_sha(src),"540ae4c81adb5f64a9f61e222b688880da24300e")
    def test_tagged_objc2_original_policy_preserves_apple_warning(self):
        self.assertEqual(len(OBJC2_UPSTREAM_POLICY),19)
        for item in OBJC2_UPSTREAM_POLICY:
            rule=CURATED_MISSING_NOTICES[item["name"]+":"+item["version"]]
            self.load(rule)
            rows=verified_curated_notices(self.repo,"cargo",item["name"],item["version"],
                                           item["declared"])
            self.assertEqual(len(rows),1)
            actual=rows[0][0].read_bytes()
            self.assertIn(b"Apple SDKs",actual)
            self.assertIn(b"unclear whether distributing derived works",actual)
            self.assertEqual(git_blob_sha(actual),rows[0][1]["blobSha"])
    def test_native_external_seven_exact_license_pairs_and_source_proof(self):
        self.assertEqual(len(EXTERNAL_NATIVE_SDK_SOURCE_RULES),7)
        for r in EXTERNAL_NATIVE_SDK_SOURCE_RULES:
            rule=CURATED_MISSING_NOTICES[r["name"]+":"+r["version"]]
            self.assertEqual(len(rule["files"]),2)
            self.load(rule)
            rows=verified_curated_notices(self.repo,"cargo",r["name"],r["version"],r["declared"])
            self.assertEqual(len(rows),2)
            for original,source in rows:
                self.assertEqual(git_blob_sha(original.read_bytes()),source["blobSha"])
                self.assertNotIn(b"\r",original.read_bytes())
    def test_mutated_notice_fails_closed(self):
        rule=CURATED_MISSING_NOTICES["alloc-stdlib:0.3.0"]
        self.load(rule)
        target=self.repo/rule["files"][0]["checkedIn"]
        target.write_bytes(target.read_bytes()+b"\n")
        with self.assertRaisesRegex(Denied,"curated_notice_differs_from_exact_original_release"):
            verified_curated_notices(self.repo,"cargo","alloc-stdlib","0.3.0","BSD-3-Clause")
    def test_missing_upstream_text_fails_closed(self):
        with self.assertRaisesRegex(Denied,"missing_or_unsafe_curated_source_notice"):
            verified_curated_notices(self.repo,"cargo","tauri-plugin","2.7.2","Apache-2.0 OR MIT")
    def test_no_borrowing_wrong_version_or_license_choice(self):
        rule=CURATED_MISSING_NOTICES["tauri-plugin:2.7.2"]
        self.load(rule)
        self.assertEqual(verified_curated_notices(self.repo,"cargo","tauri-plugin","2.7.3",
                                                  rule["declaredLicense"]),[])
        with self.assertRaisesRegex(Denied,"curated_notice_exact_license_expression_drift"):
            verified_curated_notices(self.repo,"cargo","tauri-plugin","2.7.2","MIT")
    def test_full_dual_license_remains_undecided(self):
        self.assertEqual(len(CURATED_MISSING_NOTICES["tauri-plugin:2.7.2"]["files"]),2)
        self.assertEqual(CURATED_MISSING_NOTICES["tauri-plugin:2.7.2"]["declaredLicense"],
                         "Apache-2.0 OR MIT")
if __name__=="__main__":unittest.main()
