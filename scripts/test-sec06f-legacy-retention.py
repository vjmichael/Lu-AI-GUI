#!/usr/bin/python3
"""Filesystem contract tests; no simulated kernel acceptance."""
import os
import pathlib
import runpy
import tempfile
import unittest
from unittest.mock import patch

source=pathlib.Path(__file__).with_name('lu-sec06c-owned-kernel-manager.py')
K=runpy.run_path(str(source))
retain=K['preserve_legacy_marker']; fence=K['require_unspent_legacy_nonce']; sweep=K['retain_legacy_markers']

class LegacyRetention(unittest.TestCase):
    def setUp(self):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup)
        self.root=pathlib.Path(temp.name);self.shared=self.root/'run'/'lu-sec06c';self.denials=self.root/'var'/'lib'/'lu-sec06c'/'legacy-denials'
        self.shared.mkdir(parents=True);self.denials.mkdir(parents=True)
        self.nonce='a'*32;self.marker=self.shared/(self.nonce+'.go');self.denial=self.denials/(self.nonce+'.denied')
        # Unprivileged filesystem contract: actual ownership checks are separately required by root guest.
        mock=patch.dict(retain.__globals__,{'APPROVAL':self.shared,'LEGACY_DENIALS':self.denials,'checked_root_dir':lambda p:p.mkdir(parents=True,exist_ok=True)})
        mock.start();self.addCleanup(mock.stop)

    def test_fresh_nonce_neither_reserved_nor_denied(self):
        self.assertFalse(retain(self.nonce));fence(self.nonce);self.assertFalse(self.denial.exists())

    def test_ambiguous_marker_never_removed_and_replay_stays_denied_after_volatile_loss(self):
        self.marker.write_bytes(b'old unknown marker');before=self.marker.read_bytes()
        self.assertTrue(retain(self.nonce));self.assertEqual(self.marker.read_bytes(),before)
        preserved=self.denial.read_bytes();self.assertIn(b'LEGACY_DENIAL',preserved);self.assertNotIn(b'KERNEL_EMPTY',preserved)
        self.marker.unlink() # fixture simulates OS removal of /run, never production cleanup
        with self.assertRaisesRegex(RuntimeError,'denial only'):fence(self.nonce)
        self.assertEqual(self.denial.read_bytes(),preserved)

    def test_dangling_marker_and_partial_denial_fail_closed(self):
        self.marker.symlink_to(self.root/'missing')
        with self.assertRaises(RuntimeError):fence(self.nonce)
        self.assertTrue(self.marker.is_symlink());self.denial.write_bytes(b'')
        with self.assertRaises(RuntimeError):fence(self.nonce)
        self.assertEqual(self.denial.read_bytes(),b'')

    def test_untrusted_denial_entry_never_followed(self):
        target=self.root/'untouched';target.write_bytes(b'untouched');self.denial.symlink_to(target)
        with self.assertRaises(RuntimeError):fence(self.nonce)
        self.assertEqual(target.read_bytes(),b'untouched')

    def test_invalid_nonce_never_selects_files(self):
        with self.assertRaises(RuntimeError):retain('../invalid')
        self.assertEqual(list(self.denials.iterdir()),[])

    @unittest.skipUnless(os.geteuid()==0,'requires root for maintenance sweep')
    def test_non_destructive_sweep_and_idempotence(self):
        self.marker.write_bytes(b'unknown');other=self.shared/'unrelated';other.write_bytes(b'keep')
        self.assertEqual(sweep(),1);before=self.denial.read_bytes();self.assertEqual(sweep(),1)
        self.assertEqual(self.marker.read_bytes(),b'unknown');self.assertEqual(other.read_bytes(),b'keep');self.assertEqual(self.denial.read_bytes(),before)

if __name__=='__main__':unittest.main()
