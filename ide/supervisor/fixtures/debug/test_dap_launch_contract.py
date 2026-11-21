"""Restored fixed-launch and framing regressions against the current probe."""
import json
import unittest

from dap_launch_contract import FIXED_TARGETS, MAX_BUFFER, launch_arguments, pop_frame


def frame(value):
    body = json.dumps(value).encode()
    return b"Content-Length: %d\r\n\r\n" % len(body) + body


class DiagnosticContractTests(unittest.TestCase):
    def test_exact_fixed_launch(self):
        self.assertEqual(len(FIXED_TARGETS), 2)
        for target in FIXED_TARGETS:
            self.assertEqual(launch_arguments(target), {
                "program": target, "stopOnEntry": True, "noDebug": False,
                "disableASLR": False, "disableSTDIO": True,
                "args": [], "env": {}, "runInTerminal": False,
            })

    def test_foreign_targets_denied(self):
        for target in ("/bin/sh", "/ls/debuggee/../bin/lldb-dap", ""):
            with self.subTest(target=target), self.assertRaises(ValueError):
                launch_arguments(target)

    def test_every_partial_boundary(self):
        event = {"type": "event", "event": "initialized"}
        encoded = frame(event)
        for index in range(len(encoded)):
            with self.subTest(index=index):
                data = bytearray(encoded[:index])
                self.assertIsNone(pop_frame(data))
                self.assertEqual(data, encoded[:index])
                data.extend(encoded[index:])
                self.assertEqual(pop_frame(data), event)
                self.assertFalse(data)

    def test_consumes_only_one_frame(self):
        first = {"type": "response", "request_seq": 2, "success": False}
        second = {"type": "event", "event": "stopped"}
        data = bytearray(frame(first) + frame(second))
        self.assertEqual(pop_frame(data), first)
        self.assertEqual(data, frame(second))
        self.assertEqual(pop_frame(data), second)
        self.assertFalse(data)

    def test_malformed_frames_fail_closed(self):
        for raw in (
            b"garbage\r\n\r\n{}", b"Content-Length: 999999\r\n\r\n",
            b"Content-Length: -1\r\n\r\n", b"Content-Length: 0\r\n\r\n",
            b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}",
            b"Content-Length: 2\r\n\r\n[]", b"Content-Length: 1\r\n\r\n{",
            b"X" * 516, b"X" * (MAX_BUFFER + 1),
        ):
            with self.subTest(raw=raw[:20]), self.assertRaisesRegex(ValueError, "^invalid_frame$"):
                pop_frame(bytearray(raw))

    def test_maximum_body_can_be_consumed(self):
        body = b'{"x":"' + b'a' * (32768 - 8) + b'"}'
        self.assertEqual(len(body), 32768)
        data = bytearray(b"Content-Length: 32768\r\n\r\n" + body)
        self.assertEqual(len(pop_frame(data)["x"]), 32760)
        self.assertFalse(data)


if __name__ == "__main__":
    unittest.main()
