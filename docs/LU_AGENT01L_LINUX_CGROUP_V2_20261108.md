# AGENT-01L Linux cgroup-v2 custody — crash-surviving source candidate

**Repository stack:** `lu/agent01l-linux-cgroup-v2-20261108` on AGENT-01L Windows Job Object PR #173. This is a native worker-custody layer, NOT a sandbox for Lu Core and NOT an automatic model continuation feature.

## What is implemented

1. A first-party `lu-linux-cgroup-guard` executable runs as a **separate child of Lu, outside the worker's dedicated cgroup**. It derives its cgroup-v2 parent solely from its own kernel `/proc/self/cgroup` membership, not a model-supplied path, PID, or service name. It creates a random private `lu-review-<nonce>` subtree using only ordinary delegated permissions. If the host session lacks delegation or a real cgroup-v2 interface, it returns `UNAVAILABLE`; it never uses sudo, pkexec or root escalation.
2. The reviewed native supervisor validates the original persisted `lu_reviewed_turn_origin` chat/user turn, starts the custodian only for a scoped Git or FFprobe operation, and waits for its exact `READY\t<kernel-path>` handshake. The helper binary is a trusted installed sibling (or a debug-only explicitly pinned test override), executable, regular, non-symlinked, and not writable by other users/groups.
3. The worker joins that exact newly created subtree **inside its `pre_exec` before any Git/FFprobe code can execute**, by writing the kernel's current-process marker `0` to its cgroup's `cgroup.procs`. Failed migration refuses exec. The existing `PR_SET_PDEATHSIG` additionally protects its immediate worker if Lu dies during admission.
4. The custodian holds a unique stdin lifetime pipe. If Lu closes it normally or is `SIGKILL`ed, the first-party custodian sees EOF and writes `1` to its privately owned subtree's `cgroup.kill`. This signal covers children which have called `setsid()` and descendant cgroups. The custodian polls recursive `cgroup.events` and will **NOT** publish success until `populated 0` is observed within a bounded deadline.
5. Only after that kernel observation does the custodian create a **new** 0600, fsynced, per-operation historical receipt `lu-cgroup-os-<human_turn>-<reviewed_call>.receipt`. It records exact chat/turn/call, an independently generated nonce and the kernel `POPULATED=0` verdict. Existing receipt names cannot be overwritten or replayed. On ordinary successful worker completion, Lu waits for this acknowledgement before marking the task terminal. On abnormal Lu exit, the independently running custodian can still record the kernel observation while the old task/effect journal remains eligible for separate recovery handling.
6. AGENT-01K's original-turn custody reader is wired on Linux: it independently reopens the authenticated original TaskStore SQLite and the scoped receipt, refuses symlinks, unsafe permissions, malformed receipts, mismatch of conversation/turn/call, an uncompleted task or missing journal. Its positive historical observation **cannot** by itself authorize a tool. AGENT-01K still separately requires independently verified original effect/oracle evidence, current ANSF sources, released host owner and verified ancestor custody. Native child-plan admission checks remain in force before preparation, resource picker and execution claim.

## Actual Linux OS acceptance gates

- `src-tauri/lu-win-worker-job/src/bin/lu-e1-p08-linux-cgroup-crash-parent.rs` (test-only Cargo feature) creates the custodian and a reviewed-process fixture whose grandchild deliberately calls `setsid()`; it records their original kernel identities.
- `benchmarks/e1/agent01l_cgroup_escape_crash_acceptance.py` pins the escaped grandchild with `pidfd`, independently confirms the different process groups but equal cgroup membership, `SIGKILL`s the Lu parent so Rust Drop cannot run, then requires the independently produced kernel populated-zero receipt and the escaped grandchild to stop. An unrelated control process must remain alive. The script emits `AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1` **only after actual kernel checks**. If cgroup delegation is unavailable, it prints `AGENT01L_CGROUP_DELEGATION_NOT_AVAILABLE=1` and exits 78; this is NOT a pass.
- A separate ignored native Lu Core test `agent01l_linux_real_git_cgroup_custody_after_restart` exercises actual reviewed Git HEAD, original TaskStore completion, stdout validation, independent kernel cgroup receipt after reopening, and rejection of cross-chat/turn/call substitution.
- `.github/workflows/lu-agent01l-linux-cgroup-v2.yml` supplies exact-head Ubuntu/Windows compile/source-contract checks, the existing kernel `setsid()` negative control, and a **hard-failing** delegated-cgroup positive acceptance job with artifacts. No CI skip or green result is allowed to masquerade as a positive kernel test.

