# Lu Linux kernel cgroup-v2 positive custody — runner and acceptance

**State: OPEN, environment-gated.** The original C4 [run #43743708785](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43743708785) ran at `177840533611198694436ec39d95b46953510d55`. Its Ubuntu-hosted `linux-real-cgroup` job reached the unchanged native binaries, then printed `AGENT01L_CGROUP_DELEGATION_NOT_AVAILABLE=1 positive_escape_custody=OPEN` and exited **78**. The original full-C4 reconciliation failed, correctly. Separate native Windows/Linux, security/recovery, root-owned Git and pinned real-Qwen jobs passed at that exact SHA, but are not a substitute for the Linux escaped-descendant positive.

## Real blocker — not a failed Rust compile

The legacy AGENT-01L first-party cgroup custodian in `src-tauri/lu-win-worker-job/src/bin/lu-linux-cgroup-guard.rs` creates a per-operation subtree under the process's `/proc/self/cgroup` root. It **must** find a real unified cgroup-v2 mount, writable new leaf with kernel `cgroup.kill`, and a **non-writable, differently owned immediate ancestor `cgroup.procs`**. The non-writable ancestor protects against a compromised same-UID worker migrating outside the leaf before `cgroup.kill`; disabling `protected_ancestor`, using `sudo` from the model, trusting killpg, or treating exit 78 as passing would invalidate the security objective. The test additionally proves Lu parent SIGKILL, a nested `setsid` child, recursive kernel `populated 0`, an unaffected control process, an original scoped durable receipt, and actual native Git restart custody.

GitHub's regular `ubuntu-24.04` runner has cgroup-v2 mounted but does not provide this exact protected delegated execution topology. **Even a generic user-owned `Delegate=yes` service may be UNSAFE**: the same user may write its parent `cgroup.procs`. The secure current production execution path uses the separately root-owned fixed SEC-06C systemd supervisor, which has passed its original real OS Git, cancellation, SIGKILL/recovery, and hostile-ancestor checks. These receipts do **not** automatically pass the original legacy escaped-descendant gate.

## Corrected one-SHA CI routing

- C4 `linux-real-cgroup` is routed to **`[self-hosted, linux, x64, lu-cgroup-v2-delegated]`**, and runs only if the repository variable `LU_CGROUP_POSITIVE_RUNNER_READY` is exactly `true`. When absent, it is **SKIPPED**; the existing all-or-nothing `exact-sha-consolidated-reconciliation` still requires `needs.linux-real-cgroup.result == success` and thus **FAILS**, not green.
- The separate manual `lu-agent01l-linux-cgroup-v2.yml` destructive positive test also routes to that special runner. No change to the crash fixture, native guard, protected production broker, positive evidence matcher, original test expectations, or security negatives.
- Do **not** set the readiness variable or apply this special label until a trusted administrator has configured and verified a real native Linux host with this precise custody capability. Labels are scheduling hints, not kernel proof.
- The planned owner physical validation is separate. **Do not provision a Docker container, ephemeral VM or speculative remote service on the user's behalf.** A Linux kernel environment and administrative OS custody must actually be available before trying the test.

## Administrator acceptance checklist (not proof by itself)

1. On a trusted native Linux machine, inspect `/proc/self/cgroup` and `/sys/fs/cgroup/cgroup.controllers`, and verify `cgroup.kill`, `cgroup.events`, and `cgroup.procs` are supplied by the genuine kernel.
2. Arrange a protected, independently owned Linux custody parent so the worker cannot write its ancestor `cgroup.procs`. A normal same-user `Delegate=yes` setting is **insufficient**. Where needed, use an OS/root-controlled supervisor to migrate a child *before* dropping worker privileges; this may require a separately reviewed update to the **legacy test fixture**. Do not broaden the production tool grant.
3. Prove the exact hostile same-UID migration attempt fails. Verify the independently root-owned `SEC06C` supervisor remains outside its worker leaf and its original `KERNEL_EMPTY=1` witness survives Lu SIGKILL.
4. Register a self-hosted runner with `linux`, `x64` and the dedicated `lu-cgroup-v2-delegated` label **only after** step 2 can be demonstrated. Set `LU_CGROUP_POSITIVE_RUNNER_READY=true` through GitHub repository variables when intentionally enabling the native gate.
5. Run **one immutable-SHA** acceptance: the exact original `benchmarks/e1/agent01l_cgroup_escape_crash_acceptance.py` with pinned first-party guard and host binaries, then the ignored real Git/restart test, and inspect their original logs/artifacts and `AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1`. Anything else remains OPEN.
6. Preserve independent SEC-06F, installed NSIS/Debian, physical Radeon/R9700/Qwen27B/MMProj and human-authorized `main` merge as separate gates.

**No native positive proof was created by this routing correction. No full C4 or release acceptance is claimed.**
