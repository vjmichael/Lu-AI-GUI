#!/usr/bin/env python3
"""P03 bounded fixed Windows LLDB-DAP loader witness unit tests; no process spawn."""
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import probe_windows_dap_initialize as probe


def framed(obj):
    body = json.dumps(obj, separators=(",", ":")).encode()
    return b"Content-Length: %d\r\n\r\n" % len(body) + body


class DapPreflightBoundaries(unittest.TestCase):
    def test_native_loader_codes_are_category_only(self):
        self.assertEqual(probe.classify_exit(3221225781), "dependency_dll_missing")
        self.assertEqual(probe.classify_exit(-1073741515), "dependency_dll_missing")
        self.assertEqual(probe.classify_exit(0xC0000022), "kernel_access_denied")
        self.assertEqual(probe.classify_exit(0xC0000142), "dll_initialization_failed")
        self.assertEqual(probe.classify_exit(0xC000007B), "invalid_binary_format")
        self.assertEqual(probe.classify_exit(None), "timeout_or_unreaped")

    def test_stderr_diagnostic_is_fixed_class_only(self):
        self.assertEqual(probe.classify_stderr(b""), "empty")
        self.assertEqual(probe.classify_stderr(
            b"Fatal Python error: init_fs_encoding: No module named encodings"),
            "python_runtime_or_module_path")
        self.assertEqual(probe.classify_stderr(
            b"C:\\private\\sensitive.txt: Permission denied"),
            "permission_denied_unspecified_object")
        self.assertEqual(probe.classify_stderr(
            b"WriteFile pipe handle was invalid"), "native_stdio_or_handle")
        self.assertEqual(probe.classify_stderr(b"irrelevant user content"),
                         "unclassified_nonempty")

    def test_fixed_private_environment_is_closed_and_matches_native_runner(self):
        local = Path("D:/ci/private")
        tools = Path("D:/ci/reviewed/bin")
        system = Path("C:/Windows")
        env = probe.fixed_private_environment(local, tools, system)
        self.assertEqual(set(env), {
            "APPDATA", "HOME", "LOCALAPPDATA", "PATH", "SystemRoot",
            "TEMP", "TMP", "USERPROFILE"})
        for name in ("APPDATA", "HOME", "LOCALAPPDATA", "TEMP",
                     "TMP", "USERPROFILE"):
            self.assertEqual(env[name], str(local))
        self.assertEqual(env["PATH"], str(tools) + ";" + str(system / "System32"))
        self.assertNotIn("PYTHONPATH", env)
        self.assertNotIn("LLDBDAP_LOG", env)

    def test_only_exact_successful_initialize_response_counts(self):
        valid = {"seq": 2, "type": "response", "request_seq": 1,
                 "command": "initialize", "success": True}
        self.assertTrue(probe.parse_initialize_response(framed(valid)))
        self.assertFalse(probe.parse_initialize_response(
            framed({"seq": 1, "type": "event", "event": "initialized"}) + framed(valid)))
        for invalid in [
            {"seq": 1, "type": "request", "command": "runInTerminal"},
            {"seq": 3, "type": "response", "request_seq": 9,
             "command": "initialize", "success": True},
            {"seq": 4, "type": "response", "request_seq": 1,
             "command": "launch", "success": True},
            dict(valid, success=False),
        ]:
            self.assertFalse(probe.parse_initialize_response(framed(invalid)))
        self.assertFalse(probe.parse_initialize_response(b"Content-Length: 99999\r\n\r\n"))
        self.assertFalse(probe.parse_initialize_response(b"not a frame"))
        self.assertFalse(probe.parse_initialize_response(b"x" * (probe.MAX_BYTES + 1)))


if __name__ == "__main__":
    unittest.main()
