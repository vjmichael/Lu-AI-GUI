#!/usr/bin/env python3
"""SEC-06C genuine systemd-delegated Linux cgroup-v2 admission.

Outer launcher: systemd PID 1 creates a Delegate=yes service running as the
original NON-ROOT account. Only the service manager uses privilege; the
first-party Lu worker/guardian, verifier and original Git journal do not.
Unavailable delegation exits 77, an observed ancestor escape exits failure.
No model-selected commands, PIDs or cgroup paths are accepted.
"""
import argparse
import json
import os
import pathlib
import pwd
import secrets
import shutil
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
CGROUP_MOUNT = pathlib.Path("/sys/fs/cgroup")
NAME = "kernel-delegation.json"


class Unavailable(Exception):
    pass


def emit(evidence, status, expected, **extra):
    evidence.mkdir(parents=True, exist_ok=True)
    data = {
        "schema": "lu-sec06c-native-delegation-v1",
        "status": status,
        "head": expected,
        "euid": os.geteuid(),
        "kernelTestAccepted": False,
        "releaseOrInstallAuthorized": False,
        **extra,
    }
    target = evidence / NAME
    # The original artifact is preserved, not replaced by a later attempt.
    with target.open("x", encoding="utf-8") as handle:
        json.dump(data, handle, sort_keys=True, indent=2)
        handle.write("\n")
    print(f"SEC06C_DELEGATION_STATUS={status}", flush=True)
    return data


def real_head(expected):
    actual = subprocess.check_output(
        ["git", "-C", str(REPO), "rev-parse", "HEAD"],
        text=True, timeout=30,
    ).strip()
    if actual != expected or len(actual) != 40:
        raise RuntimeError("SEC06C_EXACT_SOURCE_SHA_MISMATCH")


def observed_kernel_delegation():
    if sys.platform != "linux" or os.geteuid() == 0:
        raise Unavailable("must execute nonroot on native Linux")
    own = pathlib.Path("/proc/self/cgroup").read_text(encoding="ascii").splitlines()
    members = [row[3:] for row in own if row.startswith("0::")]
    if len(own) != 1 or len(members) != 1 or not members[0].startswith("/"):
        raise Unavailable("not one unified native cgroup v2")
    if members[0] == "/":
        raise Unavailable("global cgroup root is not a delegated service")
    mount_lines = pathlib.Path("/proc/self/mountinfo").read_text().splitlines()
    mount = [
        row for row in mount_lines
        if " - cgroup2 " in row and row.split(" - ", 1)[0].split()[4] == "/sys/fs/cgroup"
    ]
    if len(mount) != 1 or "rw" not in mount[0].split(" - ", 1)[0].split()[5].split(","):
        raise Unavailable("cgroup v2 mount missing or read-only")
    root = CGROUP_MOUNT.resolve(strict=True)
    path = (root / members[0].lstrip("/")).resolve(strict=True)
    if not path.is_relative_to(root) or path == root or not path.is_dir():
        raise Unavailable("untrusted or missing delegated cgroup path")
    if path.stat().st_uid != os.geteuid():
        raise Unavailable("delegated cgroup not owned by test user")
    try:
        tag = os.getxattr(path, "user.delegate")
    except OSError as exc:
        raise Unavailable("no systemd user.delegate boundary") from exc
    if tag != b"1":
        raise Unavailable("systemd did not confirm Delegate=yes")
    if not (path / "cgroup.procs").is_file():
        raise Unavailable("missing authentic kernel cgroup.procs")
    if not (path / "cgroup.controllers").is_file():
        raise Unavailable("missing authentic cgroup controllers")
    child = path / ("lu-sec06c-probe-" + secrets.token_hex(12))
    try:
        child.mkdir()
        if not (child / "cgroup.kill").is_file() or not (child / "cgroup.procs").is_file():
            raise Unavailable("delegated child lacks native cgroup.kill/procs")
        if "populated 0" not in (child / "cgroup.events").read_text().splitlines():
            raise Unavailable("probe child is unexpectedly populated")
    except (PermissionError, OSError) as exc:
        raise Unavailable("delegated cgroup mkdir/kernel probe failed") from exc
    finally:
        if child.is_dir():
            child.rmdir()
    return {
        "originalCgroup": members[0],
        "realDelegation": True,
        "systemdDelegateXattr": True,
        "kernelCgroupKillPresent": True,
        "probePopulatedZero": True,
        "ancestorCgroupProcsUserWritable": os.access(path / "cgroup.procs", os.W_OK),
        "kernel": os.uname().release,
    }


