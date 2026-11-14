#!/usr/bin/env python3
"""P03 Windows test-only copied LLDB-DAP loader + initialize preflight.

Uses one fixed DAP initialize request against the CI-measured adapter bundle.
No project execution, native launcher permission, installer signing, process
attach, terminal/evaluate requests, network, or raw stderr emission. This is
NOT proof of AppContainer rights or installed native debugger acceptance.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time

MAX_BYTES = 65536
FRAME = {
    "seq": 1, "type": "request", "command": "initialize",
    "arguments": {
        "adapterID": "lu-ide", "clientID": "lu-ide",
        "linesStartAt1": True, "columnsStartAt1": True,
        "pathFormat": "path", "supportsRunInTerminalRequest": False,
        "supportsVariableType": False,
    },
}


def classify_exit(code):
    if code is None:
        return "timeout_or_unreaped"
    codes = {
        0xC0000135: "dependency_dll_missing",
        0xC0000142: "dll_initialization_failed",
        0xC0000022: "kernel_access_denied",
        0xC000007B: "invalid_binary_format",
    }
    return codes.get(code & 0xFFFFFFFF, "other")


def parse_initialize_response(stream):
    if len(stream) > MAX_BYTES:
        return False
    remaining = stream
    while remaining:
        end = remaining.find(b"\r\n\r\n")
        if end < 0 or end > 512:
            return False
        header = remaining[:end]
        match = re.fullmatch(rb"Content-Length: ([1-9][0-9]{0,4})", header)
        if match is None:
            return False
        length = int(match.group(1))
        if length > MAX_BYTES or len(remaining) < end + 4 + length:
            return False
        try:
            message = json.loads(remaining[end + 4:end + 4 + length])
        except (ValueError, UnicodeDecodeError):
            return False
        # Before initialize RESPONSE, DAP forbids unsolicited events and
        # reverse adapter requests. A copied binary that emits either must
        # not be counted as a healthy initialize preflight.
        if not isinstance(message, dict):
            return False
        return (message.get("type") == "response"
                and message.get("request_seq") == 1
                and message.get("command") == "initialize"
                and message.get("success") is True)
    return False


def run():
    # Test-only environmental comparison. This still runs OUTSIDE the
    # AppContainer and grants no native sandbox, debugger, or ACL authority.
    if sys.argv[1:] not in ([], ["--mirrored-env"]):
        print("PROTECTED_WO6_DAP_PREFLIGHT=denied_unknown_mode")
        return 2
    mirrored = sys.argv[1:] == ["--mirrored-env"]
    prefix = "PROTECTED_WO6_DAP_MIRRORED_ENV" if mirrored else "PROTECTED_WO6_DAP_PREFLIGHT"
    if sys.platform != "win32" or os.environ.get("GITHUB_ACTIONS") != "true":
        print("PROTECTED_WO6_DAP_PREFLIGHT=denied_non_ci_windows")
        return 2
    temp = Path(os.environ.get("RUNNER_TEMP", "")).resolve()
    root = Path(os.environ.get("LU_IDE_WO6_DAP_BUNDLE", "")).resolve()
    if (not root.is_relative_to(temp) or root == temp
        or not root.is_dir() or root.is_symlink()):
        print("PROTECTED_WO6_DAP_PREFLIGHT=denied_untrusted_bundle")
        return 2
    exe = root / "bin" / "lldb-dap.exe"
    if not exe.is_file() or exe.is_symlink():
        print("PROTECTED_WO6_DAP_PREFLIGHT=denied_missing_fixed_adapter")
        return 2
    body = json.dumps(FRAME, separators=(",", ":")).encode("utf-8")
    request = b"Content-Length: %d\r\n\r\n" % len(body) + body
    env = {
        "SystemRoot": os.environ.get("SystemRoot", r"C:\Windows"),
        "PATH": str(exe.parent) + ";" + str(Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"),
        "TEMP": str(temp), "TMP": str(temp),
    }
    if mirrored:
        # Under RUNNER_TEMP only; this mimics min_environment's variables,
        # NOT the AppContainer package SID, directory ACLs, or OS token.
        local = temp / "wo6-unconfined-env-probe" / "LocalState"
        local.mkdir(parents=True, exist_ok=True)
        env.update({key: str(local) for key in (
            "APPDATA", "HOME", "LOCALAPPDATA", "USERPROFILE", "TEMP", "TMP"
        )})
    try:
        child = subprocess.Popen([str(exe)], cwd=str(exe.parent),
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            env=env, creationflags=subprocess.CREATE_NO_WINDOW)
    except OSError:
        print(prefix + "=adapter_spawn_denied")
        return 0 if mirrored else 2
    # Do not use communicate(): it can buffer unlimited adapter output until
    # timeout. Reader threads capture at most MAX_BYTES+1 for each OS pipe.
    lock = threading.Lock()
    oversized = threading.Event()
    out = bytearray()
    err = bytearray()

    def drain(pipe, target):
        while True:
            try:
                piece = os.read(pipe.fileno(), 4096)
            except OSError:
                return
            if not piece:
                return
            with lock:
                allowance = max(0, MAX_BYTES + 1 - len(target))
                target.extend(piece[:allowance])
                if len(target) > MAX_BYTES or len(piece) > allowance:
                    oversized.set()

    readers = [
        threading.Thread(target=drain, args=(child.stdout, out), daemon=True),
        threading.Thread(target=drain, args=(child.stderr, err), daemon=True),
    ]
    for reader in readers:
        reader.start()
    try:
        child.stdin.write(request)
        child.stdin.flush()
    except OSError:
        pass  # Early loader failure still requires a fixed exit classification.
    deadline = time.monotonic() + 8
    ready = False
    while time.monotonic() < deadline and not oversized.is_set():
        with lock:
            observed = bytes(out)
        if parse_initialize_response(observed):
            ready = True
            break
        if child.poll() is not None:
            break
        time.sleep(0.05)
    timed_out = not ready and child.poll() is None and not oversized.is_set()
    actual_exit = child.poll()  # Before deliberate cleanup; keep loader status.
    if child.poll() is None:
        child.kill()
    child.wait(timeout=3)
    for reader in readers:
        reader.join(timeout=1)
    with lock:
        stdout = bytes(out)
    status = classify_exit(actual_exit)
    # No raw stdout/stderr, absolute paths, user project data or file names.
    if status not in ("other", "timeout_or_unreaped"):
        print(prefix + "_LOADER=" + status)
    if oversized.is_set():
        print(prefix + "=denied_unbounded_output")
        return 0 if mirrored else 2
    if ready and parse_initialize_response(stdout):
        print(prefix + "=initialize_pass_no_containment_claim")
        return 0
    print(prefix + "="
          + ("no_initialize_timeout" if timed_out else "no_initialize_" + status))
    return 0 if mirrored else 2


if __name__ == "__main__":
    raise SystemExit(run())
