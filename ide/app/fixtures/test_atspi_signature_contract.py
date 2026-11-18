#!/usr/bin/env python3
"""P02 offline AST guard: GTK/AT-SPI activation keyword names must match helper.

This is a source contract check, NOT evidence that a native installed window
was launched. It prevents the documented WO8/WO9 Python keyword regression.
"""
import ast
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent
SCRIPTS = [
    ROOT / "wo8_linux_editor_atspi.py",
    ROOT / "wo9_linux_workflow_atspi.py",
    ROOT / "wo9_linux_protected_graph_atspi.py",
]

class AtspiSignatureContract(unittest.TestCase):
    def test_all_activation_keyword_names_match_helper(self):
        module = ast.parse(SCRIPTS[0].read_text(encoding="utf-8"))
        signatures = {
            node.name: {
                argument.arg for argument in (
                    node.args.posonlyargs + node.args.args + node.args.kwonlyargs
                )
            }
            for node in module.body if isinstance(node, ast.FunctionDef)
        }
        self.assertIn("timeout", signatures["activate"])
        self.assertNotIn("seconds", signatures["activate"])
        self.assertIn("seconds", signatures["accessible"])
        for source in SCRIPTS:
            with self.subTest(script=source.name):
                tree = ast.parse(source.read_text(encoding="utf-8"))
                for node in ast.walk(tree):
                    if not isinstance(node, ast.Call):
                        continue
                    target = node.func
                    name = target.id if isinstance(target, ast.Name) else (
                        target.attr if isinstance(target, ast.Attribute)
                        and isinstance(target.value, ast.Name)
                        and target.value.id == "ui" else None
                    )
                    if name in {"activate", "accessible"}:
                        allowed = signatures[name]
                        unexpected = {arg.arg for arg in node.keywords
                                      if arg.arg is None or arg.arg not in allowed}
                        self.assertFalse(unexpected,
                                         f"{source.name}:{node.lineno} "
                                         f"{name} unexpected keyword {unexpected}")

    def test_portfolio_toggle_observes_checked_state_and_never_double_toggles(self):
        from types import SimpleNamespace
        tree = ast.parse(SCRIPTS[0].read_text())
        fn = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                  and node.name == "choose_open_workflow_project")
        class Clock:
            now = 0
            def monotonic(self): return self.now
            def sleep(self, _): self.now += 1
        class Checkbox:
            name = "project (open)"
            checked = False
            count = 0
            should_check = True
            def getRoleName(self): return "check box"
            def getState(self):
                return SimpleNamespace(contains=lambda flag: flag != 5 or self.checked)
            def queryAction(self):
                return SimpleNamespace(nActions=1, getName=lambda _: "toggle", doAction=self.toggle)
            def toggle(self, _):
                self.count += 1
                self.checked = self.should_check
                return True
        checkbox = Checkbox()
        ns = {"time": Clock(), "pyatspi": SimpleNamespace(STATE_VISIBLE=1,
              STATE_SHOWING=2, STATE_ENABLED=3, STATE_SENSITIVE=4, STATE_CHECKED=5),
              "elements": lambda: [checkbox], "_native_dialog_ancestor": lambda _: False,
              "deny": lambda reason: (_ for _ in ()).throw(RuntimeError(reason))}
        exec(compile(ast.Module(body=[fn], type_ignores=[]), "portfolio_policy", "exec"), ns)
        ns["choose_open_workflow_project"]("project", seconds=3)
        self.assertTrue(checkbox.checked)
        self.assertEqual(checkbox.count, 1)
        checkbox.checked, checkbox.should_check, checkbox.count = False, False, 0
        with self.assertRaisesRegex(RuntimeError, "exact_checked_state_not_observed"):
            ns["choose_open_workflow_project"]("project", seconds=3)
        self.assertEqual(checkbox.count, 1)
        checkbox.count = 0
        ns["elements"] = lambda: [checkbox, Checkbox()]
        with self.assertRaisesRegex(RuntimeError, "exact_checked_state_not_observed"):
            ns["choose_open_workflow_project"]("project", seconds=3)
        self.assertEqual(checkbox.count, 0)
        for script in SCRIPTS[1:]:
            self.assertIn("ui.choose_open_workflow_project(", script.read_text())


    def test_default_windows_debugger_gate_matches_current_denial_and_never_enables(self):
        script = (ROOT / "Test-Wo8NativeDesktopUI.ps1").read_text()
        guard = script.split("if ($Scenario -eq 'Dev045ProjectDebuggerDenied')", 1)[1].split(
            "if ($Scenario -eq 'ProtectedDebuggerCancellation')", 1)[0]
        web = (ROOT.parent / "web" / "main.jsx").read_text()
        self.assertIn("protected native release entitlement", web)
        self.assertIn("Require 'protected native release entitlement'", guard)
        self.assertIn("Find-Control 'Debug last verified project build' 'Button'", guard)
        self.assertIn("if ($debug.Current.IsEnabled)", guard)
        self.assertIn("default_unsigned_installer_exposed_project_execution", guard)
        self.assertIn("(ProtectedWorkerCount) -ne $workerBefore", guard)
        self.assertIn("-ne $sourceBefore", guard)
        self.assertNotIn("Click 'Debug last verified project build'", guard)

    def test_windows_native_clipboard_requires_new_os_copy_and_exact_digest(self):
        script = (ROOT / "Test-B2InstalledDraftRecovery.ps1").read_text()
        self.assertIn("Exact installed source editor failed to acquire OS keyboard focus", script)
        self.assertIn("Native clipboard did not observe exact changed editor text", script)
        self.assertIn("[System.Windows.Forms.Clipboard]::Clear()", script)
        self.assertEqual(script.count("draftSha256='+$exactDraftSha+' "), 2)
        self.assertIn("Explicit Save did not restore exact privately preserved draft bytes", script)
        self.assertIn("Installed editor wrote source before explicit Save", script)

    def test_native_acceptance_retains_review_and_scoped_editor_identity(self):
        script = (ROOT / "Test-Wo8NativeDesktopUI.ps1").read_text()
        self.assertIn("'Move file','Trash file','Restore file','Toggle split editor'", script)
        self.assertIn("$fresh.Current.HasKeyboardFocus", script)
        self.assertIn("'Confirm local Git commit' 'Commit exact staged file' 'WO9 installed reviewed Git source'", script)
        self.assertIn("$expected='Source editor text · First editor: '+$leaf", script)
        linux = (ROOT / "b4_linux_installed_split_atspi.py").read_text()
        self.assertIn("for obj in _native_source_editor_nodes():", linux)
        self.assertIn("pyatspi.STATE_FOCUSED", linux)
        self.assertIn("expected_two_distinct_native_atspi_editor_surfaces_", linux)

if __name__ == "__main__":
    unittest.main()
