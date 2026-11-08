#!/usr/bin/env python3
"""WO6 diagnostic-only real LLDB-DAP fixed launch probe.

Only probes the independently prepared, root-owned test bundle and a fixed
first-party debuggee on an ephemeral GitHub Linux runner. The actual kernel
cgroup/delegated-host witness is separate and mandatory. No project path,
user-selected executable, untrusted arguments, terminal, raw DAP data or
stderr ever reaches the output. Every process is terminated as a group.
"""
import json
import os
import selectors
import signal
import subprocess
import time
from pathlib import Path

BUNDLE = "/opt/lu-ide-wo6-dap-test"
ADAPTER = BUNDLE + "/bin/lldb-dap"
TARGET = BUNDLE + "/debuggee/wo6-selftest"
BWRAP = [
    "/usr/bin/bwrap", "--die-with-parent", "--new-session",
    "--unshare-all", "--clearenv",
    "--ro-bind", "/usr", "/usr",
    "--ro-bind-try", "/bin", "/bin",
    "--ro-bind-try", "/lib", "/lib",
    "--ro-bind-try", "/lib64", "/lib64",
    "--ro-bind", BUNDLE, "/ls",
    "--tmpfs", "/tmp", "--proc", "/proc", "--dev", "/dev",
    "--setenv", "HOME", "/tmp",
    "--setenv", "PATH", "/ls/bin:/usr/bin:/bin",
    "--setenv", "LANG", "C",
    "--chdir", "/tmp", "--", "/ls/bin/lldb-dap",
]
MAX_BYTES = 65536
REQUEST_INIT = {
    "seq": 1, "type": "request", "command": "initialize",
    "arguments": {
        "adapterID": "lu-ide", "clientID": "lu-ide",
        "linesStartAt1": True, "columnsStartAt1": True,
        "pathFormat": "path", "supportsRunInTerminalRequest": False,
        "supportsVariableType": False,
    },
}


def launch_request(fixed_target):
    return {
        "seq": 2, "type": "request", "command": "launch",
        "arguments": {
            "program": fixed_target, "stopOnEntry": True, "noDebug": False,
            "disableASLR": False, "disableSTDIO": True,
            "args": [], "env": [], "runInTerminal": False,
        },
    }


def frame(payload):
    encoded = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    assert 0 < len(encoded) < 4096
    return b"Content-Length: %d\r\n\r\n" % len(encoded) + encoded


def parse(buffer):
    """Consume only bounded complete DAP frames, no raw output to logs."""
    messages = []
    while buffer:
        sep = buffer.find(b"\r\n\r\n")
        if sep < 0:
            if len(buffer) > 512:
                raise ValueError("invalid_header")
            break
        if sep > 512:
            raise ValueError("long_header")
        header = bytes(buffer[:sep]).split(b"\r\n")
        sizes = [line[16:] for line in header
                 if line.lower().startswith(b"content-length: ")]
        if len(sizes) != 1 or not sizes[0].isdigit():
            raise ValueError("invalid_length")
        size = int(sizes[0])
        if size <= 0 or size > MAX_BYTES:
            raise ValueError("oversized_frame")
        end = sep + 4 + size
        if len(buffer) < end:
            break
        try:
            message = json.loads(buffer[sep + 4:end])
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ValueError("invalid_json") from exc
        del buffer[:end]
        if not isinstance(message, dict):
            raise ValueError("invalid_message")
        messages.append(message)
    return messages


def classification(response):
    """Fixed-vocabulary native diagnostics; never print error payload."""
    body = response.get("body")
    error = body.get("error") if isinstance(body, dict) else {}
    error = error or {}
    fragments = []
    if isinstance(error, str):
        fragments.append(error)
    if isinstance(error, dict):
        fragments.extend(str(error.get(k, "")) for k in ("format", "description", "message"))
    fragments.extend(str(response.get(k, "")) for k in ("message", "description"))
    text = " ".join(fragment[:384] for fragment in fragments).lower()
    if any(word in text for word in ("env must", "environment", "invalid argument", "json", "type error")):
        return "launch_schema_or_arguments"
    if any(word in text for word in ("ptrace", "trace me", "debugserver")):
        return "debugger_trace_permission"
    if any(word in text for word in ("permission denied", "operation not permitted", "access denied")):
        return "kernel_or_file_permission"
    if any(word in text for word in ("personality", "aslr")):
        return "aslr_policy"
    if any(word in text for word in ("file not found", "no such file", "not found", "not exist")):
        return "fixed_binary_or_runtime_missing"
    if any(word in text for word in ("failed to launch", "launch failed", "unable to launch", "spawn")):
        return "debuggee_launch_failed"
    return "adapter_failure_other" if text else "empty_error"


