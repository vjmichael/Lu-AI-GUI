"""WO7 signed candidate must deny absent upstream proof and mismatched Tauri CLI."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[2]

class SignedWO7SourceGates(unittest.TestCase):
    def setUp(self):
        self.content=(ROOT/"ide/packaging/Build-Wo7ReviewedNsis.ps1").read_text(encoding="utf-8")
    def test_signed_builder_requires_provenance_before_and_after_copy(self):
        self.assertEqual(self.content.count("--require-complete --require-upstream"), 2)
        self.assertIn("independent_resolved_dependency_license_review_missing",self.content)
        self.assertIn("source_tree_not_clean",self.content)
    def test_reviewed_cli_follows_current_tauri_minor(self):
        self.assertIn("$cliVersion -ne '2.12.4'", self.content)
        self.assertIn("no-binary-patching", self.content)
    def test_no_automatic_release_approval(self):
        self.assertIn("releaseApproved=$false", self.content)

if __name__ == "__main__":
    unittest.main()
