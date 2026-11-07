"""The temporary export accepts only a pinned source lock, never credentials."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[2]
class ExportPolicyTests(unittest.TestCase):
    def test_only_two_public_lock_paths_and_pinned_guard(self):
        s=(ROOT/"ide/packaging/p06_emit_pinned_lock_b64.py").read_text()
        self.assertIn("result=check(root,platform)",s)
        self.assertIn('ide/app/src-tauri/Cargo.lock',s)
        self.assertIn('ide/app/package-lock.json',s)
        self.assertIn("lockfile_changed_between_review_and_export",s)
        self.assertNotIn("GITHUB_TOKEN",s)
        self.assertNotIn("os.environ",s)
    def test_only_pinned_linux_export_in_read_only_ci(self):
        s=(ROOT/".github/workflows/ide-p06-dependency-evidence.yml").read_text()
        self.assertIn("if: matrix.os == 'ubuntu-24.04'",s)
        self.assertIn("python ide/packaging/p06_emit_pinned_lock_b64.py",s)
        self.assertIn("contents: read",s)
        self.assertNotIn("contents: write",s)
if __name__=="__main__":unittest.main()
