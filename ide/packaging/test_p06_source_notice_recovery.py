"""First-party and binary esbuild notices cannot be fabricated or borrowed."""
from pathlib import Path
import unittest
from collect_verification_licenses import UPSTREAM
ROOT=Path(__file__).resolve().parents[2]
class SourceNotices(unittest.TestCase):
    def test_local_crates_carry_actual_repo_apache_license(self):
        upstream=(ROOT/"LICENSE").read_bytes()
        for folder in ("ide/host","ide/app/src-tauri"):
            with self.subTest(package=folder):
                self.assertEqual((ROOT/folder/"LICENSE").read_bytes(),upstream)
                self.assertIn('license = "Apache-2.0"',(ROOT/folder/"Cargo.toml").read_text())
    def test_esbuild_notices_are_source_pinned_not_generic_mit_guess(self):
        text=(ROOT/"ide/packaging/collect_verification_licenses.py").read_text()
        self.assertIn('version=="0.25.9" and declared=="MIT"',text)
        self.assertIn('name in {"@esbuild/linux-x64","@esbuild/win32-x64"}',text)
        self.assertIn("if (not notices and retrieval is not None",text)
        self.assertIn('SOURCES["esbuild-0.25.9-LICENSE-MIT.txt"]',text)
        self.assertEqual(UPSTREAM["esbuild-0.25.9-LICENSE-MIT.txt"],"v0.25.9")
if __name__=="__main__":unittest.main()
