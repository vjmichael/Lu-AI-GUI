#!/usr/bin/env python3
"""SEC-06C root-owned *candidate* outer kernel boundary, test-only.

A genuine systemd system service owns the operation cgroup. The first-party
probe and setsid descendant run as the existing unprivileged account. An
independent, unprivileged verifier writes to the actual ancestor cgroup.procs
and MUST be denied by the kernel. On real parent SIGKILL, systemd's
KillMode=control-group must reap the original descendant.

This is a boundary *prototype*, NOT Lu production worker integration or
SEC-06C release acceptance. Never treat it as a substitute for original
lu-linux-cgroup-guard/SEC-05 adversarial acceptance. No root worker tests,
fake mounts, or model-selected commands.
"""
import argparse
import json
import os
import pathlib
import pwd
import secrets
import select
import signal
import subprocess
import sys
import time

REPO = pathlib.Path(__file__).resolve().parents[2]
MOUNT = pathlib.Path("/sys/fs/cgroup")
FORMAT = "lu-sec06c-os-owned-boundary-native-v1"

def fail(message):
    raise RuntimeError("SEC06C_BOUNDARY_BLOCKED: " + message)

def head(expected):
    actual = subprocess.check_output(
        ["git", "-C", str(REPO), "rev-parse", "HEAD"], text=True, timeout=30,
    ).strip()
    if actual != expected:
        fail("source identity changed")

def live(pid):
    try:
        state = pathlib.Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[0]
        return state not in ("Z", "X", "x")
    except FileNotFoundError:
        return False

def proc_group(pid):
    rows = pathlib.Path(f"/proc/{pid}/cgroup").read_text().splitlines()
    if len(rows) != 1 or not rows[0].startswith("0::/"):
        fail("missing single native cgroup-v2 identity")
    return rows[0][3:]

def atomic_observation(path, data):
    with path.open("x", encoding="utf8") as file:
        json.dump(data, file, sort_keys=True, indent=2)
        file.write("\n")
        file.flush()
        os.fsync(file.fileno())

