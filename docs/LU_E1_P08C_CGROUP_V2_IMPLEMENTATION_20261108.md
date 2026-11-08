# E1-P08C — delegated Linux cgroup-v2 custody (source candidate)

## Scope

This is the **separate** strong-custody implementation for a future class
of escape-capable reviewed external application operations.

**Existing E1-P08 cooperative Git/FFprobe operations still use the
process-group guardian.** That code is unchanged on this branch. There is
no automatic fallback from strong cgroup mode to PGID mode. Lu Core
remains an ordinary native, non-sandboxed application. These boundaries
only concern external worker process lifetimes.

## Implemented native code

- `lu-win-worker-job/src/linux_cgroup.rs` provides
  `LinuxCgroupGuardian::spawn()`, `bind_before_exec()` and
  `stop_and_verify_empty()`, and a first-party
  `lu-linux-cgroup-guard` executable. None is exposed as an arbitrary
  model tool, general shell runner or webview-selected PID/path.
- The guardian obtains its own cgroup-v2 identity from
  `/proc/self/cgroup` and the kernel mount from `/proc/self/mountinfo`.
  It creates a **new child** cgroup under its inherited parent only if
  the kernel actually permits creation. No guessed paths or sudo calls.
  It refuses non-directory/symlink components, missing `cgroup.kill`,
  and a pre-populated worker leaf.
- The guardian lives **in the delegated parent**, not in the worker leaf,
  and keeps a host-only stdin lifetime pipe. The host opens a pinned
  `cgroup.procs` FD and attaches each worker through a
  pre-exec write of `0`: the worker cannot execute third-party code
  before the kernel membership operation completes successfully.
- If Lu is SIGKILLed, its pipe closes without relying on Rust Drop.
  The guardian writes `1` into its owned leaf `cgroup.kill`, waits
  until `cgroup.events` reports recursive `populated 0`, then
  removes empty nested cgroups bottom-up within a bounded 16-level,
  1,024-directory, no-symlink walk before removing the leaf. On orderly host cleanup the parent requires
  the guardian's successful exit and absence of the former leaf.
- A missing delegated subtree or first-party guardian image is a
  **hard admission failure** for this path. There is no unchecked
  process-group fallback. Test-only helper selection exists only in
  debug builds, and the destructive crash-host fixture is compiled
  only with the explicit `e1-p08c-cgroup-crash-fixture` Cargo feature.

## Independent OS acceptance (OPEN until native results)

The dedicated `.github/workflows/lu-e1-p08c-cgroup-v2.yml` builds
native code on Windows and Linux and separately obtains a *real systemd
delegated service unit* on Ubuntu 24.04. The CI runner uses `sudo
systemd-run` **only to provision the test service**, with
`Delegate=yes` and `User=<runner>`; the Python orchestrator, Lu-like
Rust host, first-party guardian and subprocesses run as an unprivileged
user. The code under test never uses sudo.

The independent Python test:

1. Starts a genuine Rust host which obtains cgroup custody and places a
   worker in the leaf *before* exec.
2. Spawns a real `setsid()` descendant with a different PGID, then
   moves it into an **actual nested child cgroup** of the worker leaf.
   Requires the nested group to report `populated 1`, while the guard
   remains outside the killable leaf.
3. Records real `cgroup.events populated=1` and exact
   `/proc/<pid>/cgroup` identities for guardian, worker and child.
4. Kills the Lu-like host with `SIGKILL`, without Rust Drop, and requires
   the worker/escaped descendant to cease running, cgroup removal after
   a kernel `populated=0` check, and survival of an unrelated process.
5. Uses kernel `pidfd` handles for cleanup on test failure, not stale
   PIDs or a fake cgroup kill response.

If the CI kernel lacks actual writable delegation, this test MUST
**fail**. A skipped or mocked test cannot close acceptance.

## Important remaining assurance gates

- Actual current-head CI results are pending. This is committed source,
  **not yet a native acceptance pass** or an installed Linux artifact.
- Systemd is the cgroup tree's single manager. Product integration must
  use an explicitly delegated unit / trusted OS-managed scope, not
  indiscriminate directory creation within a nondelegated scope.
  `/sys/fs/cgroup` need not be writable in an ordinary user session.
- **Delegation alone is not adversarial migration confinement.** A worker
  executing with the same Linux UID and write permission on the parent
  `cgroup.procs` can potentially migrate itself outside the child leaf.
  It would then escape leaf-level `cgroup.kill`. Production full-tree
  custody requires an independently verified **nonmigratable** boundary
  (e.g. systemd-owned cgroup with kernel-denied escape, not just a
  writable same-UID ancestor). A hostile migration negative test and
  product integration remain OPEN. This candidate only proves
  cgroup-based custody against `setsid()`/PGID escape, once the native
  CI really passes.
- Recursive cgroup `populated 0` means no live processes in the worker
  subtree. It does **not** prove Lu `wait()`-reaped every adopted zombie.
- The separate reviewed Git/FFprobe guardian, model-selected actual GGUF
  tool acceptance, Linux release packaging/provenance, and physically
  installed Windows acceptance retain their own independent gates.

References:
- https://docs.kernel.org/admin-guide/cgroup-v2.html
- https://systemd.io/CONTROL_GROUP_INTERFACE/
