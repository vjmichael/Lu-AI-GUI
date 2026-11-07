# IDE Task 2 — Linux native containment evidence

**Date:** 2026-11-04  
**Status:** **PARTIAL — native kernel probes, not production executable-worker acceptance**  
**Repository branch:** `ide/workspace-development`

## Tested implementation

- Standalone Rust crate `ide/sandbox`, isolated from the IDE Tauri UI.
- No general-purpose command execution method; only fixed, explicitly compiled-in diagnostic probes.
- Uses an absolute, root-owned, non-setuid, non-writable `/usr/bin/bwrap`.
- Constructs a new network/PID/user/mount namespace, a new session and parent-death cleanup.
- Mounts minimal runtime directories read-only, an empty `/home` and `/tmp` temporary filesystem, with project fixture at `/workspace` **read-only**.
- Clears inherited environment and refuses direct execution fallback.
- Refuses broad system directories as a project root; the updated version also requests that nested user namespaces be disabled and tests an execution deadline.

## Verified Linux results

**First successful kernel run:** https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41966511290

- Ubuntu 26.04.1 LTS, kernel 7.0.0-1017-azure, x86-64.
- Rust 1.88 static suite: **3/3 passed**.
- Rust formatting check: passed.
- Bubblewrap real OS checks: **4/4 passed**:
  1. Namespace setup.
  2. Host credential path exclusion.
  3. Read-only mounted workspace.
  4. External network connection denied.

**Expanded hardened run:** https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41967265761

- Linux native Rust static tests **3/3 passed**, rustfmt passed.
- Bubblewrap 0.11.1, Ubuntu 26.04.1 LTS.
- Five actual kernel-level checks **5/5 passed**: namespace setup, hidden host secret paths, read-only workspace, offline network, and fixed long-running process termination on deadline.
- The tested policy now passes `--unshare-all --unshare-user --disable-userns`, explicitly refusing nested user namespaces.
- Previous attempts with `--disable-userns` but no explicit `--unshare-user` failed **closed**. The corrected combination passed.
- This is **not** evidence of resource quotas, process descendant cleanup on arbitrary complex workloads, or resistance to kernel vulnerabilities.

## Outstanding security gates

The following are NOT validated and MUST NOT be described as safe or enabled:
- Actual arbitrary project code or build tools in a restricted worker.
- CPU/RAM/PID/file-size quotas and runaway fork/IO stress under OS-managed resource limits.
- Long-lived process-tree cancellation and cross-crash orphan cleanup under adversarial load.
- TOCTOU-safe pinned directory file descriptors and Linux Landlock/seccomp layered confinement.
- Stable absolute executable/toolchain admission and supply-chain pinning for downloaded compilers or package managers.
- Per-operation user grants, audit/event receipts and atomic write/rollback in the standalone IDE host.
- Secure package/network download broker, no credential leaks and no user namespace escape.
- Windows execution sandbox, restricted token/AppContainer and actual hostile worker tests.
- Native installed-app UI acceptance (interactive folder picker/reopen/restart).
- Any Lu bridge or model-originated code execution. No bridge exists yet.

## Why this is not enough to run arbitrary code

Bubblewrap is a low-level sandbox **builder**. Its safety depends on mount, namespace, credential, PID, network and syscall policy. Passing fixed probes demonstrates those specific properties on the tested kernel but does not prove protection against every malicious language server, build system, package hook, compiler or escape. The IDE's executable operations remain disabled until stronger independent tests pass.

References:
- https://github.com/containers/bubblewrap
- https://docs.kernel.org/userspace-api/landlock.html