def inside(unit, evidence, expected):
    head(expected)
    if sys.platform != "linux" or os.geteuid() == 0:
        fail("worker must be nonroot native Linux")
    if proc_group(os.getpid()).rsplit("/", 1)[-1] != unit:
        fail("not running within the exact OS-owned unit")
    subtree = (MOUNT / proc_group(os.getpid()).lstrip("/")).resolve(strict=True)
    if subtree == MOUNT or not subtree.is_relative_to(MOUNT):
        fail("untrusted unit root")
    if subtree.stat().st_uid == os.geteuid():
        fail("worker owns its OS operation boundary")
    procs = subtree / "cgroup.procs"
    if procs.stat().st_uid == os.geteuid():
        fail("worker owns ancestor process migration")
    worker = subprocess.Popen(
        ["/bin/sleep", "120"], start_new_session=True,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    try:
        if not live(worker.pid) or proc_group(worker.pid) != proc_group(os.getpid()):
            fail("setsid descendant did not remain in exact OS unit")
        atomic_observation(evidence / "worker-identity.json", {
            "schema": FORMAT, "head": expected, "unit": unit,
            "mainPid": os.getpid(), "descendantPid": worker.pid,
            "workerEuid": os.geteuid(),
            "originalGroup": proc_group(os.getpid()),
            "allProcessesWithinOSOwnedUnit": True,
            "releaseOrInstallAuthorized": False,
        })
        # Mirror the real Lu-owned service worker's crash/cancel lifetime
        # pipe. An explicitly open stdin keeps it alive; a killed host
        # closes that pipe and must trigger own-child reaping.
        while True:
            readable, _, _ = select.select([sys.stdin.buffer], [], [], .1)
            if readable and not sys.stdin.buffer.read(1):
                break
    finally:
        # The independent test kills the main PID by exact pidfd, bypassing
        # Python cleanup; normal early errors still reap the disposable child.
        if live(worker.pid):
            worker.kill()
        worker.wait(timeout=5)

def pidfd_terminated(fd, timeout):
    waiter = select.poll()
    waiter.register(fd, select.POLLIN)
    return bool(waiter.poll(round(timeout * 1000)))

def outer(unit, evidence, expected, scenario):
    head(expected)
    if sys.platform != "linux" or os.geteuid() == 0:
        fail("verifier must be nonroot native Linux")
    service = pathlib.Path("/usr/bin/systemd-run")
    sudo = pathlib.Path("/usr/bin/sudo")
    if not service.is_file() or not sudo.is_file():
        fail("real systemd service manager unavailable")
    who = pwd.getpwuid(os.geteuid()).pw_name
    python = str(pathlib.Path(sys.executable).resolve(strict=True))
    argv = [
        str(sudo), "-n", str(service), "--system", "--quiet", "--wait",
        "--pipe", "--collect", "--service-type=exec", f"--unit={unit}",
        f"--property=User={who}", "--property=Delegate=no",
        "--property=KillMode=control-group", "--property=Restart=no",
        f"--working-directory={REPO}",
        python, str(pathlib.Path(__file__).resolve()), "--inside",
        "--expected-sha", expected, "--evidence", str(evidence), "--unit", unit,
    ]
    os.makedirs(evidence, mode=0o700, exist_ok=True)
    service_process = subprocess.Popen(
        argv, cwd=REPO, stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    mainfd = childfd = None
    worker_pid = child_pid = None
    try:
        identity_file = evidence / "worker-identity.json"
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if identity_file.is_file():
                break
            if service_process.poll() is not None:
                fail("OS-owned unit exited before original worker identity")
            time.sleep(.03)
        if not identity_file.is_file():
            fail("OS-owned unit never published original identity")
        identity = json.loads(identity_file.read_text())
        if identity.get("schema") != FORMAT or identity.get("head") != expected:
            fail("wrong original identity or source")
        if identity.get("unit") != unit or identity.get("workerEuid") != os.geteuid():
            fail("OS unit/worker euid mismatch")
        worker_pid, child_pid = identity["mainPid"], identity["descendantPid"]
        if worker_pid <= 1 or child_pid <= 1 or worker_pid == child_pid:
            fail("unsafe original PID identities")
        mainfd, childfd = os.pidfd_open(worker_pid), os.pidfd_open(child_pid)
        original = proc_group(worker_pid)
        if original != identity["originalGroup"] or proc_group(child_pid) != original:
            fail("OS process membership changed")
        unit_path = (MOUNT / original.lstrip("/")).resolve(strict=True)
        parent = unit_path.parent
        if unit_path.name != unit or unit_path.stat().st_uid == os.geteuid():
            fail("non-root process has unit ownership or wrong unit")
        if not (unit_path / "cgroup.kill").is_file():
            fail("missing authentic kernel kill interface")
        # Reproduce the *same external attacker* used in original SEC-05:
        # direct nonroot write of the pinned setsid descendant's PID into
        # the ancestor's cgroup.procs. An acknowledged migration is a
        # security FAIL, even if later cleanup kills the process.
        migrated = False
        try:
            (parent / "cgroup.procs").write_text(str(child_pid), encoding="ascii")
        except PermissionError:
            pass
        else:
            migrated = True
        if migrated or proc_group(child_pid) != original:
            fail("SEC06C_ANCESTOR_ESCAPE_SUCCEEDED")
        if scenario == "cancel":
            # The root-owned system manager, not an attacker-chosen PID,
            # stops this exact disposable unit and must reap setsid children.
            cancelled = subprocess.run(
                [str(sudo), "-n", "/usr/bin/systemctl", "stop", unit],
                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
                timeout=20,
            )
            if cancelled.returncode != 0:
                fail("native OS-managed cancellation rejected")
        elif scenario == "host-crash":
            # The original nonroot launcher client dies outside the unit.
            # A surviving worker is a blocker; don't substitute a successful
            # stop-by-verifier after the fact for actual automatic recovery.
            launcher_fd = os.pidfd_open(service_process.pid)
            try:
                # The Lu host's private original stdin is now gone. The
                # service worker must notice EOF and reap its child; a
                # surviving service after launcher death is not accepted.
                service_process.stdin.close()
                service_process.stdin = None
                signal.pidfd_send_signal(launcher_fd, signal.SIGKILL)
                if not pidfd_terminated(launcher_fd, 15):
                    fail("original host launcher survived SIGKILL")
            finally:
                os.close(launcher_fd)
        else:
            signal.pidfd_send_signal(mainfd, signal.SIGKILL)
            if not pidfd_terminated(mainfd, 15):
                fail("SIGKILL did not terminate original service main")
        if not pidfd_terminated(childfd, 15):
            fail("setsid descendant survived OS-managed "+scenario+" recovery")
        if live(child_pid):
            fail("original escaped descendant still runnable after manager cleanup")
        atomic_observation(evidence / "kernel-boundary-proof.json", {
            "schema": FORMAT, "head": expected, "unit": unit,
            "originalGroup": original,
            "nonrootEuid": os.geteuid(),
            "rootOwnedUnit": True, "ancestorMigrationDenied": True,
            "exactOriginalPidfds": True, "nativeMainSigkill": True,
            "setsidDescendantReaped": True,
            "recoveryScenario": scenario,
            "systemdKillMode": "control-group",
            "noReleaseOrInstallAuthority": True,
        })
        print("SEC06C_ROOT_OWNED_UNIT_KERNEL_PROTOTYPE_PASS=1 scenario="+scenario, flush=True)
        return 0
    finally:
        if childfd is not None:
            if not pidfd_terminated(childfd, 0):
                try:
                    signal.pidfd_send_signal(childfd, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            os.close(childfd)
        if mainfd is not None:
            if not pidfd_terminated(mainfd, 0):
                try:
                    signal.pidfd_send_signal(mainfd, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            os.close(mainfd)
        if service_process.poll() is None:
            # A failed test must not leave a root-owned unit running. Only
            # this fixed, randomly generated test unit may be stopped.
            subprocess.run([str(sudo), "-n", "/usr/bin/systemctl",
                            "stop", unit], check=False, timeout=20)
        try:
            service_process.communicate(timeout=20)
        except subprocess.TimeoutExpired:
            service_process.kill()
            service_process.communicate(timeout=5)

def main():
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--inside", action="store_true")
    mode.add_argument("--outer", action="store_true")
    parser.add_argument("--expected-sha", required=True)
    parser.add_argument("--evidence", required=True)
    parser.add_argument("--unit")
    parser.add_argument("--scenario", choices=["worker-main-sigkill",
                                                "cancel", "host-crash"],
                        default="worker-main-sigkill")
    args = parser.parse_args()
    if len(args.expected_sha) != 40 or any(c not in "0123456789abcdef" for c in args.expected_sha):
        parser.error("invalid source SHA")
    path = pathlib.Path(args.evidence).resolve()
    if not path.is_relative_to(REPO) or path == REPO:
        parser.error("original evidence directory must remain inside checked-out source")
    unit = args.unit if args.inside else "lu-sec06c-boundary-" + secrets.token_hex(7) + ".service"
    if args.inside and (not unit.startswith("lu-sec06c-boundary-") or not unit.endswith(".service")):
        parser.error("unsafe systemd unit identity")
    try:
        return inside(unit, path, args.expected_sha) if args.inside else outer(unit, path, args.expected_sha, args.scenario)
    except Exception as exc:
        print(f"SEC06C_ROOT_OWNED_BOUNDARY_BLOCKED: {exc}", file=sys.stderr)
        return 1

if __name__ == "__main__":
    sys.exit(main())
