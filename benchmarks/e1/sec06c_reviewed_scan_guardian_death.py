#!/usr/bin/env python3
"""Admin-only disposable guest observer. No production permission or receipt.
SIGSTOP the real scanner and SIGKILL its exact root manager via pidfds;
verify independent systemd containment and Lu's durable fail-closed result.
Requires preinstalled immutable first-party broker, scanner, core test binary.
"""
import os
import pathlib
import pwd
import re
import select
import signal
import sqlite3
import subprocess
import tempfile
import time

BASE = pathlib.Path('/sys/fs/cgroup/system.slice')
CORE = '/usr/libexec/lu/core-tests'
SCAN = '/usr/libexec/lu/lu-file-scan-worker-acceptance'
TASK = 'scan-' + 'e'*32


def wait_for(action, seconds):
    deadline = time.monotonic()+seconds
    while time.monotonic() < deadline:
        result = action()
        if result:
            return result
        time.sleep(.01)
    raise RuntimeError('original live custody observation timed out')


def dead(fd):
    p = select.poll(); p.register(fd, select.POLLIN)
    return bool(p.poll(0))


def state(pid):
    return pathlib.Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[0]


def main():
    assert os.geteuid() == 0 and pathlib.Path('/proc/1/comm').read_text().strip() == 'systemd'
    uid = pwd.getpwnam('luprobe').pw_uid
    assert uid > 0
    with tempfile.TemporaryDirectory(prefix='lu-sec06c-manager-death-') as name:
        profile = pathlib.Path(name)
        os.chown(profile, uid, pwd.getpwnam('luprobe').pw_gid)
        proc = subprocess.Popen(['runuser', '-u', 'luprobe', '--', 'env',
            f'LU_SEC06C_VM_PROFILE={profile}', 'LU_SEC06C_BROKER_DEBUG=1', CORE,
            '--ignored', '--nocapture', 'sec06c_vm_reviewed_scan_guardian_death'])
        control = subprocess.Popen(['runuser', '-u', 'luprobe', '--', '/bin/sleep', '120'])
        fds = []
        try:
            ready = profile/'ready'
            def ready_nonce():
                if proc.poll() is not None:
                    raise RuntimeError('Lu fixture exited before manager kill')
                if ready.is_file():
                    value = ready.read_text().strip()
                    return value if re.fullmatch('[a-f0-9]{32}', value) else None
            nonce = wait_for(ready_nonce, 55)
            with sqlite3.connect(profile/'tasks.sqlite') as conn:
                admission = conn.execute('SELECT nonce,chat,turn,kind FROM lu_task_linux_os_owned_native_admission WHERE task_id=?', (TASK,)).fetchone()
            assert admission == (nonce, 'c-'+'1'*32, 'g-'+'2'*32, 'scan-file')
            unit = f'lu-sec06c-native-{nonce}.service'
            group = BASE/unit
            leaf = group/f'lu-sec06c-leaf-{nonce}'
            assert group.stat().st_uid == leaf.stat().st_uid == 0
            mainpid = int(subprocess.check_output(['systemctl', 'show', unit, '-p', 'MainPID', '--value'], text=True))
            assert mainpid > 1 and mainpid in [int(x) for x in (group/'cgroup.procs').read_text().split()]
            assert pathlib.Path(f'/proc/{mainpid}').stat().st_uid == 0
            members = [int(x) for x in (leaf/'cgroup.procs').read_text().split()]
            assert len(members) == 2
            worker = next(pid for pid in members if os.readlink(f'/proc/{pid}/exe') == SCAN)
            for pid in [mainpid, *members]:
                assert pid > 1
                fds.append(os.pidfd_open(pid))
            assert not any(dead(fd) for fd in fds)
            # Freeze only the pinned scanner to prevent a natural-finish race;
            # thus the observed failure is actually in a running worker tree.
            workerfd = fds[1+members.index(worker)]
            signal.pidfd_send_signal(workerfd, signal.SIGSTOP)
            wait_for(lambda: state(worker) in ('T', 't'), 3)
            assert not dead(fds[0])
            signal.pidfd_send_signal(fds[0], signal.SIGKILL)
            wait_for(lambda: dead(fds[0]), 10)
            wait_for(lambda: all(dead(fd) for fd in fds), 25)
            wait_for(lambda: not group.exists() or (group/'cgroup.events').read_text().splitlines()[0] == 'populated 0', 20)
            assert control.poll() is None
            assert proc.wait(timeout=80) == 0
            receipt = pathlib.Path('/var/lib/lu-sec06c/native-receipts')/(nonce+'.receipt')
            assert not receipt.exists(), 'dead manager forged a completion receipt'
            with sqlite3.connect(profile/'tasks.sqlite') as conn:
                assert conn.execute('SELECT state FROM lu_tasks WHERE task_id=?',(TASK,)).fetchone() == ('INTERRUPTED',)
                assert conn.execute('SELECT reason FROM lu_task_custody_uncertainty WHERE task_id=?',(TASK,)).fetchone() == ('OS_CUSTODY_UNVERIFIED',)
            with sqlite3.connect(profile/'schedule.sqlite') as conn:
                assert conn.execute('SELECT state FROM lu_schedule WHERE task_id=?',(TASK,)).fetchone() == ('NEEDS_REVIEW',)
            print('SEC06C_VM_REAL_MANAGER_SIGKILL_PASS root_pidfd=1 worker_stalled=1 all_descendants_dead=1 unrelated_survived=1 no_receipt=1 durably_interrupted=1 no_replay=1',flush=True)
        finally:
            for fd in fds: os.close(fd)
            for child in (proc, control):
                if child.poll() is None: child.terminate()
                child.wait(timeout=15)


if __name__ == '__main__':
    main()
