# Wave D — Execution boundary and D1 authorization preflight

**D1 branch:** `ide/wave-d1-execution-intent-20261105` based on C6 source SHA `c403a8ba47d6ac1662b8627ce5ab34a9a16b591e`. Source work in D1 does **not** close Wave D's actual worker containment or execution acceptance gates.

## Why two distinct phases?

The standalone IDE was previously read-only except for explicit editor saves, task notes, snapshots and catalog writes. Enabling arbitrary build scripts from source folders without a containment gate would violate the capability-first design: source text and user-facing buttons cannot grant authority. Wave D starts by establishing a trusted native preparation, human review and durable evidence protocol *before* introducing actual processes.

A human can now prepare a **fixed** known recipe in the Job requests UI:
- `rust_check`: `cargo check --locked --manifest-path Cargo.toml`;
- `rust_tests`: `cargo test --locked --manifest-path Cargo.toml`;
- `lu_ide_host_tests`: `cargo test --locked --manifest-path ide/host/Cargo.toml`.

These are **display-only proposed actions**. No process, shell, package manager, compiler, network request or arbitrary arguments are invoked by D1. Recipes cannot come from untrusted project files or AI tool descriptions.

## D1 native contract

The only frontend-supplied fields are `projectId`, a small `recipeId` enum, an opaque intent ID, and a decision (`record_review` or `cancel`). All other values are supplied/validated by the trusted native host.

1. **Prepare:** verify active registered project canonical root; require local inspected Git HEAD to be exactly 40 hex characters, a clean Git worktree without tracked/staged/untracked changes; inspect an allowlisted fixed Cargo manifest with non-symlink/reparse components, <=1 MiB and SHA-256. Persist an opaque random ID with source HEAD, manifest SHA and 120-second expiry, bound to the current desktop process session.
2. **Review:** human selects the exact prepared item and acknowledges no execution, host compares project ID, session, expiry, same source HEAD/clean worktree and manifest SHA again. Append `review_recorded_no_execution` once only; no reusable grant, bearer token, OS execution capability, serialized command or worker handle is returned.
3. **Cancel:** append `cancelled` before review or `review_withdrawn` after review; no child process exists.
4. **Read/restart:** bounded `execution-intents.jsonl` in native app data, journal mutex, synced append, versioned replay, validated monotonic transitions, explicit corruption handling. Restart creates a new session ID, rendering old unreviewed intents as requiring fresh preparation. Bounded max 128 history events, 16 intents per project and 256 KiB journal; full journals fail explicitly instead of evicting evidence.

D1 responses always state `execution=never_started`, `executionAuthority=not_issued`, `workerContainment=not_implemented`, `grantTokenIssued=false`. No project-local execution journal or Lu bridge is added.

## Security limits

A Git HEAD, even with a clean libgit2 status, **is not** a full immutable filesystem snapshot (ignored files, unsaved buffers and external dependencies are not certified). D1's intent is informational; any actual D2 runner must independently revalidate all preconditions and capture its *real* tested inputs. Existing user-writable app-profile journal paths are not resistant to an adversary with local account filesystem write privileges; **never** interpret unsigned journal content as privilege-bearing authorization. An approved request or discovered compiler is not executable authority.

No untrusted process is launched, so D1 has **not** demonstrated Windows Job Object / Linux cgroup containment, cancellation, crash recovery, resource budgeting or OS permission mediation.

## D2 acceptance design

1. Native dedicated supervisor with a **new non-replayable per-job grant** created only after explicit human approval, permission/effect classification and refreshed source snapshot. Never use D1 historical review as a grant.
2. OS-specific containment: Windows Job Objects and Linux cgroup v2 (or a verified equivalent), fail closed if unavailable. Spawn suspended/contained before untrusted code executes; restrict process descendants and resources. No fallback to unrestricted host execution.
3. Bound command identity and working directory from approved typed recipes, not shell interpretation. Close inherited handles and environment credentials, honor cancellation and full process-tree kill, enforce CPU/memory/pid/runtime budgets, cap logs and artifacts. Expose durable prepared/running/cancelled/failed/uncertain/verified receipts, including exact input snapshot provenance.
4. Hostile native test fixtures: escaping process trees, reparse/symlink path swaps, timeouts, cancellation races, restart during execution, failed journal publication, orphan cleanup, and recovery of uncertain jobs. Require actual Linux and Windows native tests before enabling any user-visible Run button.
5. Wave E then adds LSP/build/test/debug atop those verified primitives; don't introduce application-level exec shortcuts.

## Current acceptance status

D1 source is implemented and subject to CI. GitHub native matrix and installed-app behavior must be independently checked at exact head. Wave D is not complete until D2's real OS worker/containment gates pass on both intended platforms.

## D2-W1 — Independent Windows Job Object acceptance fixture

