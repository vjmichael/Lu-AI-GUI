# DEV-04.1 — Project debugger target provenance preparation

**Scope:** first task after DEV-03.5 implementation. This task is a native
non-executing review; it does NOT convert the existing fixed LLDB-DAP
self-test into a general project debugger.

## Implemented

Only the registered active project may request an exact saved `Cargo.toml`
SHA-256 review. The native host requires a clean committed Git HEAD, zero
tracked/staged/untracked modifications, capability-bound non-symlink
manifest read and a second source/worktree consistency check. A
domain-separated review fingerprint covers project identity, fixed
debug profile, manifest SHA and clean HEAD.

The returned contract explicitly says:
- no sealed executable and no completed native-build receipt bound;
- no binary SHA or independently authenticated source-to-output binding;
- no verified debug symbols or approved project adapter;
- target executable `null`, no process started, no authorization issued;
- no installed application acceptance.

The Jobs UI displays the read-only source review. Its JS shape guard
rejects fabricated executable paths, installed success and grants. The
native command does not accept arbitrary commands, binary paths, runtime
environment or adapter operations.

Native Windows/Linux fixtures include a real Git/manifest review,
stale SHA, dirty worktree, closed project and Unix symlink denial.

## Remaining task sequence

1. **DEV-04.2:** Bind a completed exact-job native `rust_build` receipt
   and independently hashed output manifest to its frozen source snapshot
   and toolchain, without accepting a user-supplied binary path.
2. **DEV-04.3:** Identify and attest one reviewed binary and debug-symbol
   closure, with no symlink/reparse/hardlink traversal or arbitrary
   output-identifier selection; reject missing debug information.
3. **DEV-04.4:** Implement a project debug launch only after a fresh
   human OS approval and the same fully protected Windows/Linux worker
   custody, including LLDB-DAP fixed transport, cancellation and crash
   reconciliation. Preserve source-to-binary evidence over restart.
4. **DEV-04.5:** Genuine installed Windows/Linux source→build→debug
   normal/cancel/restart and hostile probe acceptance. A source test,
   debug fixture or unsigned default desktop does not close this gate.

**Open dependencies:** DEV-03.3B/03.4/03.5 exact-head CI; actual Linux
cgroup-v2 and Windows AppContainer/SCM worker acceptance; independently
pinned debug adapter distribution, signed/published provenance and legal
release checks. Never use source-only reviews to bypass those gates.
