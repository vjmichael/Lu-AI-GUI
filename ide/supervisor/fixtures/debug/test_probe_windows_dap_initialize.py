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
