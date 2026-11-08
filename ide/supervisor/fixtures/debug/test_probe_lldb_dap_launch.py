"""Fixed LLDB launch diagnostic parser and security boundary tests."""
import unittest

from probe_lldb_dap_launch import (
    BUNDLE, TARGET, BWRAP, MAX_BYTES, classification, frame, launch_request,
    parse,
)


class FixedLaunchDiagnosticTests(unittest.TestCase):
    def test_exact_fixture_and_no_privileged_launch_controls(self):
        req = launch_request("/ls/debuggee/wo6-selftest")
        self.assertEqual(req["type"], "request")
        self.assertEqual(req["command"], "launch")
        self.assertEqual(req["arguments"]["program"], "/ls/debuggee/wo6-selftest")
        self.assertEqual(req["arguments"]["args"], [])
        self.assertEqual(req["arguments"]["env"], [])
        self.assertFalse(req["arguments"]["runInTerminal"])
        self.assertFalse(req["arguments"]["disableASLR"])
        self.assertTrue(req["arguments"]["disableSTDIO"])
        self.assertEqual(TARGET, BUNDLE + "/debuggee/wo6-selftest")
        self.assertIn("--unshare-all", BWRAP)
        self.assertIn("--die-with-parent", BWRAP)
        self.assertIn("--ro-bind", BWRAP)
        self.assertIn("--proc", BWRAP)
        self.assertNotIn("--share-net", BWRAP)
        self.assertNotIn("--cap-add", BWRAP)

    def test_one_complete_frame_consumed_without_echoing_message(self):
        data = bytearray(frame({
            "seq": 7, "type": "response", "request_seq": 2,
            "command": "launch", "success": False,
            "message": "Private sensitive path must never be logged",
        }))
        response, = parse(data)
        self.assertEqual(response["request_seq"], 2)
        self.assertEqual(data, b"")

    def test_partial_frame_preserved_for_next_chunk(self):
        payload = frame({"type": "event", "event": "initialized"})
        data = bytearray(payload[:-4])
        self.assertEqual(parse(data), [])
        data.extend(payload[-4:])
        self.assertEqual(parse(data)[0]["event"], "initialized")
        self.assertEqual(data, b"")

    def test_rejects_non_dap_or_oversized_content(self):
        for raw in (
            b"garbage\r\n\r\n{}",
            b"Content-Length: 99999999\r\n\r\n",
            b"Content-Length: -1\r\n\r\n",
            b"X-H: 1\r\n\r\n{}",
            b"Content-Length: 2\r\n\r\n[]",
        ):
            with self.subTest(raw=raw[:20]), self.assertRaises(ValueError):
                parse(bytearray(raw))
        with self.assertRaises(ValueError):
            parse(bytearray(b"A" * 513))

    def test_classification_fixed_vocabulary_only(self):
        cases = [
            ({"message": "Invalid environment type at /secret"}, "launch_schema_or_arguments"),
            ({"body": {"error": {"format": "ptrace denied /secret"}}},
             "debugger_trace_permission"),
            ({"message": "Operation not permitted /secret"}, "kernel_or_file_permission"),
            ({"message": "failed to launch /secret"}, "debuggee_launch_failed"),
            ({"body": {"error": {"format": "No such file /secret"}}},
             "fixed_binary_or_runtime_missing"),
            ({"message": ""}, "empty_error"),
        ]
        for error, expected in cases:
            with self.subTest(expected=expected):
                self.assertEqual(classification(error), expected)
                self.assertNotIn("/secret", classification(error))
        self.assertEqual(MAX_BYTES, 65536)


if __name__ == "__main__":
    unittest.main()
