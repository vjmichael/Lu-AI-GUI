"""Real verification jobs must not silently omit or duplicate upstream receipt."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[2]/".github/workflows"

class CIReceiptTests(unittest.TestCase):
    def verify(self,body,expected_jobs):
        self.assertEqual(body.count("python ../packaging/upstream_notice_receipt.py")
                         +body.count("python ide/packaging/upstream_notice_receipt.py"),expected_jobs)
        self.assertEqual(body.count("python ../packaging/collect_verification_licenses.py")
                         +body.count("python ide/packaging/collect_verification_licenses.py"),expected_jobs)
        self.assertEqual(body.count("--upstream-receipt"),expected_jobs)
        self.assertEqual(body.count("--require-upstream"),expected_jobs)
        self.assertEqual(body.count("@tauri-apps/cli@2.12.4"),expected_jobs)
        for line in body.splitlines():
            if "collect_verification_licenses.py" in line:
                self.assertEqual(line.count("--upstream-receipt"),1)
            if "upstream_notice_receipt.py" in line:
                self.assertEqual(line.count("--output"),1)
    def test_wo9_windows_linux_genuine_native_chain(self):
        source=(ROOT/"ide-wo9-installed-native.yml").read_text(encoding="utf-8")
        windows,linux=source.split("  installed-linux-gtk-zip:",1)
        self.verify(windows,1)
        self.verify(linux,1)
    def test_wo8_each_native_platform_has_complete_source_chain(self):
        for name in ("ide-wo8-installed-windows.yml","ide-wo8-installed-linux.yml"):
            with self.subTest(platform=name):
                self.verify((ROOT/name).read_text(encoding="utf-8"),1)
    def test_e2_both_windows_installer_builds_include_upstream(self):
        for name in ("ide-e2-supervised-lsp.yml","ide-e2-unsigned-installed-smoke.yml"):
            with self.subTest(source=name):
                self.verify((ROOT/name).read_text(encoding="utf-8"),1)
if __name__=="__main__": unittest.main()
