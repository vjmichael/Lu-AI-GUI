"""Exact r-efi crate AUTHORS from upstream tags; no automatic LGPL selection."""
from pathlib import Path
import tempfile
import unittest
from collect_verification_licenses import (
    verified_r_efi_authors, git_blob_sha, R_EFI_AUTHOR_GIT_BLOBS,
    R_EFI_LICENSE, Denied
)
ROOT=Path(__file__).resolve().parents[2]
class OriginalCrateNotices(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo=Path(self.tmp.name)
        self.vendor=self.repo/"ide/packaging/upstream-crate-notices"
        self.vendor.mkdir(parents=True)
        self.crate=self.repo/"cargo-registry-r-efi"
        self.crate.mkdir()
    def copy_released_notice(self,version):
        text=(ROOT/"ide/packaging/upstream-crate-notices"/
              ("r-efi-"+version+"-AUTHORS.txt")).read_bytes()
        (self.vendor/("r-efi-"+version+"-AUTHORS.txt")).write_bytes(text)
        (self.crate/"AUTHORS").write_bytes(text)
        return text
    def test_both_full_upstream_notice_texts(self):
        for version in ("5.3.0","6.0.0"):
            with self.subTest(version=version):
                text=self.copy_released_notice(version)
                self.assertIn(b"AUTHORS-MIT:",text)
                self.assertIn(b"AUTHORS-ASL:",text)
                self.assertIn(b"AUTHORS-LGPL:",text)
                self.assertIn(b"COPYRIGHT:",text)
                self.assertEqual(git_blob_sha(text),R_EFI_AUTHOR_GIT_BLOBS[version])
                self.assertEqual(verified_r_efi_authors("cargo","r-efi",version,
                    R_EFI_LICENSE,self.crate,self.repo).name,"AUTHORS")
    def test_changed_upstream_original_is_denied(self):
        self.copy_released_notice("6.0.0")
        (self.crate/"AUTHORS").write_text("MIT license maybe")
        with self.assertRaisesRegex(Denied,"untrusted_r_efi_original_authors"):
            verified_r_efi_authors("cargo","r-efi","6.0.0",
                R_EFI_LICENSE,self.crate,self.repo)
    def test_other_version_no_borrowed_license(self):
        self.copy_released_notice("6.0.0")
        self.assertIsNone(verified_r_efi_authors("cargo","r-efi","6.1.0",
            R_EFI_LICENSE,self.crate,self.repo))
    def test_wrong_expression_cannot_silently_select_mit(self):
        self.copy_released_notice("5.3.0")
        with self.assertRaisesRegex(Denied,"r_efi_exact_license_expression_changed"):
            verified_r_efi_authors("cargo","r-efi","5.3.0",
                "MIT",self.crate,self.repo)
    def test_symlinked_crate_authors_rejected(self):
        self.copy_released_notice("5.3.0")
        (self.crate/"AUTHORS").unlink()
        try:
            (self.crate/"AUTHORS").symlink_to(self.vendor/"r-efi-5.3.0-AUTHORS.txt")
        except (OSError,NotImplementedError):
            self.skipTest("runner cannot create symlinks")
        with self.assertRaisesRegex(Denied,"untrusted_r_efi_original_authors"):
            verified_r_efi_authors("cargo","r-efi","5.3.0",
                R_EFI_LICENSE,self.crate,self.repo)
if __name__=="__main__":unittest.main()
