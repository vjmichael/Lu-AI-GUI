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

if __name__ == "__main__":
    unittest.main()