def inside(expected, evidence):
    real_head(expected)
    try:
        kernel = observed_kernel_delegation()
    except Unavailable as exc:
        emit(evidence, "NOT_AVAILABLE", expected, reason=str(exc))
        return 77
    emit(evidence, "DELEGATED_KERNEL_PREFLIGHT_ONLY", expected, **kernel)
    # Production control remains the unchanged independent SEC-06 coordinator.
    # Its actual ancestor, guardian and original Git receipts decide admission.
    node = shutil.which("node")
    if not node:
        print("SEC06C_NOT_AVAILABLE: no native Node", file=sys.stderr)
        return 77
    status = subprocess.run(
        [node, "scripts/p0-sec06-consolidated.mjs", "--mode", "linux",
         "--evidence", str(evidence)],
        cwd=REPO, check=False,
    ).returncode
    receipt = evidence / "linux.receipt.json"
    if not receipt.is_file():
        print("SEC06C_BLOCKED: missing native original Linux receipt", file=sys.stderr)
        return 1
    try:
        proof = json.loads(receipt.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return 1
    if (status != 0 or proof.get("head") != expected
            or proof.get("allNativeCasesPassed") is not True
            or proof.get("status") != "PROVISIONAL_NATIVE_PASS_OWNER_GATE_PENDING"):
        print("SEC06C_BLOCKED: native case or ancestor containment failed", file=sys.stderr)
        return 1
    print("SEC06C_NATIVE_PROVISIONAL_PASS_INDEPENDENT_REVIEW_PENDING=1", flush=True)
    return 0


def outer(expected, evidence):
    real_head(expected)
    if sys.platform != "linux" or os.geteuid() == 0:
        emit(evidence, "NOT_AVAILABLE", expected, reason="outer launcher must be nonroot native Linux")
        return 77
    systemd = pathlib.Path("/usr/bin/systemd-run")
    sudo = pathlib.Path("/usr/bin/sudo")
    if not systemd.is_file() or not sudo.is_file():
        emit(evidence, "NOT_AVAILABLE", expected, reason="native systemd/sudo provisioning unavailable")
        return 77
    user = pwd.getpwuid(os.geteuid()).pw_name
    python = str(pathlib.Path(sys.executable).resolve(strict=True))
    unit = "lu-sec06c-" + secrets.token_hex(7) + ".service"
    # System-manager privilege is used ONLY to create a transient delegated
    # unit with User=<original nonroot uid>. All test code runs inside as User.
    # No fallback to root execution or a synthetic mount is permitted.
    env_keys = ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME",
                "PKG_CONFIG_PATH", "LD_LIBRARY_PATH", "LU_TOOLS03_TEST_GIT_PATH"]
    env = [f"--setenv={key}={os.environ[key]}" for key in env_keys if key in os.environ]
    argv = [
        str(sudo), "-n", str(systemd), "--system", "--quiet", "--wait",
        "--pipe", "--collect", "--service-type=exec",
        f"--unit={unit}", "--property=Delegate=yes", f"--property=User={user}",
        f"--working-directory={REPO}", *env,
        python, str(pathlib.Path(__file__).resolve()), "--inside",
        "--expected-sha", expected, "--evidence", str(evidence),
    ]
    print("SEC06C_REQUEST_REAL_SYSTEMD_DELEGATION=1", flush=True)
    try:
        process = subprocess.run(argv, cwd=REPO, check=False, timeout=9000)
    except (OSError, subprocess.TimeoutExpired) as exc:
        if not (evidence / NAME).exists():
            emit(evidence, "NOT_AVAILABLE", expected, reason=f"systemd delegate launch failed: {type(exc).__name__}")
        return 77
    evidence_file = evidence / NAME
    if not evidence_file.is_file():
        emit(evidence, "NOT_AVAILABLE", expected, reason="systemd did not launch verifiable nonroot delegated service")
        return 77
    try:
        info = json.loads(evidence_file.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return 1
    if info.get("head") != expected or info.get("status") == "NOT_AVAILABLE":
        return 77
    if info.get("status") != "DELEGATED_KERNEL_PREFLIGHT_ONLY" or info.get("euid") != os.geteuid():
        return 1
    proof = evidence / "linux.receipt.json"
    if not proof.is_file() or process.returncode != 0:
        return 1
    try:
        record = json.loads(proof.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return 1
    if (record.get("head") != expected or record.get("allNativeCasesPassed") is not True
            or record.get("status") != "PROVISIONAL_NATIVE_PASS_OWNER_GATE_PENDING"):
        return 1
    return 0


def main():
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--outer", action="store_true")
    group.add_argument("--inside", action="store_true")
    parser.add_argument("--expected-sha", required=True)
    parser.add_argument("--evidence", required=True)
    args = parser.parse_args()
    if len(args.expected_sha) != 40 or any(c not in "0123456789abcdef" for c in args.expected_sha):
        parser.error("expected SHA must be an exact lowercase Git commit")
    evidence = pathlib.Path(args.evidence).resolve()
    if not evidence.is_relative_to(REPO) or evidence == REPO:
        parser.error("evidence must be in the verified checkout")
    try:
        code = outer(args.expected_sha, evidence) if args.outer else inside(args.expected_sha, evidence)
    except Exception as exc:
        print(f"SEC06C_BLOCKED_NATIVE_DELEGATION: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 1
    return code


if __name__ == "__main__":
    sys.exit(main())
