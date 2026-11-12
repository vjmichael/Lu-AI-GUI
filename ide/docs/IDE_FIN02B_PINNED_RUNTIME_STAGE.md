# FIN-02B — Actual pinned native runtime build-stage admission

The opt-in `fin02-protected-alpha` Cargo build now requires both the existing
remeasured offline LSP stage **and** `fin02-runtime-pins.json` under the
Tauri source root. Its schema is `lu.ide.fin02.runtime-pins.v1` and
it must have `platform`, `releaseInstalled:false`,
`nativeHumanAcceptance:false`, `executionAuthorized:false` and an
exact `pins` mapping.

The build script remeasures fixed resource roots **instead of trusting
environment variables or a caller-provided path**:
- Windows: `resources/build-tools/windows-rust-offline` (the existing
  native D7 tree algorithm), `resources/build-tools/windows-runtimes`
  for Python/Node/CMake (native runtime tree transcript),
  `resources/debug-adapters/lldb-dap` (native debugger bundle transcript),
  and `resources/recovery/windows_recovery_service.exe`.
- Linux: root-owned /usr/bin/bwrap/cargo/rustc and the first fixed trusted
  GNU linker (existing native Cargo identity transcript), plus fixed,
  independently provisioned /opt/lu-ide/runtimes/{profile} and
  /opt/lu-ide/debug-adapters/lldb-dap trees.
- A mismatch, missing runtime, missing notice, malformed digest, extra
  pin or unapproved platform **aborts the protected-alpha build**.
  Only observed real digests are embedded through rustc-env.

This is a build-stage source implementation. There are currently no
verified runtime binaries nor an approved complete stage manifest in
the repository; the opt-in build is therefore **expected to refuse**.
Actual Windows AppContainer SCM admission, Job Object containment,
Linux cgroup-v2/Bubblewrap delegation and installed user consent still
must be observed on actual target machines; a matching hash is not that
evidence. Linux privileged /opt provisioning and Windows NSIS complete
payload remain blocking distribution work. No CI/PR/native installed
acceptance was executed.
