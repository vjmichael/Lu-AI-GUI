#!/usr/bin/env python3
"""No GTK dependency: fail-closed selection policy and real fixture contract."""
from pathlib import Path
import unittest
from gtk_chooser_confirmation_policy import (
    candidate_captions, retry_requires_native_fallback
)


class ChooserPolicyTests(unittest.TestCase):
    def test_explicit_native_folder_caption_is_preferred(self):
        self.assertEqual(
            candidate_captions(["Open", "Select Folder", "Choose Folder"], 0),
            ("select folder", "choose folder", "open"))
        self.assertEqual(candidate_captions(["Open Folder", "Open"], 0),
                         ("open folder", "open"))

    def test_generic_open_is_allowed_once_not_repeated(self):
        self.assertEqual(candidate_captions(["Open"], 0), ("open",))
        for retry in (1, 2, 3):
            self.assertEqual(candidate_captions(["Open"], retry), ())
            self.assertTrue(retry_requires_native_fallback(["Open"], retry))

    def test_only_native_confirmation_vocabulary_can_be_selected(self):
        self.assertEqual(
            candidate_captions(["Delete", "Cancel", "Open New Window"], 0), ())
        self.assertEqual(candidate_captions(["select", "Open", "Cancel"], 2),
                         ("select",))

    def test_invalid_retry_does_not_expand_authority(self):
        for index in (-1, 4, 500):
            self.assertEqual(candidate_captions(["Open"], index), ())
        self.assertEqual(candidate_captions([None, 123, {"name": "Open"}], 0), ())

    def test_actual_installed_atspi_script_requires_native_outcome(self):
        fixture = Path(__file__).with_name("wo8_linux_editor_atspi.py").read_text(
            encoding="utf-8")
        self.assertIn("from gtk_chooser_confirmation_policy import candidate_captions",
                      fixture)
        self.assertIn("if _x11_dialog_activation(attempt):", fixture)
        self.assertIn("if _atspi_bounded_chooser_mouse(attempt):", fixture)
        self.assertIn("key(\"Return\")", fixture)
        self.assertIn("key(\"ctrl+l\")", fixture)
        self.assertIn("if _native_chooser_still_open():", fixture)
        self.assertIn("deny(\"native_folder_chooser_not_accepted\")", fixture)
        self.assertIn("if not _registered_native_project_is_exposed():", fixture)
        self.assertIn("deny(\"native_folder_chooser_did_not_register_project\")", fixture)
        self.assertNotIn("subprocess.run([\"gh\"", fixture)


    def test_real_windows_b5_measurement_has_one_exact_source_gate(self):
        script = Path(__file__).with_name("Test-Wo8NativeDesktopUI.ps1").read_text(
            encoding="utf-8")
        self.assertEqual(script.count("function B5-Memory("), 1,
                         "Duplicate function after final cleanup is not executable")
        self.assertEqual(script.count("b5_performance_source_commit_not_exact_checkout"), 1)
        self.assertIn("if ($env:B5_EXACT_CHECKOUT_SHA -notmatch '^[a-f0-9]{40}$' -or",
                      script)
        self.assertIn("sourceCommit=$env:B5_EXACT_CHECKOUT_SHA", script)
        self.assertIn("rssPeakObservedBytes", script)
        self.assertIn("privatePeakObservedBytes", script)
        self.assertNotIn("\n -or\n          (git rev-parse HEAD)", script)
        self.assertTrue(script.rstrip().endswith("}"),
                        "Unexpected executable block after the native app cleanup")


if __name__ == "__main__":
    unittest.main()
