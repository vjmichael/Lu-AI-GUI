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
        self.assertIn('native_key = ("Return", "alt+s", "alt+o", "Return")[attempt]', fixture)
        self.assertIn("P04_GTK_NATIVE_CONFIRM_KEY=",fixture)
        # Do not return after the unreliable spatial action before trying
        # real native OS affirmative keys for the same confirmed GTK dialog.
        section=fixture.split("def _native_picker_confirm",1)[1].split(
            "def _native_chooser_still_open",1)[0]
        self.assertLess(section.index("if _x11_dialog_activation(attempt):"),
                        section.index("if _atspi_bounded_chooser_mouse(attempt):"))

        self.assertIn("key(\"ctrl+l\")", fixture)
        self.assertIn("if _native_chooser_still_open():", fixture)
        self.assertIn("def _native_chooser_diagnostics():", fixture)
        self.assertIn("def _native_parent_and_exact_child():", fixture)
        self.assertIn("P04_GTK_PARENT_LOCATION=exact_native_parent_submitted", fixture)
        self.assertIn("def _deep_visible_gtk_chooser_nodes():", fixture)
        self.assertIn("def _focus_native_gtk_file_list():", fixture)
        self.assertIn("P04_GTK_NATIVE_FILE_LIST_FOCUS=", fixture)
        self.assertIn("def _focus_gtk_file_list_via_bounded_pointer():", fixture)
        self.assertIn("P04_GTK_FILE_LIST_POINTER=verified_native_chooser_interior_attempt",
                      fixture)
        self.assertIn("if not _focus_gtk_file_list_via_bounded_pointer():", fixture)
        self.assertIn("rect.width < 480 or rect.height < 340", fixture)
        self.assertIn("rect.x + 200 < x < rect.x + rect.width - 40", fixture)

        self.assertIn("for node in _deep_visible_gtk_chooser_nodes():", fixture)
        self.assertIn("if not _focus_native_gtk_file_list():", fixture)
        self.assertIn(
            "P04_GTK_CHILD_TYPEAHEAD=native_candidate_preserved_until_real_confirmation",
            fixture)
        block=fixture.split("def _native_parent_and_exact_child():",1)[1].split(
            "def select_native_folder():",1)[0]
        self.assertIn("return True", block)
        self.assertNotIn("return _select_real_fixture_directory()", block)
        self.assertNotIn('key("space")', block)
        self.assertIn('deny("native_folder_chooser_did_not_register_project")', fixture)
        self.assertIn("_native_chooser_diagnostics()", fixture)
        self.assertIn("P04_GTK_NATIVE_CHOOSER_INVENTORY=", fixture)
        self.assertIn("and states.contains(pyatspi.STATE_SHOWING)", fixture)
        self.assertIn("and states.contains(pyatspi.STATE_VISIBLE)", fixture)
        self.assertIn("Never print raw accessible names", fixture)
        self.assertIn("deny(\"native_folder_chooser_not_accepted\")", fixture)
        self.assertIn("outcome = _wait_for_native_registered_project(seconds=20)", fixture)
        self.assertIn('if outcome != "registered_exact_root_and_webview":', fixture)
        self.assertIn("P04_NATIVE_GTK_REGISTRATION_RESULT=", fixture)
        selector=fixture.split("def select_native_folder():",1)[1].split(
            "def editor_project():",1)[0]
        self.assertNotIn("if not _native_parent_and_exact_child()", selector)
        self.assertIn("P04_GTK_EXACT_PATH_SELECTION=exact_fixture_reasserted_in_native_chooser",
                      selector)
        self.assertIn('type_text(str(ROOT))', selector)

        self.assertIn("P04_NATIVE_GTK_HOST_PICKER_RESULT=", fixture)
        self.assertIn("def _native_catalog_project_disposition():", fixture)
        self.assertIn('str(row.get("root", "")) == str(ROOT)', fixture)
        self.assertIn('str(row.get("root", "")) == str(ROOT.parent)', fixture)
        self.assertIn('observed == "exact_native_root_persisted"', fixture)
        self.assertIn('_registered_native_project_is_exposed()', fixture)
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


    def test_real_installed_gtk_wrapper_requires_ewmh_window_manager(self):
        script = Path(__file__).with_name("Test-Wo8InstalledDeb.sh").read_text(
            encoding="utf-8")
        self.assertIn('command -v openbox >/dev/null', script)
        self.assertIn('openbox >/dev/null 2>&1 &', script)
        self.assertIn('xprop -root _NET_SUPPORTING_WM_CHECK', script)
        self.assertIn('WO8_NATIVE_LINUX_DENIED:ewmh_window_manager_not_verified',script)
        self.assertIn('WO8_NATIVE_LINUX_X11_EWMH_WM=verified', script)
        self.assertIn('/usr/bin/python3 ide/app/fixtures/wo8_linux_editor_atspi.py', script)
        self.assertIn('|| deny linux_native_editor_acceptance_failed',script)


if __name__ == "__main__":
    unittest.main()