Source lives under the *existing*, unconnected `ide/sandbox` experiment, in `src/bin/windows_probe.rs`. It is not linked into the IDE host or WebView, does not consume D1 historical reviews, and cannot run user or AI selected projects. The only child modes are compiled-in self-executing fixtures. The Windows native Job Object path uses `CreateProcessW(CREATE_SUSPENDED)`, configures `KILL_ON_JOB_CLOSE`, a job memory limit and process cap, assigns the child, then resumes it, avoiding pre-containment execution. The fixed tests also terminate a deadline-exceeding child and its spawned grandchild. The Windows workflow separately tests these OS primitives.

**Scope warning:** Job Objects manage CPU/resource/process-tree effects; they are not a Windows restricted token, AppContainer, filesystem jail, network broker or full hostile-code containment. Independent worker activation requires additional OS security boundaries and crash/kill race proofs. A successful fixed-fixture CI is not installed-app acceptance nor proof of arbitrary build/test script safety.

The Linux legacy Bubblewrap native probe crate is reused rather than rewritten; its previous 5/5 kernel checks do not prove process quotas or full descendant cleanup. Both platforms must meet all D2 gates before any future `job.run` capability is advertised.

## D3 — Durable execution control plane (source implementation)

The `ide/supervisor` crate introduces a bounded, host-only plan and approval
model and durable hash-linked execution receipts. It is not wired into Tauri.
A fresh 60-second single-job grant is kept only in volatile supervisor state,
bound to the original plan hash and observed source. Review documents and
older D1 records cannot become grants. The only production admission entry
returns `SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED` on every OS until the broker
has independently demonstrated all required rights/quotas/cleanup checks.

The journal records immutable transitions, including cancellation **before**
OS termination and uncertain/crash-recovery on incomplete cleanup. On restart
in-flight jobs are quarantined, and a trusted OS tree-termination broker must
prove cleanup before they are reconciled. No auto rerun is permitted. The
SHA-256 chain only detects accidental or naive tampering; it is **not signed**
and does not protect against a local account with write access to the entire
app-data journal. A secure protected broker/publisher is still needed.

Native Rust tests use simulated brokers to verify the state machine. Real
Windows Job Object and Linux Bubblewrap OS probes are separate earlier
evidence; neither alone demonstrates production restricted rights or real
worker-execution safety. Future platform worker modules require Windows
restricted identity and filesystem/network policy, Linux cgroup-v2 resource
admission and namespace isolation, race-safe source/toolchain snapshot
publication, native process-tree kill and crash orphans, capped output,
hostile fixtures and physical installed desktop acceptance.

## D4 — Restricted rights and delegated quota admission primitives

Fixed, separately packaged native OS probes now complement D2 Job Object and
older Linux Bubblewrap tests. On Windows, the probe creates a SID-restricted
token with privileges removed and verifies `IsTokenRestricted`. It never
launches a child. On Linux, the probe checks delegation and configures
CPU/memory/PID quotas on an EMPTY cgroup-v2 child, then deletes it; denied
delegation is explicitly recorded, with no fallback. See
`ide/sandbox/OS_RIGHTS_ADMISSION.md`.

**This is not worker authorization.** Windows restricted token creation plus
Job Objects still lacks verified low-integrity child execution and filesystem
and network isolation. Linux cgroup creation plus Bubblewrap still lacks
pre-exec attached worker/descendant tests, security broker and crash recovery.
The D3 production admission gate remains deny-only until full OS native
acceptance and trusted source/output publication are proven.

## D5 — First native worker implementation (feature-gated Linux)

A staged immutable-input publisher now copies source bytes into a private
app-owned directory with bounded file counts/size, regular-file-only
checks and a deterministic tree/manifest seal. The worker rechecks the seal
before any process admission.

The opt-in Linux native runner joins a delegated quota-controlled cgroup
in `pre_exec`, before the trusted Bubblewrap launcher executes, then uses
isolated user/PID/mount/network namespaces with the source snapshot mounted
read-only and a bounded private output directory mounted writable. Only
allowlisted offline Cargo check/test recipes are encoded. A fresh native
one-shot grant is consumed atomically in the supervisor journal before
spawning; a new OS tree-termination verifier uses `cgroup.kill` and checks
empty membership. Capped output/log checks and a timeout/cancel watchdog
return concrete receipts. Interrupted executions remain quarantined until
explicit cgroup orphan reconciliation.

This is **not yet a complete user-visible production worker**: Linux native
hostile running tests, race-safe source-handle traversal, Windows AppContainer
or equivalent restricted worker process and installed Windows UI acceptance
are open. There is no `job.run` Tauri handler. Standard Linux CI without
delegated cgroup rights must fail closed. See
`ide/supervisor/WORKER_ACCEPTANCE.md`.
