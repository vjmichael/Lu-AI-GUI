"""P05 native picker repairs must use real OS UI, not shortcut source state."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[2]
class NativePickerSourceTests(unittest.TestCase):
    def test_windows_native_selection_only_in_owner_modal(self):
        s=(ROOT/"ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1").read_text()
        self.assertIn("GetWindow($hwnd,4)",s)
        self.assertIn("$dialog.Current.ProcessId -eq $proc.Id",s)
        self.assertIn("TreeScope]::Children",s)
        self.assertIn("WO8_NATIVE_WINDOWS_PICKER=real_owned_native_dialog_invoked",s)
        self.assertIn("WO8_NATIVE_WINDOWS_PICKER=path_accepted_by_installed_ui",s)
        self.assertIn("Require (Split-Path $fixed -Leaf)",s)
        self.assertNotIn("if (!$button) { Deny 'native_picker_confirmation_not_found' }",s)
    def test_linux_true_absolute_folder_and_ui_registration(self):
        s=(ROOT/"ide/app/fixtures/wo8_linux_editor_atspi.py").read_text()
        a=s.split("def select_native_folder():",1)[1].split("def editor_project():",1)[0]
        self.assertIn("type_text(str(ROOT))",a)
        self.assertNotIn("type_text(str(ROOT.parent))",a)
        self.assertNotIn("_select_real_fixture_directory()",a)
        self.assertIn("_native_picker_confirm()",a)
        self.assertIn('accessible(ROOT.name, seconds=15)',a)
        self.assertIn('deny("native_folder_chooser_not_accepted")',a)
        self.assertIn("ephemeral_fixture_was_already_registered_before_picker",a)
    def test_neither_platform_uses_internal_api_or_synthetic_acceptance(self):
        for path in ("ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1",
                     "ide/app/fixtures/wo8_linux_editor_atspi.py"):
            s=(ROOT/path).read_text()
            self.assertNotIn("invoke_tauri",s.lower())
            self.assertNotIn("window.__TAURI_INTERNALS__",s)
            self.assertNotIn("localhost:3000",s)
if __name__=="__main__":
    unittest.main()
