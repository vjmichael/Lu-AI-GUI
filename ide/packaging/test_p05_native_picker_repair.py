"""P05 native picker repairs must use real OS UI, not shortcut source state."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[2]
class NativePickerSourceTests(unittest.TestCase):
    def test_windows_native_selection_only_in_owner_modal(self):
        s=(ROOT/"ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1").read_text(encoding="utf-8")
        self.assertIn("GetWindow($hwnd,4)",s)
        self.assertIn("$dialog.Current.ProcessId -eq $proc.Id",s)
        self.assertIn("TreeScope]::Children",s)
        self.assertIn("WO8_NATIVE_WINDOWS_PICKER=dialog_closed_by_os_path_enter",s)
        self.assertIn("WO8_NATIVE_WINDOWS_PICKER=owned_native_dialog_confirmed",s)
        self.assertIn("WO8_NATIVE_WINDOWS_PICKER=real_owned_pane_mouse_click",s)
        self.assertIn("WO8_NATIVE_WINDOWS_PICKER=installed_hwnd_native_pane_click",s)
        self.assertIn("native_picker_still_open_in_installed_hwnd",s)
        self.assertIn("$installedRoot=WindowRoot",s)
        self.assertIn("native_folder_picker_still_open_after_confirmation",s)
        self.assertIn("ControlType.Pane",s)
        self.assertIn("mouse_event(2,0,0,0",s)
        self.assertIn("Require (Split-Path $fixed -Leaf)",s)
        self.assertNotIn("if (!$button) { Deny 'native_picker_confirmation_not_found' }",s)
    def test_linux_true_absolute_folder_and_ui_registration(self):
        s=(ROOT/"ide/app/fixtures/wo8_linux_editor_atspi.py").read_text(encoding="utf-8")
        a=s.split("def select_native_folder():",1)[1].split("def editor_project():",1)[0]
        self.assertIn("type_text(str(ROOT))",a)
        self.assertNotIn("type_text(str(ROOT.parent))",a)
        self.assertIn("_native_parent_and_exact_child()",a)
        self.assertIn("_native_picker_confirm(attempts)",a)
        self.assertIn('"select folder"',s)
        self.assertIn('"open folder"',s)
        self.assertIn('_native_dialog_ancestor(obj)',s)
        self.assertIn('accessible(ROOT.name, seconds=15)',a)
        self.assertIn('deny("native_folder_chooser_not_accepted")',a)
        self.assertIn("ephemeral_fixture_was_already_registered_before_picker",a)
    def test_restarted_project_must_reopen_through_actual_installed_ui(self):
        windows=(ROOT/"ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1").read_text(encoding="utf-8")
        mark=windows.split("    Require 'Registered projects'",1)[1]
        self.assertIn("Click 'Reopen'",mark)
        self.assertLess(mark.index("Click 'Reopen'"),mark.index("Click 'Edit'"))
        self.assertIn("Require 'Open · editor available'",mark)
        self.assertIn("WO8_NATIVE_PROJECT_REOPEN=real_installed_ui_reactivated",mark)
        linux=(ROOT/"ide/app/fixtures/wo8_linux_editor_atspi.py").read_text(encoding="utf-8")
        editor=linux.split("def editor_project():",1)[1].split("def main():",1)[0]
        self.assertIn('_activate_exact_project_button("Reopen project "',editor)
        self.assertIn('_activate_exact_project_button("Edit project "',editor)
        self.assertLess(editor.index('"Reopen project "'),editor.index('"Edit project "'))
        self.assertIn('accessible("Source explorer and editor", seconds=15)',editor)
        frontend=(ROOT/"ide/app/web/main.jsx").read_text(encoding="utf-8")
        self.assertIn("Closed · ready to reopen",frontend)
        self.assertIn(">Reopen</button>",frontend)
        self.assertIn(">Edit</button>",frontend)
    def test_windows_git_one_use_approval_stays_under_real_owned_dialog(self):
        windows=(ROOT/"ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1").read_text(encoding="utf-8")
        section=windows.split("function Confirm-NativeOneUseDialog(",1)[1].split(
            "\ntry {\n  $proc=Start-Process",1)[0]
        self.assertIn("if ($win.Current.ProcessId -ne $proc.Id",section)
        self.assertIn("if ($hwnd -eq [IntPtr]::Zero -or $hwnd -eq $proc.MainWindowHandle)",section)
        self.assertIn("$name.IndexOf($titleFragment",section)
        self.assertIn("[string]$child.Current.Name -cne $approvalCaption",section)
        self.assertIn("$child.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)",section)
        self.assertIn("$rect=$child.Current.BoundingRectangle",section)
        self.assertIn("$windowBounds=$win.Current.BoundingRectangle",section)
        self.assertIn("exact_owned_native_text_pane_mouse_click",section)
        self.assertIn("real_native_consent_dialog_not_observed",section)
        self.assertNotIn("window.__TAURI_INTERNALS__",section)

    def test_linux_gtk_active_chooser_not_hidden_role_and_exact_project_ui(self):
        source=(ROOT/"ide/app/fixtures/wo8_linux_editor_atspi.py").read_text(encoding="utf-8")
        checker=source.split("def _native_chooser_still_open():",1)[1].split(
            "def _registered_native_project_is_exposed():",1)[0]
        self.assertIn("pyatspi.STATE_SHOWING", checker)
        self.assertIn("pyatspi.STATE_VISIBLE", checker)
        self.assertIn('if "file chooser" not in role:', checker)
        self.assertIn("_native_dialog_ancestor(obj)", source)
        self.assertIn('if observed == "exact_native_root_persisted" and _registered_native_project_is_exposed():',source)
        self.assertIn('deny("native_folder_chooser_did_not_register_project")',source)
    def test_windows_git_committed_head_must_really_change(self):
        source=(ROOT/"ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1").read_text(encoding="utf-8")
        self.assertIn("$gitDeadline=(Get-Date).AddSeconds(24)",source)
        self.assertIn("git -C $fixed rev-parse HEAD",source)
        self.assertIn("WO9_NATIVE_COMMIT_OBSERVATION=unchanged_head",source)
        self.assertIn("Deny 'wo9_native_commit_not_observed'",source)
        self.assertIn("git -C $fixed diff-tree --no-commit-id",source)
        self.assertIn("WO9_NATIVE_STAGE_EXACT_INDEX_AND_SOURCE_BLOB=PASS",source)
        self.assertIn("git -C $fixed rev-parse :main.rs",source)
        self.assertIn("git -C $fixed rev-parse 'HEAD:main.rs'",source)
        self.assertIn("git -C $fixed rev-parse 'HEAD^'",source)
        self.assertIn("wo9_native_index_not_clean_after_commit",source)
        self.assertIn("Find-Control ('Local commit: '+$after) '' 15 -Exact",source)
        self.assertNotIn("Require 'Local commit'",source)
        self.assertLess(source.index("WO9_NATIVE_STAGE_EXACT_INDEX_AND_SOURCE_BLOB=PASS"), source.index("Click 'Confirm separate local commit (no push)'"))

    def test_split_file_selection_waits_for_fresh_enabled_exact_control(self):
        source=(ROOT/"ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1").read_text(encoding="utf-8")
        b4=source.split("if ($Scenario -in @('B4SplitEditor','Dev01Modern'))",1)[1].split("function Get-B4NativeEditors",1)[0]
        self.assertIn("Find-Control 'second.rs' 'Button' 2 -Exact",b4)
        self.assertIn("$secondFile.Current.IsEnabled",b4)
        self.assertIn("$secondFile.Current.IsOffscreen",b4)
        self.assertEqual(b4.count(".Invoke()"),1)
        self.assertLess(b4.index("Source editor text · First editor: second.rs"), b4.index("Click 'Toggle split editor'"))

    def test_gtk_spatial_acceptance_only_inside_native_visible_chooser(self):
        src=(ROOT/"ide/app/fixtures/wo8_linux_editor_atspi.py").read_text(encoding="utf-8")
        section=src.split("def _atspi_bounded_chooser_mouse(attempt):",1)[1].split(
            "def _native_picker_confirm(",1)[0]
        for term in ("xdotool", "getdisplaygeometry", "pyatspi.STATE_SHOWING",
                     "pyatspi.STATE_VISIBLE", "DESKTOP_COORDS",
                     "rect.width < 320", "rect.height < 180",
                     "rect.x + rect.width > screen_w", "rect.y + rect.height > screen_h",
                     '"mousemove"', '"click"'):
            self.assertIn(term,section)
        self.assertIn("if _atspi_bounded_chooser_mouse(attempt):",src)
        self.assertIn('if observed == "exact_native_root_persisted" and _registered_native_project_is_exposed():',src)
        self.assertNotIn("window.__TAURI_INTERNALS__",section)
        self.assertNotIn("invoke_tauri",section)

    def test_linux_os_chooser_keyboard_fallback_requires_real_native_active_window(self):
        source=(ROOT/"ide/app/fixtures/wo8_linux_editor_atspi.py").read_text(encoding="utf-8")
        section=source.split("def _x11_dialog_activation(attempt):",1)[1].split(
            "def _native_picker_confirm(",1)[0]
        for term in ("_native_chooser_still_open()", "xdotool", "getactivewindow",
                     "getwindowname", "windowactivate", "Return", "alt+o",
                     "unrecognized_active_window_title"):
            self.assertIn(term, section)
        self.assertIn("if _x11_dialog_activation(attempt):",source)
        self.assertIn('if observed == "exact_native_root_persisted" and _registered_native_project_is_exposed():',source)
        self.assertIn('deny("native_folder_chooser_did_not_register_project")',source)
        self.assertNotIn("window.__TAURI_INTERNALS__",section)
        self.assertNotIn("invoke_tauri",section)

    def test_linux_real_dialog_prefers_exact_select_and_spatial_acceptance(self):
        native=(ROOT/"ide/app/fixtures/wo8_linux_editor_atspi.py").read_text(encoding="utf-8")
        chooser=native.split("def _native_picker_confirm(",1)[1].split(
            "def _native_chooser_still_open():",1)[0]
        self.assertIn("candidate_captions(",chooser)
        self.assertIn('getExtents(pyatspi.DESKTOP_COORDS)',chooser)
        self.assertIn('ranked.sort(key=lambda record: record[:4])',chooser)
        self.assertIn('if attempt > 0 and not ranked:',chooser)
        self.assertIn('_native_dialog_ancestor(obj)',chooser)
        self.assertIn('native_dialog_confirmation_not_found',chooser)
        parent_route=native.split('def _native_parent_and_exact_child():',1)[1].split(
            'def select_native_folder():',1)[0]
        self.assertIn('if _select_real_fixture_directory():\n        return True',parent_route)
        self.assertIn('return _select_real_fixture_directory()',parent_route)
        self.assertIn('parent_child_candidate = _native_parent_and_exact_child()',native)
        self.assertIn('if parent_child_candidate:',native)
        self.assertIn('_native_parent_and_exact_child()',native)
        self.assertIn('type_text(str(ROOT.parent))',native)
        self.assertIn('type_text(str(ROOT))',native)
    def test_installed_windows_startup_only_uses_real_os_focus_before_uia(self):
        native=(ROOT/"ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1").read_text(encoding="utf-8")
        startup=native.split('function Warm-InstalledWebViewAccessibility {',1)[1].split(
            'function Find-Control(',1)[0]
        self.assertIn("Find-Control 'Workspace projects' '' 6",startup)
        self.assertIn('[P04NativeWebViewWindows]::GetForegroundWindow()',startup)
        self.assertIn('[System.Windows.Forms.SendKeys]::SendWait(\'{TAB}\')',startup)
        self.assertIn('installed_webview_accessibility_not_ready_after_native_focus',startup)
        self.assertIn("Warm-InstalledWebViewAccessibility\n  Require 'Workspace projects'",native)
        self.assertNotIn('window.__TAURI_INTERNALS__',startup)

    def test_wo9_git_head_never_overwrites_worker_autostart_baseline(self):
        native=(ROOT/"ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1").read_text(encoding="utf-8")
        self.assertIn("$workerBaseline=@{}",native)
        self.assertIn("$workerBaseline[$name]=BoundedProcess $name",native)
        self.assertIn("foreach($name in $workerBaseline.Keys)",native)
        self.assertIn("(BoundedProcess $name) -gt $workerBaseline[$name]",native)
        self.assertIn("$gitHeadBefore=(git -C $fixed rev-parse HEAD).Trim()",native)
        self.assertIn("$gitHeadBefore -eq $after",native)
        self.assertNotIn("$before.Keys",native)
        self.assertNotIn("$before=(git",native)
    def test_linux_wo9_zip_destination_is_only_real_native_chooser(self):
        native=(ROOT/"ide/app/fixtures/wo9_linux_workflow_atspi.py").read_text(encoding="utf-8")
        self.assertIn("ui._native_chooser_still_open()",native)
        self.assertIn("ui._native_picker_confirm()",native)
        self.assertIn('deny("export_native_chooser_remained_open")',native)
        self.assertNotIn('ui.activate(caption',native)
        self.assertIn("archive_bytes_differ_from_source",native)

    def test_neither_platform_uses_internal_api_or_synthetic_acceptance(self):
        for path in ("ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1",
                     "ide/app/fixtures/wo8_linux_editor_atspi.py"):
            s=(ROOT/path).read_text(encoding="utf-8")
            self.assertNotIn("invoke_tauri",s.lower())
            self.assertNotIn("window.__TAURI_INTERNALS__",s)
            self.assertNotIn("localhost:3000",s)
if __name__=="__main__":
    unittest.main()
