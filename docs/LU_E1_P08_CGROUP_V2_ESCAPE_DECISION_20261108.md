# E1-P08 decision: cgroup v2 for escaped Linux descendants
Date: November 8, 2026

## Engineering decision

**YES: a securely owned cgroup-v2 subtree is required if Lu promises to
terminate ALL OS descendants of an escape-capable reviewed application.**

The existing first-party process-group guardian is useful for fixed Git
and FFprobe workers and ordinary descendants that remain in its PGID.
It cannot truthfully promise termination of a descendant that calls
`setsid()`, `setpgid()`, or daemonizes into another process group.

This is NOT a decision to sandbox Lu Core. Lu itself remains a normal,
native, non-sandboxed desktop application; the stronger process custody
is solely for Lu-created *child workers* and executable adapters.

## Evidence — distinguish current and prior native results

- On November 8, a controlled real Linux kernel experiment spawned one
  group leader and one `setsid()` descendant. The descendant had a
  different PGID but **the same cgroup membership**. After
  `killpg(leader_pgid,SIGKILL)`, the leader died and the escaped child
  remained runnable. A pinned `pidfd` then terminated the exact child.
  This proves the limitation of process-group ownership, not Lu's
  full installed-app behavior.
- The same negative control is now committed as
  `benchmarks/e1/e1_p08_setsid_escape_probe.py` and required alongside
  the real host-SIGKILL + grandchild guardian acceptance workflow.
- Earlier E1-P08 native Windows and Ubuntu portable SQLite tests passed
  (Windows 3/3; Ubuntu 4/4), but the earlier Windows Tauri test compile
  failed with Rust E0369 caused by an invalid test
  `matches!("git"|"media",...)`. This has a source fix on PR #130.
  Those are earlier-SHA receipts; do not claim exact-current-head pass.
- Latest-sha Windows/Linux jobs remain open until their *actual* new
  native logs and artifacts are inspected.

## Why cgroup v2 addresses this particular weakness

Kernel documentation specifies that a forked child inherits its
parent's cgroup, regardless of later process-session/PGID changes.
`cgroup.kill` in a non-root v2 cgroup signals the entire subtree,
including descendant cgroups, with SIGKILL. It handles concurrent forks
and is protected against migrations within the owned subtree.
`cgroup.events` `populated 0` is the kernel's recursive live-process
emptiness signal. None of these requires a sandbox on the Lu process.

The kernel also requires delegation containment: an unprivileged
delegatee cannot migrate a process outside its delegated hierarchy
merely by writing `cgroup.procs`; common-ancestor write access is
required. This is important because uncontrolled external cgroup
migration would defeat the advertised containment boundary.

Authoritative references:
- https://docs.kernel.org/admin-guide/cgroup-v2.html
  (Organizing Processes, Delegation Containment, cgroup.kill,
  cgroup.events)
- https://systemd.io/CONTROL_GROUP_INTERFACE/
  (delegated service/scope management)
- https://manpages.ubuntu.com/manpages/noble/man2/prctl.2.html
  (parent-death signal is not inherited by forked grandchildren)

## E1-P08C implementation / acceptance contract (OPEN)

1. Detect cgroup-v2 and obtain an *actually delegated*, per-operation
   subgroup through trusted OS/systemd integration. Do not assume any
   unprivileged desktop can write `/sys/fs/cgroup`, use `sudo`, or
   attach to an arbitrary ancestor. The user's normal Lu process must
   stay outside the worker cleanup cgroup.
2. The trusted host allocates the cgroup and verifies its identity,
   ownership, delegation boundary, and `cgroup.kill` interface. No
   model-selected cgroup path, PID, service or arbitrary shell command.
   Workers must be admitted into that subtree before untrusted work
   executes; avoid post-spawn migration races.
3. An **independent lifetime guardian outside the killable worker
   subgroup** must survive abnormal Lu death long enough to issue
   `cgroup.kill=1`. A normal desktop process dying before calling a
   cleanup function is *not* sufficient. A systemd scope/service is an
   option only if unit lifetime and cleanup on host crash are verified,
   not assumed.
4. Require `cgroup.events populated 0` after `cgroup.kill` for each
   *exact, host-owned* worker subtree, within a bounded deadline. Record
   an OS observation receipt. `populated 0` proves no live group
   processes; it does not prove Lu directly `wait()`-reaped every
   adopted zombie. Keep process-state/accounting claims distinct.
5. Native Linux acceptance must SIGKILL the Lu parent while a reviewed
   child forks, double-forks, calls `setsid()`, changes PGID, and
   (where permitted) creates a child cgroup. Verify all are no longer
   alive, and an unrelated control process stays alive. A denied
   delegation must be reported as **NOT AVAILABLE**, not silently
   counted as a passing containment test.
6. When no delegated cgroup exists, do not claim an escape-capable
   worker is fully supervised. Either deny such operations or explicitly
   designate them lower-assurance reviewed/cooperative adapters.
   Never silently downgrade a claimed *all-descendant-kill* guarantee
   to process-group signalling.

## Scope boundary

Current E1-P08 guardian + SIGKILL OS tests remain separate from the
cgroup-v2 acceptance. Cgroup implementation, delegation, installation,
Linux GUI acceptance and arbitrary escape-capable executable admission
are OPEN. The Git/FFprobe reviewed operations can continue under their
documented PGID guarantee, without claiming cgroup-strong containment.
Windows uses Job Object ownership and requires its own exact-head tests.
Physical installed Windows acceptance remains deferred toward v1.0.
