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
        if (isinstance(message, dict)
            and message.get("type") == "response"
            and message.get("request_seq") == 1
            and message.get("command") == "initialize"):
            return message.get("success") is True
        remaining = remaining[end + 4 + length:]
    return False


def run():
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
    try:
        child = subprocess.Popen([str(exe)], cwd=str(exe.parent),
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            env=env, creationflags=subprocess.CREATE_NO_WINDOW)
    except OSError:
        print("PROTECTED_WO6_DAP_PREFLIGHT=adapter_spawn_denied")
        return 2
    timed_out = False
    try:
        try:
            stdout, stderr = child.communicate(input=request, timeout=8)
        except subprocess.TimeoutExpired:
            timed_out = True
            child.kill()
            stdout, stderr = child.communicate(timeout=3)
    finally:
        if child.poll() is None:
            child.kill()
            child.wait(timeout=3)
    status = classify_exit(child.returncode)
    # No output of adapter stderr or paths. The code is only a loader hint.
    if status != "other":
        print("PROTECTED_WO6_DAP_PREFLIGHT_LOADER=" + status)
    if len(stdout) > MAX_BYTES or len(stderr) > MAX_BYTES:
        print("PROTECTED_WO6_DAP_PREFLIGHT=denied_unbounded_output")
        return 2
    if parse_initialize_response(stdout):
        print("PROTECTED_WO6_DAP_PREFLIGHT=initialize_pass_no_containment_claim")
        return 0
    print("PROTECTED_WO6_DAP_PREFLIGHT="
          + ("no_initialize_timeout" if timed_out else "no_initialize_" + status))
    return 2


if __name__ == "__main__":
    raise SystemExit(run())
