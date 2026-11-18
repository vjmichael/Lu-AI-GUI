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
                    if name in {"activate", "accessible", "native_workflow_review_control", "activate_native_workflow_review"}:
                        allowed = signatures[name]
                        unexpected = {arg.arg for arg in node.keywords
                                      if arg.arg is None or arg.arg not in allowed}
                        self.assertFalse(unexpected,
                                         f"{source.name}:{node.lineno} "
                                         f"{name} unexpected keyword {unexpected}")

    def test_dev035_native_build_button_scoped_deep_disabled_and_fail_closed(self):
        import sys
        from types import SimpleNamespace, ModuleType
        from unittest.mock import patch
        tree = ast.parse((ROOT / "dev035_jobs_installed_atspi.py").read_text(encoding="utf-8"))
        fn = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                  and node.name == "native_protected_build_button")
        class Node:
            def __init__(self, name, role, children=(), enabled=False, visible=True, dialog=False):
                self.name, self.role, self.children = name, role, list(children)
                self.enabled, self.visible, self.dialog = enabled, visible, dialog
            @property
            def childCount(self): return len(self.children)
            def getRoleName(self): return self.role
            def getState(self):
                return SimpleNamespace(contains=lambda state: self.visible if state == 1 else self.enabled)
        class Clock:
            now = 0
            def monotonic(self): return self.now
            def sleep(self, _): self.now += 1
        btn = Node("Request protected Build/Test", "push button")
        outside = Node(btn.name, btn.role, enabled=True)
        composer = Node("Protected native build and test", "region", [btn])
        deep = composer
        for _ in range(16): deep = Node("", "panel", [deep])
        workspace = Node("Execution intent review", "landmark", [deep])
        api = ModuleType("wo8_linux_editor_atspi")
        api.elements = lambda: [outside, workspace]
        api._native_dialog_ancestor = lambda node: node.dialog
        clock = Clock()
        ns = {"pyatspi": SimpleNamespace(STATE_VISIBLE=1, STATE_ENABLED=2, STATE_SENSITIVE=3),
              "children_of": lambda node: iter(node.children),
              "deny": lambda reason: (_ for _ in ()).throw(RuntimeError(reason))}
        exec(compile(ast.Module(body=[fn], type_ignores=[]), "dev035_scope", "exec"), ns)
        lookup = ns["native_protected_build_button"]
        with patch.dict(sys.modules, {"wo8_linux_editor_atspi": api, "time": clock}):
            self.assertIs(lookup(seconds=2), btn)
            btn.enabled = True
            with self.assertRaisesRegex(RuntimeError, "admitted_protected_execution"):
                lookup(seconds=2)
            btn.enabled = False
            for children in ([btn, Node(btn.name, btn.role)], [],
                             [Node(btn.name, "label")], [Node(btn.name, btn.role, visible=False)],
                             [Node(btn.name, btn.role, dialog=True)]):
                composer.children = children
                with self.assertRaisesRegex(RuntimeError, "disabled_build_control_not_verified"):
                    lookup(seconds=2)
            composer.children = [btn]
            api.elements = lambda: [outside, workspace, Node(workspace.name, workspace.role)]
            with self.assertRaisesRegex(RuntimeError, "disabled_build_control_not_verified"):
                lookup(seconds=2)
            api.elements = lambda: [outside, workspace]
            workspace.children.append(Node(composer.name, composer.role, [btn]))
            with self.assertRaisesRegex(RuntimeError, "disabled_build_control_not_verified"):
                lookup(seconds=2)
            workspace.children = [deep]
            composer.children = [btn] + [Node("", "label") for _ in range(256)]
            with self.assertRaisesRegex(RuntimeError, "disabled_build_control_not_verified"):
                lookup(seconds=2)

    def test_workflow_review_status_scoped_and_blocked_denied(self):
        from types import SimpleNamespace
        tree = ast.parse(SCRIPTS[0].read_text(encoding="utf-8"))
        fn = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                  and node.name == "native_workflow_review_control")
        class Node:
            def __init__(self, name, role, children=(), visible=True, dialog=False):
                self.name, self.role, self.children = name, role, list(children)
                self.visible, self.dialog = visible, dialog
            @property
            def childCount(self): return len(self.children)
            def getChildAtIndex(self, i): return self.children[i]
            def getRoleName(self): return self.role
            def getState(self): return SimpleNamespace(contains=lambda flag: self.visible)
        class Clock:
            now = 0
            def monotonic(self): return self.now
            def sleep(self, _): self.now += 1
        title="Eligible for human source review"
        correct=Node(title,"status bar")
        outside=Node(title,"status")
        deep=correct
        for _ in range(17): deep=Node("","panel",[deep])
        region=Node("Multi-project native workflow evidence","region",[deep])
        ns={"time":Clock(),"pyatspi":SimpleNamespace(STATE_VISIBLE=1),
            "elements":lambda:[outside,region], "_native_dialog_ancestor":lambda node:node.dialog,
            "deny":lambda reason: (_ for _ in ()).throw(RuntimeError(reason))}
        exec(compile(ast.Module(body=[fn],type_ignores=[]),"review_scope","exec"),ns)
        lookup=ns["native_workflow_review_control"]
        self.assertIs(lookup(title,seconds=2),correct)
        for children in ([], [Node(title,"label")], [correct,Node(title,"status")],
                         [Node(title,"status",visible=False)], [Node(title,"status",dialog=True)],
                         [correct]+[Node("","label") for _ in range(256)]):
            region.children=children
            with self.assertRaisesRegex(RuntimeError,"exact_control_not_verified"):
                lookup(title,seconds=2)
        region.children=[correct,Node("Blocked by native evidence","status")]
        with self.assertRaisesRegex(RuntimeError,"blocked_by_native_evidence"):
            lookup(title,seconds=2)
        region.children=[correct]
        ns["elements"]=lambda:[region,Node(region.name,region.role)]
        with self.assertRaisesRegex(RuntimeError,"exact_control_not_verified"):
            lookup(title,seconds=2)

    def test_b5_undo_requires_fresh_nonempty_actual_source(self):
        from types import SimpleNamespace
        fixture=(ROOT / "b5_linux_installed_perf_atspi.py").read_text(encoding="utf-8")
        tree=ast.parse(fixture)
        fn=next(node for node in tree.body if isinstance(node,ast.FunctionDef) and node.name=="fresh_editor_copy")
        class Clock:
            now=0
            def perf_counter(self): return self.now
            def sleep(self,_): self.now+=1
        events=[]
        value=["B5_NO_FRESH_COPY"]
        def input_run(*args,**kwargs):
            value[0]=kwargs["input"]
        def key(*keys):
            events.append(keys)
            if keys==("ctrl+c",): value[0]="// B5_SOURCE_main native source\n"
        ns={"time":Clock(),"subprocess":SimpleNamespace(run=input_run,DEVNULL=-1),
            "focus_editor":lambda leaf:events.append((leaf,)),"clipboard":lambda:value[0],"key":key,
            "deny":lambda reason: (_ for _ in ()).throw(RuntimeError(reason))}
        exec(compile(ast.Module(body=[fn],type_ignores=[]),"copy_witness","exec"),ns)
        copy=ns["fresh_editor_copy"]
        self.assertIn("B5_SOURCE_main",copy("main.rs",tail=True))
        self.assertEqual(events[0],("main.rs",))
        self.assertEqual(events[-1],("ctrl+c",))
        ns["key"]=lambda *args:None
        self.assertEqual(copy("main.rs"),"")
        self.assertIn('if "B5_SOURCE_main" in observed and PERF_SENTINEL not in observed: break',fixture)
        self.assertIn('"clipboardWitness":"bounded-fresh-head-or-tail-v2"',fixture)

    def test_portfolio_scope_requires_one_real_group_not_legend(self):
        from types import SimpleNamespace
        tree = ast.parse(SCRIPTS[0].read_text(encoding="utf-8"))
        fn = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                  and node.name == "_native_workflow_project_nodes")
        class Node:
            name = "Include registered, open projects"
            def __init__(self, role, children=(), name=None):
                self.role, self.children = role, children
                if name is not None: self.name = name
            def getRoleName(self): return self.role
            def getState(self): return SimpleNamespace(contains=lambda _: True)
        checkbox = Node("check box")
        legend = Node("label")
        root = Node("panel", [checkbox])
        region = Node("region", [legend, root], "Projects in workflow")
        ns = {"elements": lambda: [region],
              "pyatspi": SimpleNamespace(STATE_VISIBLE=1),
              "_native_dialog_ancestor": lambda _: False,
              "children_of": lambda obj: obj.children}
        exec(compile(ast.Module(body=[fn], type_ignores=[]), "portfolio_scope", "exec"), ns)
        self.assertEqual(list(ns["_native_workflow_project_nodes"]()), [root, checkbox])
        ns["elements"] = lambda: [root, Node("grouping")]
        self.assertEqual(list(ns["_native_workflow_project_nodes"]()), [])
        ns["elements"] = lambda: [region, Node("region", [root], "Projects in workflow")]
        self.assertEqual(list(ns["_native_workflow_project_nodes"]()), [])
        ns["elements"] = lambda: [Node("region", [root, Node("grouping")], "Projects in workflow")]
        self.assertEqual(list(ns["_native_workflow_project_nodes"]()), [])

    def test_portfolio_toggle_observes_checked_state_and_never_double_toggles(self):
        from types import SimpleNamespace
        tree = ast.parse(SCRIPTS[0].read_text(encoding="utf-8"))
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
              "_native_workflow_project_nodes": lambda: [checkbox], "_native_dialog_ancestor": lambda _: False,
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
        ns["_native_workflow_project_nodes"] = lambda: [checkbox, Checkbox()]
        with self.assertRaisesRegex(RuntimeError, "exact_checked_state_not_observed"):
            ns["choose_open_workflow_project"]("project", seconds=3)
        self.assertEqual(checkbox.count, 0)
        # A real focused keyboard checkbox gets precisely one Space input,
        # with checked-state readback and no AT-SPI toggle fallback.
        class KeyboardCheckbox(Checkbox):
            def queryComponent(self):
                return SimpleNamespace(grabFocus=lambda: True)
        native = KeyboardCheckbox()
        ns["pyatspi"].STATE_FOCUSABLE = 6
        ns["pyatspi"].STATE_FOCUSED = 7
        ns["_native_workflow_project_nodes"] = lambda: [native]
        keys = []
        def key(value):
            keys.append(value)
            native.checked = native.should_check
        ns["key"] = key
        ns["choose_open_workflow_project"]("project", seconds=3)
        self.assertEqual(keys, ["space"])
        self.assertEqual(native.count, 0)
        native.checked, native.should_check = False, False
        with self.assertRaisesRegex(RuntimeError, "exact_checked_state_not_observed"):
            ns["choose_open_workflow_project"]("project", seconds=3)
        self.assertEqual(keys, ["space", "space"])
        self.assertEqual(native.count, 0)
        for script in SCRIPTS[1:]:
            self.assertIn("ui.choose_open_workflow_project(", script.read_text(encoding="utf-8"))


    def test_default_windows_debugger_gate_matches_current_denial_and_never_enables(self):
        script = (ROOT / "Test-Wo8NativeDesktopUI.ps1").read_text(encoding="utf-8")
        guard = script.split("if ($Scenario -eq 'Dev045ProjectDebuggerDenied')", 1)[1].split(
            "if ($Scenario -eq 'ProtectedDebuggerCancellation')", 1)[0]
        web = (ROOT.parent / "web" / "main.jsx").read_text(encoding="utf-8")
        self.assertIn("protected native release entitlement", web)
        self.assertIn("Require 'protected native release entitlement'", guard)
        self.assertIn("Find-Control 'Debug last verified project build' 'Button'", guard)
        self.assertIn("if ($debug.Current.IsEnabled)", guard)
        self.assertIn("default_unsigned_installer_exposed_project_execution", guard)
        self.assertIn("(ProtectedWorkerCount) -ne $workerBefore", guard)
        self.assertIn("-ne $sourceBefore", guard)
        self.assertNotIn("Click 'Debug last verified project build'", guard)

    def test_windows_native_clipboard_requires_new_os_copy_and_exact_digest(self):
        script = (ROOT / "Test-B2InstalledDraftRecovery.ps1").read_text(encoding="utf-8")
        self.assertIn("Exact installed source editor failed to acquire OS keyboard focus", script)
        self.assertIn("Native clipboard did not observe exact changed editor text", script)
        self.assertIn("[System.Windows.Forms.Clipboard]::Clear()", script)
        self.assertEqual(script.count("draftSha256='+$exactDraftSha+' "), 2)
        self.assertIn("Explicit Save did not restore exact privately preserved draft bytes", script)
        self.assertIn("Installed editor wrote source before explicit Save", script)

    def test_exact_source_file_action_refuses_ambiguity_and_substrings(self):
        from types import SimpleNamespace
        tree = ast.parse(SCRIPTS[0].read_text(encoding="utf-8"))
        fn = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                  and node.name == "activate_exact_source_file")
        class Clock:
            now = 0
            def monotonic(self): return self.now
            def sleep(self, _): self.now += 1
        class Node:
            def __init__(self, name, role="push button", shown=True):
                self.name, self.role, self.shown, self.calls = name, role, shown, 0
            def getRoleName(self): return self.role
            def getState(self): return SimpleNamespace(contains=lambda flag: self.shown)
            def queryAction(self):
                return SimpleNamespace(nActions=1, getName=lambda _: "click", doAction=self.click)
            def click(self, _): self.calls += 1; return True
        exact = Node("main.rs")
        wrong = Node("Close main.rs")
        text = Node("main.rs", "text")
        hidden = Node("main.rs", shown=False)
        ns = {"time": Clock(), "pyatspi": SimpleNamespace(STATE_VISIBLE=1, STATE_SHOWING=2,
              STATE_ENABLED=3, STATE_SENSITIVE=4),
              "_native_source_editor_nodes": lambda: [wrong, text, hidden, exact],
              "deny": lambda reason: (_ for _ in ()).throw(RuntimeError(reason))}
        exec(compile(ast.Module(body=[fn], type_ignores=[]), "exact_source_row", "exec"), ns)
        ns["activate_exact_source_file"]("main.rs", seconds=2)
        self.assertEqual([n.calls for n in (wrong,text,hidden,exact)], [0,0,0,1])
        duplicate = Node("main.rs")
        ns["_native_source_editor_nodes"] = lambda: [exact,duplicate]
        with self.assertRaisesRegex(RuntimeError, "native_exact_source_file_unavailable"):
            ns["activate_exact_source_file"]("main.rs", seconds=2)
        self.assertEqual([exact.calls, duplicate.calls], [1,0])

    def test_native_acceptance_retains_review_and_scoped_editor_identity(self):
        script = (ROOT / "Test-Wo8NativeDesktopUI.ps1").read_text(encoding="utf-8")
        self.assertIn("'Move file','Trash file','Restore file','Toggle split editor','Confirm separate local commit (no push)'", script)
        self.assertIn("$fresh.Current.Name -cne $label", script)
        self.assertIn("$hit.Equals($fresh)", script)
        self.assertIn("$bounds.Contains($point)", script)
        toolbar=script.split("if ($reviewToolbar) {",1)[1].split("$element=Find-Control $label 'Button'",1)[0]
        self.assertNotIn(".Invoke()", toolbar)
        self.assertEqual(toolbar.count("::mouse_event(2,"),1)
        self.assertIn("Find-Control $label 'Button' 6 -Exact", toolbar)
        self.assertIn("Click 'Confirm separate local commit (no push)'", script)
        self.assertIn("'Confirm local Git commit' 'Commit exact staged file' 'WO9 installed reviewed Git source'", script)
        self.assertIn("$expected='Source editor text · First editor: '+$leaf", script)
        linux = (ROOT / "b4_linux_installed_split_atspi.py").read_text(encoding="utf-8")
        self.assertIn("for obj in _native_source_editor_nodes():", linux)
        self.assertIn("pyatspi.STATE_FOCUSED", linux)
        self.assertIn("expected_two_distinct_native_atspi_editor_surfaces_", linux)

if __name__ == "__main__":
    unittest.main()
