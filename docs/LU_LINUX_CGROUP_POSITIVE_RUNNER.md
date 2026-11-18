# Lu Linux kernel cgroup-v2 positive custody — runner and acceptance

**State: OPEN, environment-gated.** The original C4 [run #43743708785](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43743708785) ran at `177840533611198694436ec39d95b46953510d55`. Its Ubuntu-hosted `linux-real-cgroup` job reached the unchanged native binaries, then printed `AGENT01L_CGROUP_DELEGATION_NOT_AVAILABLE=1 positive_escape_custody=OPEN` and exited **78**. The original full-C4 reconciliation failed, correctly. Separate native Windows/Linux, security/recovery, root-owned Git and pinned real-Qwen jobs passed at that exact SHA, but are not a substitute for the Linux escaped-descendant positive.

## Real blocker — not a failed Rust compile

The legacy AGENT-01L first-party cgroup custodian in `src-tauri/lu-win-worker-job/src/bin/lu-linux-cgroup-guard.rs` creates a per-operation subtree under the process's `/proc/self/cgroup` root. It **must** find a real unified cgroup-v2 mount, writable new leaf with kernel `cgroup.kill`, and a **non-writable, differently owned immediate ancestor `cgroup.procs`**. The non-writable ancestor protects against a compromised same-UID worker migrating outside the leaf before `cgroup.kill`; disabling `protected_ancestor`, using `sudo` from the model, trusting killpg, or treating exit 78 as passing would invalidate the security objective. The test additionally proves Lu parent SIGKILL, a nested `setsid` child, recursive kernel `populated 0`, an unaffected control process, an original scoped durable receipt, and actual native Git restart custody.

GitHub's regular `ubuntu-24.04` runner has cgroup-v2 mounted but does not provide this exact protected delegated execution topology. **Even a generic user-owned `Delegate=yes` service may be UNSAFE**: the same user may write its parent `cgroup.procs`. The secure current production execution path uses the separately root-owned fixed SEC-06C systemd supervisor, which has passed its original real OS Git, cancellation, SIGKILL/recovery, and hostile-ancestor checks. These receipts do **not** automatically pass the original legacy escaped-descendant gate.

### Additional source-level admission conflict — a runner label cannot repair this

The original legacy Rust fixture calls `unix_owned::bind_worker_to_cgroup_before_exec` from a **non-root** child after `LinuxCgroupCustodian::spawn_if_delegated` has checked `protected_ancestor`. The child then tries to write `0` into the new leaf's `cgroup.procs`. Linux cgroup-v2 migration permission requires write access to the destination **and the nearest common ancestor** of source and destination. Since the child was spawned in the protected parent, the protected parent itself is the common ancestor. Therefore **the original child migration cannot succeed if that parent is genuinely non-writable to the same UID**, even when the directory permits child creation. Granting that UID write access to the parent would defeat the hostile-escape protection.

This was a **source integration requirement**, not merely missing host configuration. A narrowly fixed **root-owned SEC-06C service** must create and admit the worker into the killable leaf *before* it drops privileges or allows untrusted instructions. Its independent custodian must remain outside the leaf; the unprivileged worker must be unable to migrate to any ancestor, and the original PID/cgroup membership must be verified after launch. An independently owned service cannot simply be simulated by a shell command, a source marker, or a GitHub runner label. The test-only root-mediated adapter described below implements this separation in source, but independent native kernel verification is still required before acceptance.

**Read-only diagnostic:** From the actual native Linux runner account, invoke `python3 scripts/lu-agent01l-native-cgroup-preflight.py`. It never creates, kills, remounts or migrates a process; it reports the real kernel/mount/UID evidence. It **always returns nonzero** because it inspects the old nonroot topology rather than the separately protected root service. `SOURCE_REQUIRES_ROOT_MEDIATED_ADMISSION` means that the authentic protected ancestor exists but the current non-root source migration is still incompatible. **Do not set `LU_CGROUP_POSITIVE_RUNNER_READY` on the strength of this diagnostic.**

## Implemented native SEC-06C fixture admission (source candidate, not OS accepted)

The test-only fixed-source integration now has three pieces on the existing PR #235:

- `scripts/lu-sec06c-agent01l-fixture-launcher.py`: original nonroot crash-parent invokes **only this fixed, root-owned sudo entrypoint**, which checks the original runner UID, a **root-owned manifest of the exact compiled crash-parent SHA-256**, and the private original test profile before asking `systemd-run` for one root-owned `lu-sec06c-fixture-<nonce>.service` with `Delegate=yes`. It never accepts a worker executable, shell, cgroup path or signal target.
- `scripts/lu-sec06c-agent01l-fixture-manager.py`: executes as a genuine root-owned service; imports the *existing* SEC-06C `preexec_leaf`, root-owned service/cgroup verification, original same-UID hostile-ancestor probe and kernel populated-zero checks. It creates one root-owned leaf plus `lu-nested-host-test`, forks the fixed nonroot leader and actual `setsid` descendant **inside the nested leaf**, and then root-migrates only the known leader into the parent leaf. This guarantees real genealogical descent and separate process groups without giving the unprivileged worker the ability to write ancestor `cgroup.procs`. After the original Lu host's PIDFD reports actual exit, the independent manager invokes real recursive `cgroup.kill`, verifies `populated 0`, removes nested/leaf groups and creates an immutable root-owned, original-scope receipt. No old user-written fixture receipt counts as production custody.
- `src-tauri/lu-win-worker-job/src/bin/lu-e1-p08-linux-cgroup-crash-parent.rs`: this **test-only binary** now uses the fixed installed root-owned broker first; when `LU_AGENT01L_REQUIRE_OS_OWNED_FIXTURE` is set and the fixed broker is absent, it emits `NOT_AVAILABLE` instead of falling back to the incompatible nonroot migration. The legacy fixture path remains for historical/negative comparisons, not a positive kernel acceptance substitute.

**Preserved unchanged:** `benchmarks/e1/agent01l_cgroup_escape_crash_acceptance.py` (original destructive positive Python test), `src-tauri/lu-core/src/native_background_process.rs` (original native Git/restart test), existing SEC-06C production manager/worker/launcher code, the Windows Job-object and all hostile-source assertions. The test-only crash-parent **implementation has changed**; the original test programs and their required results have not.

**Installation (administrator action, not performed here):** On a trusted *native* Linux runner checked out at one exact, frozen SHA, its administrator may explicitly run:

```bash
# Build the CI-only original parent at the exact checked-out candidate SHA first.
cargo build --locked --manifest-path src-tauri/lu-win-worker-job/Cargo.toml \
  --features e1-p08-crash-fixture \
  --bin lu-linux-cgroup-guard --bin lu-e1-p08-linux-cgroup-crash-parent

# Then, deliberately provision from an administrator-controlled shell:
sudo bash scripts/install-lu-agent01l-native-fixture.sh <40-character-source-SHA> <nonroot-runner-account>
```

The installer pins each first-party script's exact Git blob **and the actual built original Rust test parent SHA-256 in a root-owned manifest**, owns the finite launcher/manager as ROOT, and grants only those two audited fixed launchers to the one Linux runner account. It never authorizes arbitrary `sudo`, arbitrary `systemd-run`, a shell, model tasks or network execution. **Review the sudoers diff and source implementation before installation.** Do not run this from an app worker, inside Docker/VM, or as a substitute for physical owner authorization. After installation, verify exact installed source identity, root ownership, authentic systemd unit delegation, same-UID ancestor-escape denial, original real OS SIGKILL/nested/cgroup events and root-owned receipt. A source check, successful installation or successful preflight alone is NOT positive kernel acceptance.

The read-only `scripts/lu-agent01l-native-cgroup-preflight.py` remains a diagnostic for the **old** nonroot migration topology. It intentionally returns a nonpass result even when the new independent root service might be correctly installed; it does not configure or authenticate the root service. Only the unchanged original destructive positive and native Git/restart tests can close the positive gate.

## Corrected one-SHA CI routing

- C4 `linux-real-cgroup` is routed to **`[self-hosted, linux, x64, lu-cgroup-v2-delegated]`**, and runs only if the repository variable `LU_CGROUP_POSITIVE_RUNNER_READY` is exactly `true`. When absent, it is **SKIPPED**; the existing all-or-nothing `exact-sha-consolidated-reconciliation` still requires `needs.linux-real-cgroup.result == success` and thus **FAILS**, not green.
- The separate manual `lu-agent01l-linux-cgroup-v2.yml` destructive positive test also routes to that special runner. The CI-only crash-parent adapter now calls a fixed root-owned SEC-06C service; the original destructive Python and native Git/restart **tests** (including their positive result requirements) are unchanged. The native guard and protected production broker retain their original assertions.
- Do **not** set the readiness variable or apply this special label until a trusted administrator has configured and verified a real native Linux host with this precise custody capability. Labels are scheduling hints, not kernel proof.
- The planned owner physical validation is separate. **Do not provision a Docker container, ephemeral VM or speculative remote service on the user's behalf.** A Linux kernel environment and administrative OS custody must actually be available before trying the test.

## Administrator acceptance checklist (not proof by itself)

1. On a trusted native Linux machine, inspect `/proc/self/cgroup` and `/sys/fs/cgroup/cgroup.controllers`, and verify `cgroup.kill`, `cgroup.events`, and `cgroup.procs` are supplied by the genuine kernel.
2. Arrange a protected, independently owned Linux custody parent so the worker cannot write its ancestor `cgroup.procs`. A normal same-user `Delegate=yes` setting is **insufficient**. Where needed, use an OS/root-controlled supervisor to migrate a child *before* dropping worker privileges; the test-only legacy crash-parent now uses the fixed SEC-06C root-owned admission service when installed, but this still requires positive native OS evidence. Do not broaden the production tool grant.
3. Prove the exact hostile same-UID migration attempt fails. Verify the independently root-owned `SEC06C` supervisor remains outside its worker leaf and its original `KERNEL_EMPTY=1` witness survives Lu SIGKILL.
4. Register a self-hosted runner with `linux`, `x64` and the dedicated `lu-cgroup-v2-delegated` label **only after** a root-mediated admission implementation has been independently reviewed and the *actual native machine* has demonstrated it. Neither a label nor a successful read-only preflight counts as kernel acceptance. Set `LU_CGROUP_POSITIVE_RUNNER_READY=true` through GitHub repository variables when intentionally enabling the native gate.
5. Run **one immutable-SHA** acceptance: the exact original `benchmarks/e1/agent01l_cgroup_escape_crash_acceptance.py` with pinned first-party guard and host binaries, then the ignored real Git/restart test, and inspect their original logs/artifacts and `AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1`. Anything else remains OPEN.
6. Preserve independent SEC-06F, installed NSIS/Debian, physical Radeon/R9700/Qwen27B/MMProj and human-authorized `main` merge as separate gates.

**This integration is staged at source level only. No native positive proof, full C4, installed application, or release acceptance is claimed.**
