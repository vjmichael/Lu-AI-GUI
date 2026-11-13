#!/usr/bin/env python3
"""FIN-04 read-only CI diagnostic: fixed LLDB-DAP launch in three OS contexts.

This is NOT a protected execution grant, a debugger acceptance, or a runner
policy change. It executes only the repository's compiled wo6-selftest,
bounded to 8 seconds each. Never prints DAP bytes, stderr, paths or PIDs.
"""
import json
import os
import selectors
import signal
import subprocess
import time

from probe_lldb_dap_initialize import SOURCE, COPIED, NAMESPACE, BUNDLE

FIXED_HOST = BUNDLE + "/debuggee/wo6-selftest"
CASES = (
    ("host_distro", [SOURCE], FIXED_HOST),
    ("copied_bundle", [COPIED], FIXED_HOST),
    ("bwrap_namespace", NAMESPACE, "/ls/debuggee/wo6-selftest"),
)

def classify(fragment: str) -> str:
    text = fragment[:1024].lower()
    if "ptrace" in text or "trace me" in text:
        return "ptrace_denied"
    if "personality" in text or "aslr" in text:
        return "aslr_personality_denied"
    if "operation not permitted" in text or "permission denied" in text:
        return "kernel_permission_denied"
    if "seccomp" in text or "apparmor" in text or "sandbox" in text:
        return "sandbox_policy_denied"
    if "no such file" in text or "not found" in text:
        return "fixed_dependency_missing"
    if "liblldb" in text or "python" in text or "resource" in text:
        return "debugger_resource_missing"
    if "unable to launch" in text or "failed to launch" in text or "spawn" in text:
        return "fixed_launch_rejected"
    if "error" in text or "fail" in text:
        return "other_adapter_failure"
    return "no_adapter_detail" if not text else "other_reported_detail"

def request(seq: int, command: str, args: dict) -> bytes:
    body = json.dumps({
        "seq": seq, "type": "request", "command": command, "arguments": args
    }, separators=(",", ":")).encode()
    return b"Content-Length: %d\r\n\r\n" % len(body) + body

def stage(label: str, argv: list[str], target: str) -> bool:
    child = None
    sel = selectors.DefaultSelector()
    buffer = bytearray()
    stderr = bytearray()
    sent_launch = False
    got_launch = False
    got_stop = False
    failure = "timeout_or_disconnect"
    try:
        child = subprocess.Popen(
            argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env={"PATH": "/usr/bin:/bin", "HOME": "/tmp", "LANG": "C"},
            start_new_session=True
        )
        assert child.stdin and child.stdout and child.stderr
        child.stdin.write(request(1, "initialize", {
            "adapterID": "lu-ide", "clientID": "lu-ide",
            "linesStartAt1": True, "columnsStartAt1": True,
            "pathFormat": "path", "supportsRunInTerminalRequest": False,
        }))
        child.stdin.flush()
        sel.register(child.stdout, selectors.EVENT_READ, "out")
        sel.register(child.stderr, selectors.EVENT_READ, "err")
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline and not (got_launch and got_stop):
            for key, _ in sel.select(0.15):
                raw = os.read(key.fileobj.fileno(), 8192)
                if not raw:
                    sel.unregister(key.fileobj)
                    continue
                into = buffer if key.data == "out" else stderr
                into.extend(raw[:max(0, 32768 - len(into))])
            while True:
                end_header = buffer.find(b"\r\n\r\n")
                if end_header < 0 or end_header > 512:
                    break
                header = bytes(buffer[:end_header])
                try:
                    size = int(header.split(b"Content-Length: ", 1)[1].split(b"\r\n")[0])
                except (ValueError, IndexError):
                    failure = "invalid_frame"
                    break
                if size <= 0 or size > 32768:
                    failure = "invalid_frame"
                    break
                end = end_header + 4 + size
                if len(buffer) < end:
                    break
                try:
                    item = json.loads(buffer[end_header + 4:end])
                except (ValueError, UnicodeError):
                    failure = "invalid_frame"
                    break
                del buffer[:end]
                if item.get("type") == "response" and item.get("request_seq") == 1:
                    if item.get("command") == "initialize" and item.get("success") is True:
                        if not sent_launch:
                            child.stdin.write(request(2, "launch", {
                                "program": target, "stopOnEntry": True, "noDebug": False,
                                "disableASLR": False, "disableSTDIO": True,
                                "args": [], "env": {}, "runInTerminal": False,
                            }))
                            child.stdin.flush()
                            sent_launch = True
                    else:
                        failure = "initialize_rejected"
                        break
                elif item.get("type") == "response" and item.get("request_seq") == 2:
                    if item.get("command") != "launch":
                        failure = "uncorrelated_response"
                        break
                    if item.get("success") is True:
                        got_launch = True
                    else:
                        body = item.get("body")
                        raw_error = body.get("error", {}) if isinstance(body, dict) else {}
                        detail = (raw_error.get("format", "") if isinstance(raw_error, dict)
                                  else str(raw_error))
                        failure = classify(str(detail) + " " + str(item.get("message", "")))
                        break
                elif item.get("type") == "event" and item.get("event") == "stopped":
                    got_stop = True
            if got_launch and got_stop:
                break
            if failure != "timeout_or_disconnect":
                break
            if child.poll() is not None and not sel.get_map():
                break
        if not got_launch:
            if failure == "timeout_or_disconnect":
                failure = "process_exited" if child.poll() is not None else "response_not_received"
        elif not got_stop:
            failure = "launch_ack_without_kernel_stop"
    except (OSError, BrokenPipeError, ValueError):
        failure = "native_pipe_or_spawn_failure"
    finally:
        sel.close()
        if child:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait(timeout=4)
            for pipe in (child.stdin, child.stdout, child.stderr):
                if pipe:
                    pipe.close()
    outcome = "launch_stop_observed_not_os_attested" if got_launch and got_stop else failure
    print(f"FIN04_NATIVE_LAUNCH_PROBE={label}:{outcome}", flush=True)
    return got_launch and got_stop

def main() -> int:
    outcomes = [stage(*entry) for entry in CASES]
    print("FIN04_NATIVE_LAUNCH_PROBE_SUMMARY="
          + ("all_pass_not_kernel_custody" if all(outcomes) else "one_or_more_denied"))
    # Diagnostic is intentionally non-authoritative. The original protected
    # DAP + delegated cgroup fixture MUST independently pass or stay blocked.
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
