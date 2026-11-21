#!/usr/bin/env python3
"""G03 genuine Bubblewrap checks; only run inside the new namespace."""
from pathlib import Path
import socket

source=Path("/source")
private=Path("/out")
assert source.read_bytes()==b"immutable-g03-source\n", "exact input not visible"
assert not Path("/home/runner").exists(), "host home exposed"
assert sorted(name for _, name in socket.if_nameindex())==["lo"], "network devices leaked"

def must_deny(action,label):
    try: action()
    except OSError: return
    raise AssertionError("uncontained filesystem operation: "+label)

must_deny(lambda: source.write_bytes(b"tampered"),"source-write")
must_deny(lambda: source.unlink(),"source-delete")
must_deny(lambda: source.rename("/source-renamed"),"source-rename")
proof=private/"positive-output.txt"
proof.write_bytes(b"allowed-private-only")
assert proof.read_bytes()==b"allowed-private-only"
print("IDE_G03_LINUX_BWRAP_REAL_SOURCE_WRITE_RENAME_DELETE_DENIED=PASS",flush=True)
print("IDE_G03_LINUX_BWRAP_REAL_NETWORK_NAMESPACE_LOOPBACK_ONLY=PASS",flush=True)
print("IDE_G03_LINUX_BWRAP_HOME_NOT_MOUNTED_PRIVATE_OUTPUT=PASS",flush=True)
