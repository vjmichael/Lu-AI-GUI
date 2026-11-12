# FIN-02C — Native build input preparation

Run `prepare_fin02_runtime_pins.py` only on a machine where the real
reviewed runtime/service trees are already present at the **exact native
admission paths**. The script expects an independently maintained
`lu.ide.fin02.independent-input-pins.v1` JSON with exact profile keys
and reviewer-approved SHA-256 tree hashes; the user cannot pass an
arbitrary runtime path or command.

It rehashes real bytes using the native Windows Cargo/LLDB and runtime
transcripts, checks fixed binary/notice presence, refuses symlinks,
special files and quotas, then writes
`ide/app/src-tauri/fin02-runtime-pins.json` **once**. The Cargo
`fin02-protected-alpha` build remeasures the same trees independently.

Existing staging/NSIS and Linux Debian scripts must actually copy/install
the reviewed roots and preserve their protected OS identities. Without
those materials, the stage refuses and the final integrated native build
remains blocked. A stage manifest is not code signing, license
verification, successful AppContainer/SCM/cgroup-v2 isolation or installed
acceptance. No binaries were fetched or installed in this change.
