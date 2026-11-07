#!/usr/bin/env python3
"""Test-only three-way LLDB-DAP initialization witness.

The original distro binary, a root-owned copied bundle, and that bundle in
the *same Bubblewrap mount/namespace shape* are separately examined. Only the
fixed initialize request is sent; NO project executable is launched. Output
contains fixed status categories, never raw stderr, project paths or DAP data.
This preflight is NOT proof of kernel Job/cgroup acceptance.
"""
import json
import os
import selectors
import subprocess
import time

SOURCE = "/usr/bin/lldb-dap-18"
BUNDLE = "/opt/lu-ide-wo6-dap-test"
COPIED = BUNDLE + "/bin/lldb-dap"

NAMESPACE = [
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


def category(raw: bytes) -> str:
    lower = raw.decode("utf-8", "replace").lower()
    if "error while loading shared libraries" in lower or "cannot open shared object file" in lower:
        return "shared_library_loader"
    if "liblldb" in lower:
        return "liblldb_runtime"
    if "python" in lower:
        return "python_runtime"
    if "permission denied" in lower or "operation not permitted" in lower:
        return "permission_denied"
    if "unable to create" in lower or "failed to create" in lower:
        return "cannot_create_runtime_state"
    if "resource" in lower or "platform" in lower:
        return "lldb_resource_or_platform"
    if "not found" in lower or "no such file" in lower:
        return "dependency_missing"
    if "error" in lower or "fatal" in lower:
        return "other_adapter_error"
    return "empty" if not raw else "other_nonempty"


def witness(stage: str, argv: list[str]) -> bool:
    env = {"PATH": "/usr/bin:/bin", "HOME": "/tmp", "LANG": "C"}
    request = {
        "seq": 1, "type": "request", "command": "initialize",
        "arguments": {
            "adapterID": "lu-ide", "clientID": "lu-ide",
            "linesStartAt1": True, "columnsStartAt1": True,
            "pathFormat": "path", "supportsRunInTerminalRequest": False,
            "supportsVariableType": False
        }
    }
    encoded = json.dumps(request, separators=(",", ":")).encode()
    try:
        child = subprocess.Popen(
            argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, env=env, start_new_session=True
        )
    except OSError:
        print(f"PROTECTED_WO6_DAP_PREFLIGHT={stage}:spawn_denied")
        return False
    stdout, stderr = bytearray(), bytearray()
    ready = False
    sel = selectors.DefaultSelector()
    try:
        assert child.stdin and child.stdout and child.stderr
        child.stdin.write(b"Content-Length: %d\\r\\n\\r\\n" % len(encoded) + encoded)
        child.stdin.flush()
        sel.register(child.stdout, selectors.EVENT_READ, "out")
        sel.register(child.stderr, selectors.EVENT_READ, "err")
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and not ready:
            if child.poll() is not None and not sel.get_map():
                break
            for key, _ in sel.select(0.15):
                chunk = os.read(key.fileobj.fileno(), 8192)
                if not chunk:
                    sel.unregister(key.fileobj)
                    continue
                output = stdout if key.data == "out" else stderr
                output.extend(chunk[:max(0, 65536 - len(output))])
                if len(stdout) > 65536 or len(stderr) > 65536:
                    break
            header_end = stdout.find(b"\\r\\n\\r\\n")
            if header_end >= 0 and header_end < 512:
                header = bytes(stdout[:header_end])
                try:
                    size = int(header.split(b"Content-Length: ", 1)[1].split(b"\\r\\n")[0])
                    if size <= 0 or size > 65536:
                        break
                    if len(stdout) >= header_end + 4 + size:
                        body = json.loads(stdout[header_end + 4:header_end + 4 + size])
                        ready = body.get("type") == "response" and (
                            body.get("command") == "initialize"
                            and body.get("success") is True
                        )
                        break
                except (ValueError, IndexError, json.JSONDecodeError):
                    break
    except (OSError, BrokenPipeError):
        pass
    finally:
        sel.close()
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=1)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=2)
        if child.stdin:
            child.stdin.close()
        if child.stdout:
            child.stdout.close()
        if child.stderr:
            child.stderr.close()
    if ready:
        print(f"PROTECTED_WO6_DAP_PREFLIGHT={stage}:initialize_pass")
    else:
        print(f"PROTECTED_WO6_DAP_PREFLIGHT={stage}:no_initialize:{category(bytes(stderr))}")
    return ready


def main() -> int:
    stages = [
        ("os_distro_executable", [SOURCE]),
        ("copied_root_owned_bundle", [COPIED]),
        ("copied_inside_readonly_namespace", NAMESPACE),
    ]
    successes = [witness(label, command) for label, command in stages]
    # Diagnostics are valuable even if one stage fails, but do not mislabel
    # the native kernel fixture as passed when protocol preflight failed.
    return 0 if all(successes) else 2


if __name__ == "__main__":
    raise SystemExit(main())
