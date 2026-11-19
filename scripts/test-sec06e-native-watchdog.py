#!/usr/bin/env python3
"""Unprivileged notification contract tests, not kernel watchdog acceptance."""
import os
import pathlib
import runpy
import socket
import tempfile
import time
import unittest
from unittest.mock import patch

Watchdog = runpy.run_path(str(pathlib.Path(__file__).with_name(
    'lu-sec06c-owned-kernel-manager.py')))['NativeGuardianWatchdog']


class GuardianWatchdog(unittest.TestCase):
    def test_ready_then_loop_heartbeat(self):
        with tempfile.TemporaryDirectory() as directory, socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as receiver:
            endpoint=directory+'/notify'
            receiver.bind(endpoint)
            receiver.settimeout(1)
            with patch.dict(os.environ, {'NOTIFY_SOCKET':endpoint, 'WATCHDOG_USEC':'20000000', 'WATCHDOG_PID':str(os.getpid())}, clear=True):
                watchdog=Watchdog()
                watchdog.ready()
                self.assertEqual(receiver.recv(128), b'READY=1\nWATCHDOG=1')
                watchdog.last=time.monotonic()-3
                watchdog.tick()
                self.assertEqual(receiver.recv(128), b'WATCHDOG=1')
                receiver.setblocking(False)
                watchdog.tick()
                with self.assertRaises(BlockingIOError): receiver.recv(128)

    def test_bad_watchdog_contract(self):
        valid={'NOTIFY_SOCKET':'/run/systemd/notify','WATCHDOG_USEC':'20000000','WATCHDOG_PID':str(os.getpid())}
        for changes in ({'NOTIFY_SOCKET':''}, {'NOTIFY_SOCKET':'relative'}, {'WATCHDOG_USEC':'1'}, {'WATCHDOG_PID':'0'}):
            with patch.dict(os.environ, dict(valid, **changes), clear=True):
                with self.assertRaises(RuntimeError): Watchdog()

    def test_notification_failure_is_not_a_heartbeat(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict(os.environ, {'NOTIFY_SOCKET':directory+'/missing','WATCHDOG_USEC':'20000000'}, clear=True):
                watchdog=Watchdog()
                with self.assertRaises(OSError): watchdog.ready()
                self.assertIsNone(watchdog.last)


if __name__ == '__main__': unittest.main()
