# IDE WO4 — Linux delegated cgroup-v2 containment and recovery

**Work-order inventory:** original WO1–WO10; WO1–WO3 Windows/native source and acceptance gates closed on exact Windows head `bd39d01129f8e4a894edaf3e4a1734fb3dbd1867` (D7B `42322328831`, D7D `42322328821`). **Seven** orders remain: WO4–WO10. WO4 is active; WO5 native LSP, WO6 supervised build/test/DAP, WO7 signed license-reviewed NSIS, WO8 installed-app E2E, WO9 advanced packaging/Git/multi-project, WO10 optional authorized Lu self-development are not closed by this work.

**Branch:** `ide/wo4-linux-delegated-containment-20261106` (stacked on the accepted PR #65 source head). **Status:** source hardening started; no delegated-Linux native E2E or release acceptance claimed.

## Why a dedicated delegated parent is required

The existing Linux worker inherited the process's own cgroup from `/proc/self/cgroup` and tried to create worker subgroups there. On cgroup-v2, enabling domain controllers such as `cpu` and `memory` beneath a non-root populated cgroup is disallowed by the no-internal-process rule. Merely reading `cgroup.controllers` does not prove they are *enabled* in `cgroup.subtree_control`. The GitHub hosted E2 Linux run `42322328885` reported no writable delegated cgroup and correctly refused native acceptance. Do not convert the hosted CI refusal into a simulated PASS.

**Provisioning contract (trusted installer/systemd/host launcher; never untrusted code):**

- A dedicated `lu-ide-delegated` cgroup is provisioned under an OS-delegated v2 subtree with `cpu`, `memory`, and `pids` available **and enabled** in its `cgroup.subtree_control`.
- Its `cgroup.procs` must be empty. The approved native IDE host runs inside the existing sibling leaf named `lu-ide-host`; individual worker cgroups are created under `lu-ide-delegated`, *never as descendants of the host leaf*.
- The worker must not elevate, remount cgroupfs, enable controllers, move running host processes, attach to arbitrary caller-provided directories, or fall back to an uncontained process.
- The worker verifies exact quota readback on the newly created job cgroup before consuming a one-shot grant. Kernel `cgroup.events` must prove an initially empty cgroup and a uniquely valid `populated 0` on cleanup; contradictory/missing evidence is denial.
- Any failed deletion leaves a cleanup obligation in the `Cgroup` guard. Cancellation first journals intent, then invokes `cgroup.kill`, and requires kernel-empty proof. Reconciliation never relaunches the interrupted tool.

## First implementation

1. `linux_worker.rs`: select only an explicitly named `lu-ide-delegated/lu-ide-host` topology rooted in the current kernel cgroup identity; check available **and activated** controllers and that the delegated parent has no internal processes. Do not self-provision or silently adopt another user's hierarchy.
2. Require a newly created child to be provably empty before writing and verifying CPU/memory/PID quotas.
3. Replace permissive `cgroup.events` string matching with an exact, unique `populated` record parser; reject contradictions and unreadable evidence.
4. Do not mark an empty-group cleanup obligation complete if `rmdir` fails; preserve the guard's kill/cleanup path.
5. `linux_cgroup_probe.rs`: align the independent admission probe with the actual protected topology instead of giving a misleading direct-child success.

## Required next acceptance gates

- Real delegated native Linux runner with the trusted host running in its `lu-ide-host` leaf, readback of all three enabled controllers, exact worker CPU/memory/PID limits and no arbitrary caller-selected delegation path.
- Fixed native child is joined to `cgroup.procs` **before exec**, with kernel process identity observed independently. Test a descendant tree, enforced limits, and `cgroup.kill` under real load; no source-only or simulated receipt substitutes.
- Bubblewrap namespaces genuinely reject filesystem escapes, network access, inherited credentials, and external executable paths; verify pinned installed tools and offline Cargo outputs, cancellation, journal reopen and interrupted-host reconciliation.
- Crash/kill during real execution leaves no live uncontained descendant; missing or unverifiable kernel membership permanently quarantines the job.
- Hosted CI still runs Rust format/compile/regression gates but remains **not delegated acceptance** if the host cgroup is read-only. Actual delegated acceptance must be executed on a suitable independently provisioned Linux host.
- Live native LSP, installed desktop, NSIS and Lu self-development remain disabled. Do not promote WO4 until those Linux-native kernel checks pass.

**Implementation provenance:** work starts from `bd39d01` without changing the accepted WO2/WO3 branch or claiming an installer, self-hosted runner, or actual native runtime result.


## Next implementation batch: systemd delegation, pre-exec kernel witness

**Trusted unit template:** `ide/supervisor/fixtures/linux/lu-ide-delegated.service.in` is an INSTALLER template, not installed or enabled by this PR. It requests `Delegate=cpu memory pids`, `DelegateSubgroup=lu-ide-host` and `KillMode=control-group`; it pins approved user, group and native executable paths at package construction. This requires a distribution with systemd 254+ and unified cgroup-v2. The installer must verify the fixed helper and native host binary signatures/digests/ownership and must not substitute arbitrary user-supplied executables. No Docker, ambient root, `sudo`, service-manager bypass or Shell fallback is installed by the IDE.

**Trusted preparation:** `ide/sandbox/src/bin/linux_cgroup_prepare.rs` is a fixed-purpose `ExecStartPre` program. It operates only inside `lu-ide-delegated.service` or its kernel `.control`/`lu-ide-host` child. It checks exact cgroup-v2 location, domain type and controller availability, and refuses multiple internal unit-root processes. If invoked from the unit root it migrates only *its own PID* to the fixed host leaf, never arbitrary processes. It enables missing CPU/memory/PID controllers only on the already OS-delegated service's `cgroup.subtree_control`, then requires kernel readback and an empty unit parent. Direct worker execution cannot invoke this helper as a privileged escape.

**Worker and acceptance:** both the Cargo and LSP worker now require an independently read kernel `/proc/<pid>/cgroup` identity matching the newly created worker cgroup and the exact PID in `cgroup.procs` before publishing `Running`; mismatch triggers a best-effort `cgroup.kill` and durable supervisor quarantine. Kernel PID attestation is *in addition to* pre-exec membership and exact limits. The independent `ide/sandbox/src/bin/linux_cgroup_native_witness.rs` launches only two compiled-in child/grandchild modes, opens `cgroup.procs` before spawn, writes `0` from the child in `pre_exec`, observes both PIDs in the kernel group, then requires real recursive `cgroup.kill` and a uniquely parsed `populated 0` before printing PASS. No arbitrary command/path/PID can be requested.

**Evidence discipline:** source formatting/compilation/unit tests are performed on normal CI and are not delegation acceptance. The hosted runner's absent delegated parent remains a controlled denial. The optional `ide-wo4-linux-native-delegation.yml` workflow requires an explicitly enrolled and independently provisioned `lu-ide-delegated` Linux runner; it may not synthesize group files or weaken permissions to make a PASS appear. The service/unit is not activated by this source PR.

**Still required before closing WO4:** actual source-exact native systemd unit provisioning, trusted launch/pinned package tests, a real delegated Linux Cargo test and cancellation, hostile descendant memory/CPU/PID enforcement, simulated host crash with quarantine vs authentic kernel recovery, Bubblewrap namespace denial and audit, and installed-app gates. Do not infer WO5 LSP activation from compiling its membership guard.

References: [systemd delegation](https://github.com/systemd/systemd/blob/main/docs/CGROUP_DELEGATION.md), [systemd.resource-control](https://www.freedesktop.org/software/systemd/man/257/systemd.resource-control.html), and [Linux cgroup v2 kernel documentation](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html).
