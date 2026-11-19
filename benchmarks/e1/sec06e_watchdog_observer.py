#!/usr/bin/env python3
"""Admin-only disposable guest observer. No production permission or receipt.
SIGSTOP the real scanner and its exact root manager via pidfds;
Only the independent systemd watchdog may terminate the stalled service.
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
    with tempfile.TemporaryDirectory(prefix='lu-sec06e-watchdog-') as name:
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
                    raise RuntimeError('Lu fixture exited before manager stall')
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
            props=subprocess.check_output(['systemctl','show',unit,'-p','Type','-p','NotifyAccess','-p','WatchdogUSec','-p','WatchdogSignal','-p','WatchdogTimestampMonotonic','-p','KillMode'],text=True)
            assert all(x in props.splitlines() for x in ('Type=notify','NotifyAccess=main','WatchdogUSec=20s','WatchdogSignal=9','KillMode=control-group')), props
            stamps=[x.split('=',1)[1] for x in props.splitlines() if x.startswith('WatchdogTimestampMonotonic=')]
            assert len(stamps)==1 and int(stamps[0])>0
            # A real worker stall must not be confused with a stalled manager.
            # Hold the worker longer than the deadline while the original
            # manager continues its own loop; verify PID1 received new heartbeats.
            time.sleep(22)
            assert not any(dead(fd) for fd in fds)
            latest=int(subprocess.check_output(['systemctl','show',unit,'-p','WatchdogTimestampMonotonic','--value'],text=True).strip())
            assert latest>int(stamps[0])
            print('SEC06E_HEALTHY_GUARDIAN_OVER_DEADLINE_PASS',flush=True)
            signal.pidfd_send_signal(fds[0], signal.SIGSTOP)
            wait_for(lambda: state(mainpid) in ('T','t'), 3)
            started=time.monotonic()
            assert not any(dead(fd) for fd in fds)
            assert (leaf/'cgroup.events').read_text().startswith('populated 1\n')
            wait_for(lambda: dead(fds[0]), 30)
            elapsed=time.monotonic()-started
            assert 0<elapsed<=30
            print('Original service watchdog terminated stalled guardian', flush=True)
            wait_for(lambda: all(dead(fd) for fd in fds), 25)
            wait_for(lambda: not group.exists() or (group/'cgroup.events').read_text().splitlines()[0] == 'populated 0', 20)
            # PID1 journal corroborates the watchdog cause, not simply death.
            def watchdog_failure():
                log=subprocess.check_output(['journalctl','-u',unit,'--no-pager','-o','cat'],text=True)
                return "Failed with result 'watchdog'" in log
            wait_for(watchdog_failure,10)
            assert control.poll() is None
            assert proc.wait(timeout=80) == 0
            receipt = pathlib.Path('/var/lib/lu-sec06c/native-receipts')/(nonce+'.receipt')
            assert not receipt.exists(), 'dead manager forged a completion receipt'
            with sqlite3.connect(profile/'tasks.sqlite') as conn:
                assert conn.execute('SELECT state FROM lu_tasks WHERE task_id=?',(TASK,)).fetchone() == ('INTERRUPTED',)
                assert conn.execute('SELECT reason FROM lu_task_custody_uncertainty WHERE task_id=?',(TASK,)).fetchone() == ('OS_CUSTODY_UNVERIFIED',)
            with sqlite3.connect(profile/'schedule.sqlite') as conn:
                assert conn.execute('SELECT state FROM lu_schedule WHERE task_id=?',(TASK,)).fetchone() == ('NEEDS_REVIEW',)
            print('SEC06E_VM_SYSTEMD_WATCHDOG_PASS guardian_stalled=1 independent_watchdog=1 worker_stalled=1 all_descendants_dead=1 unrelated_survived=1 no_receipt=1 durably_interrupted=1 no_replay=1',flush=True)
        finally:
            for fd in fds: os.close(fd)
            for child in (proc, control):
                if child.poll() is None: child.terminate()
                child.wait(timeout=15)


if __name__ == '__main__':
    main()
