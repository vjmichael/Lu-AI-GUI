# Lu IDE Wave D5 — Worker execution and acceptance evidence

**Status:** Source implementation in branch `ide/wave-d5-production-worker-20261105`. Compile/build CI, actual delegated Linux runtime and installed Windows UI are separate open gates.

### Trusted inputs

The user selects a registered project through the native picker; the host must confirm its current exact Git HEAD and target recipe. `PublishedInput` copies eligible regular files into the user-private application profile with a file-count and 512 MiB ceiling. It rejects symlinks, junctions/reparse points, devices, non-UTF8 names, control characters, and generated output folders. It SHA-256 hashes each file and the sorted paths, records the recipe-specific Cargo manifest digest, and verifies the copy again before native admission. A build cannot rely on a Git HEAD or a Markdown note alone.

**Remaining race gate:** although the final file uses no-follow file opens, directory enumeration and parent path traversal currently use path APIs and are not yet strictly handle-relative under a hostile same-user filesystem attacker. Cap-std/OS handle-relative traversal and a hostile rename/symlink-swap matrix are required before claiming arbitrary untrusted project safety. Private staging has source ownership assumptions; same-account attackers can potentially alter app data.

### Linux real worker path

`linux-worker` Cargo feature (off by default) adds fixed offline Cargo check/test recipes. All of these must pass before execution:

- `/usr/bin/bwrap`, `/usr/bin/cargo`, `/usr/bin/rustc` must be present, root-owned, non-symlink and non-writable by group/others, and their actual bytes must match the plan's toolchain hash; no PATH lookup or shell.
- Current Linux unified cgroup parent must be a canonical delegated subtree with cpu/memory/pids controllers. An exclusive per-job child must accept enforced CPU 50%, plan memory and PID limits, with cgroup.kill available.
- `Command::pre_exec` makes a raw syscall write of the child PID to cgroup.procs and sets no_new_privs. Failure prevents the Bubblewrap image from executing; no unrestricted retry.
- Bubblewrap uses `--unshare-all` (network/user/PID/mount namespaces), empty inherited environment, explicit read-only /workspace, restricted writable /output, private tmp/proc/dev, fixed cargo recipes `--offline --locked`. The app's credentials, HOME and private journal are not mounted.
- Time watchdog and explicit cancellation use `cgroup.kill` followed by `cgroup.events populated 0` proof. Output scanner caps files and aggregate size; RLIMIT_FSIZE constrains individual files; stdout and stderr are independently capped and hashed. Receipts distinguish completed/failed/cancelled/recovery-required/uncertain and never imply an approved human task.

### Crash/recovery

The D3 receipt journal remains fsynced and hash-linked (not cryptographically authenticated against local same-user tampering). In-flight sessions are quarantined on restart, no automatic retries. The Linux worker can reconstruct the per-job cgroup path from its native ID and current kernel scope, issue cgroup.kill, and require empty membership before a cancelled recovery receipt. If cgroup is absent, rights are missing, or cleanup fails, the job remains uncertain.

### Windows

The D2 fixed Win32 Job Object fixture passed real native suspended admission, watchdog termination and descendant kill. The D4 restricted-token fixture is a separate compile/runtime primitive. **No Windows production AppContainer + Job Object worker adapter is implemented by D5; it must not fall back to unrestricted CreateProcess.** Windows OS filesystem/network rights, source snapshot ACLs, quota/kill races and recovery must be verified before enabling Windows execution.

### Hard acceptance gates

1. Exact-head Windows/Linux Rust compile/tests, formatting, source snapshot hashing and fail-closed Linux feature build.
2. Actual delegated cgroup-v2 Linux run of a fixed offline Cargo fixture with denied network/host reads, CPU/memory/PID quotas, bounded logs/output, descendant kill and cancellation injection.
3. Crash at every spawn/admission/journal boundary, orphan reconciliation and durable receipts verified on reboot/restart.
4. Windows AppContainer or equivalently enforced low-rights process launch, Job Object attach-before-resume, denied network/file reads and full process tree recovery.
5. Trusted native-only input snapshot and toolchain identity using race-safe relative handles; signed/broker-protected receipts if hostile same-user users are in scope.
6. Installed Windows desktop UI build/run/restart cancellation acceptance. Do not publish `job.run` until gates pass.

**Do not claim full production completion from source-only CI.**