def kill_group(proc):
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
    try:
        proc.wait(timeout=3)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=3)


def witness(stage, argv, program):
    env = {"HOME": "/tmp", "LANG": "C", "PATH": "/usr/bin:/bin"}
    try:
        proc = subprocess.Popen(
            argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, env=env, start_new_session=True,
        )
    except OSError:
        print("P03_LLDB_FIXED_LAUNCH_DIAGNOSTIC=" + stage + ":spawn_denied", flush=True)
        return
    pending = bytearray()
    status = "no_initialize"
    selector = selectors.DefaultSelector()
    try:
        assert proc.stdin and proc.stdout and proc.stderr
        selector.register(proc.stdout, selectors.EVENT_READ, "stdout")
        # stderr is NEVER echoed; bounded reads prevent deadlock
        selector.register(proc.stderr, selectors.EVENT_READ, "stderr")
        proc.stdin.write(frame(REQUEST_INIT))
        proc.stdin.flush()
        deadline = time.monotonic() + 17
        sent_launch = False
        while time.monotonic() < deadline:
            if proc.poll() is not None and not selector.get_map():
                status = "adapter_terminated"
                break
            for key, _ in selector.select(timeout=0.15):
                chunk = os.read(key.fileobj.fileno(), 4096)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                if key.data == "stdout":
                    pending.extend(chunk)
                    if len(pending) > MAX_BYTES * 2:
                        status = "protocol_buffer_limit"
                        return
            try:
                messages = parse(pending)
            except ValueError:
                status = "invalid_dap_frame"
                break
            for msg in messages:
                if msg.get("type") != "response":
                    continue
                if (msg.get("request_seq") == 1 and
                        msg.get("command") == "initialize" and not sent_launch):
                    if msg.get("success") is not True:
                        status = "initialize_failed"
                        return
                    proc.stdin.write(frame(launch_request(program)))
                    proc.stdin.flush()
                    sent_launch = True
                    status = "launch_sent"
                elif (msg.get("request_seq") == 2 and
                      msg.get("command") == "launch" and sent_launch):
                    status = ("launch_ack_not_containment_pass" if
                              msg.get("success") is True else
                              "launch_rejected_" + classification(msg))
                    return
            if proc.poll() is not None:
                status = "adapter_exited_" + status
                break
    except (BrokenPipeError, OSError):
        status = "transport_io_failed"
    finally:
        selector.close()
        kill_group(proc)
        if proc.stdin:
            proc.stdin.close()
        if proc.stdout:
            proc.stdout.close()
        if proc.stderr:
            proc.stderr.close()
        print("P03_LLDB_FIXED_LAUNCH_DIAGNOSTIC=" + stage + ":" + status,
              flush=True)


def main():
    # No CLI controls or user files. CI prepares the same fixed root-owned
    # bundle as the native delegated kernel acceptance job.
    for name in (ADAPTER, TARGET, "/usr/bin/bwrap"):
        path = Path(name)
        if path.is_symlink() or not path.is_file():
            print("P03_LLDB_FIXED_LAUNCH_DIAGNOSTIC=fixed_bundle_missing",
                  flush=True)
            return 0  # Diagnostic, not native authorization.
    # Only execute a debuggee inside Bwrap. The existing initialize-only
    # witness covers the unsandboxed distribution and copied adapter.
    witness("copied_readonly_namespace", BWRAP, "/ls/debuggee/wo6-selftest")
    print("P03_LLDB_FIXED_LAUNCH_DIAGNOSTIC_ONLY=not_kernel_cgroup_acceptance",
          flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