## Observed environment evidence (November 8, 2026)

A directly executed local-container Linux negative control confirmed that a `setsid()` child survived a process-group SIGKILL while sharing its original cgroup: `LOCAL_LINUX_SETSID_NEGATIVE_CONTROL_PASS=1`. The same environment's cgroup mount was **not writable**, so it cannot attest successful delegated cgroup cleanup. That observation is separate from Lu's native CI tests.

## Limitations and next work

- **OS delegation:** standard Linux desktops and hosted CI do not necessarily delegate writable cgroup-v2 subtrees. The helper detects this. Cooperative reviewed Git/FFprobe may use the existing process-group guardian at lower assurance, but the AGENT-01K/01L child continuation guard **cannot accept** that as full-descendant custody. There is no silent downgrade of the claimed assurance.
- **Systemd lifetime:** this version survives an isolated Lu process SIGKILL by relying on an independent first-party helper and its pipe. It is **not yet proven** to survive systemd stopping an entire Lu application service/scope with `KillMode=control-group`, session logout, host power loss, or externally killing the helper itself. A separately supervised systemd user unit outside the application scope and a verified recovery path for its independently completed receipts are required before claiming those broader guarantees.
- **OS semantics:** `cgroup.events populated 0` attests no living descendants in the managed subtree at that kernel observation. It does not prove that Lu itself `wait()`-reaped zombies adopted by init, nor that no independently migrated process escaped outside a delegation boundary through privileged OS mechanisms. The workload is still scoped by Lu's reviewed-app broker.
- **Persistence:** an authenticated historical kernel receipt in a user-owned profile is not a cryptographic attestation against malicious same-user offline database/receipt tampering. As elsewhere in Lu, model-provided output is not accepted as an execution grant; AGENT-01K independently reopens the task, effects and current sources.
- **Acceptance OPEN:** actual exact-head Rust/desktop native compilation, delegated-cgroup kernel crash test, real reviewed Git positive test and original installed Linux UI workflow. Further Windows native Job Object tests and physical Windows NSIS/GGUF testing remain separate.
- **No automatic multi-step execution.** The only possible successful reconciliation outcome is that a user may be offered a NEW independently reviewed human-authorized step, never a replay of an old owner.

## Physical Windows acceptance

The existing Windows NSIS workflow `.github/workflows/lu-agent01g-physical-windows-nsis.yml` builds pinned NSIS 3.11 + current Lu + a genuine CPU llama.cpp engine and runs an isolated Windows **hosted-VM install/uninstall** cycle, then uploads an unsigned installer with SHA-256 when all gates pass. Hosted-VM installation is not the user's AMD Vulkan physical-PC acceptance. The actual NSIS artifact is not yet available until GitHub jobs complete; do not invent a download URL. A user-run physical-PC smoke test must verify GGUF+MMProj loading, one-step native Git, two fresh separately authorized Qwen steps, restart/no replay and no residual child processes.

## Implementation ledger

- Linux native custodian: source candidate committed.
- Worker pre-exec admission + native host profile binding: source candidate committed.
- Independent crash receipt, original SQLite reader and admission fence: source candidate committed.
- CI kernel escape positive acceptance: submitted, result not yet observed.
- Windows Job Object exact-head kernel tests and installed Windows NSIS: pending GitHub execution; no current pass.
- Production crash-surviving systemd service scope and full installed Linux acceptance: open.
