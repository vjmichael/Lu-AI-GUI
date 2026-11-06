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
