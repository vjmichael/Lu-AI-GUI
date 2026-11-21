import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from collect_original_lsp_notices import (
    collect, git_blob_sha1, validate, check_original,
)


class OriginalUpstreamNoticeIntakeTests(unittest.TestCase):
    def fixture(self, root:Path):
        original=Path(__file__).with_name(
            "lsp_original_upstream_notices_20261108.json"
        )
        manifest=json.loads(original.read_text(encoding="utf-8"))
        source=root/"upstream"
        for item in manifest["notices"]:
            data=("original upstream license test fixture: " +
                  item["provider"]+item["path"]).encode()
            item["gitBlobSha1"]=git_blob_sha1(data)
            destination=source/item["destination"]
            destination.parent.mkdir(parents=True,exist_ok=True)
            destination.write_bytes(data)
        return source,manifest

    def test_all_original_bytes_verified_without_distributor_approval(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            source,manifest=self.fixture(root)
            result=collect(manifest,root/"collected",source_root=source)
            self.assertEqual(len(result["notices"]),4)
            self.assertTrue(result["originalBytesVerified"])
            self.assertFalse(result["redistributionApproved"])
            self.assertFalse(result["dependenciesAudited"])
            self.assertFalse(result["legalReviewSigned"])
            for record in result["notices"]:
                self.assertEqual(len(record["sha256"]),64)
                self.assertFalse(record["reviewedForRedistribution"])
            self.assertTrue((root/"collected/original-upstream-notice-receipt.json").is_file())

    def test_forged_or_modified_notices_are_denied_without_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            source,manifest=self.fixture(root)
            entry=manifest["notices"][0]
            (source/entry["destination"]).write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError,"Git blob"):
                collect(manifest,root/"collected",source_root=source)
            self.assertFalse((root/"collected").exists())

    def test_extra_metadata_missing_notice_wrong_release_and_approval_denied(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            source,manifest=self.fixture(root)
            cases=[]
            row=copy.deepcopy(manifest)
            row["releaseApproved"]=True
            cases.append(row)
            row=copy.deepcopy(manifest)
            row["notices"].pop()
            cases.append(row)
            row=copy.deepcopy(manifest)
            row["notices"][0]["ref"]="main"
            cases.append(row)
            row=copy.deepcopy(manifest)
            row["notices"][0]["licenseApproved"]=True
            cases.append(row)
            row=copy.deepcopy(manifest)
            row["notices"][0]["destination"]="../escape"
            cases.append(row)
            for bad in cases:
                with self.assertRaises(ValueError):
                    validate(bad)

    def test_no_implicit_network_or_symlink_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            source,manifest=self.fixture(root)
            with self.assertRaisesRegex(ValueError,"network disabled"):
                collect(manifest,root/"empty")
            dest=source/manifest["notices"][0]["destination"]
            dest.unlink()
            try:
                dest.symlink_to(source/manifest["notices"][1]["destination"])
            except (NotImplementedError,OSError):
                return
            with self.assertRaises(ValueError):
                collect(manifest,root/"out",source_root=source)
            self.assertFalse((root/"out").exists())


if __name__=="__main__":
    unittest.main()
