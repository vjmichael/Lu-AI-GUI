#!/usr/bin/env python3
"""Unprivileged framing regression for the production root manager."""
import pathlib
import runpy
import unittest

prefix = runpy.run_path(str(pathlib.Path(__file__).with_name(
    'lu-sec06c-owned-kernel-manager.py')))['reviewed_scan_progress_prefix']


class ReviewedScanProgress(unittest.TestCase):
    def test_complete_prefix_only(self):
        p = b'P\t0\t100\nP\t1\t100\n'
        digest = b'D\t' + b'a' * 64 + b'\n'
        self.assertEqual(prefix(p + digest, 8192), p)
        self.assertEqual(prefix(digest + p, 8192), b'')

    def test_split_frames(self):
        p = b'P\t1\t100\n'
        for i in range(len(p)):
            self.assertEqual(prefix(p[:i], 8192), b'')
        self.assertEqual(prefix(p + p[:-1], 8192), p)

    def test_budget(self):
        p = b'P\t1\t100\n'
        self.assertEqual(prefix(p + p, len(p)), p)
        self.assertEqual(prefix(p, len(p) - 1), b'')
        self.assertEqual(prefix(p, 0), b'')

    def test_invalid_and_oversized(self):
        for raw in (b'P\t01\t100\n', b'P\t1\t0\n', b'P\t-1\t100\n',
                    b'P\t1\t100\tsecret\n', b'P\t' + b'9' * 129 + b'\n',
                    b'P\t1\t100\r\n', b'SECRET\n'):
            self.assertEqual(prefix(raw, 8192), b'')

    def test_no_skipping_nonprogress(self):
        p = b'P\t1\t100\n'
        self.assertEqual(prefix(p + b'X\n' + p, 8192), p)


if __name__ == '__main__':
    unittest.main()
