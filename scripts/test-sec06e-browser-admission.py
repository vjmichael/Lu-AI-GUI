#!/usr/bin/env python3
"""Admission-record filesystem tests only. Not browser or kernel acceptance."""
import pathlib
import runpy
import tempfile
import types
import unittest
from unittest.mock import patch

reserve=runpy.run_path(str(pathlib.Path(__file__).with_name(
    'lu-sec06c-owned-browser-launcher.py')))['consume_original_nonce']

class BrowserNonce(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path=pathlib.Path(self.temp.name)/'root'/'receipts'
        self.path.mkdir(parents=True)
        self.opts=types.SimpleNamespace(nonce='1'*32,chat='c-'+'2'*32,turn='g-'+'3'*32,call='browser-task-'+'4'*32,
            chromium_sha='5'*64,node_sha='6'*64,script_sha='7'*64,policy_sha='8'*64)
        self.record=self.path/(self.opts.nonce+'.spent')
        self.patch=patch.dict(reserve.__globals__,{'RECEIPTS':self.path})
        self.patch.start();self.addCleanup(self.patch.stop)

    def test_exact_and_exclusive(self):
        reserve(self.opts,1000)
        before=self.record.read_bytes()
        self.assertEqual(before.decode(), '\n'.join(['LU_SEC06E_BROWSER_SPENT_V1','NONCE='+self.opts.nonce,
            'CHAT='+self.opts.chat,'TURN='+self.opts.turn,'CALL='+self.opts.call,
            'CHROMIUM_SHA256='+self.opts.chromium_sha,'NODE_SHA256='+self.opts.node_sha,
            'SCRIPT_SHA256='+self.opts.script_sha,'POLICY_SHA256='+self.opts.policy_sha,'ORIGINAL_UID=1000','']))
        with self.assertRaisesRegex(RuntimeError,'already consumed'):reserve(self.opts,1000)
        self.assertEqual(self.record.read_bytes(),before)
        self.assertFalse(self.record.with_suffix('.receipt').exists())

    def test_failed_partial_record_remains_denial(self):
        self.record.write_bytes(b'')
        with self.assertRaisesRegex(RuntimeError,'already consumed'):reserve(self.opts,1000)
        self.assertEqual(self.record.read_bytes(),b'')

    def test_symlink_never_followed(self):
        target=self.path/'unrelated';target.write_bytes(b'unrelated')
        self.record.symlink_to(target)
        with self.assertRaisesRegex(RuntimeError,'already consumed'):reserve(self.opts,1000)
        self.assertEqual(target.read_bytes(),b'unrelated')

if __name__=='__main__':unittest.main()
