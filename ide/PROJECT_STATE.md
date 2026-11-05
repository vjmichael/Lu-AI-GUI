# Lu IDE — Current project state (authoritative handoff)

**Observed date:** 2026-11-05 (America/Phoenix)  
**Repo:** `vjmichael/Lu-AI-GUI` · **Branch:** `ide/workspace-development`  
**Last inspected implementation source SHA:** `d7fd8cfac1d91a1ddc3073e631a249c1f5b1f9a7`  
**Current source validation:** [Standalone IDE CI 42125275258](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42125275258) queued at last inspection. **No source acceptance established for A2/A3 yet.** Earlier source runs were superseded/cancelled.  
**Local checkout, `Delivered` folder and untracked work:** **UNKNOWN / NOT INSPECTED** in this session. Never assume GitHub is ahead of inaccessible local work.  
**Live Lu↔IDE connection:** **DEFERRED by user**. The IDE remains standalone.

## Current status by capability

| Milestone | Source | Verification | User acceptance |
| --- | --- | --- | --- |
| R1–R3 project registration, explorer, CodeMirror, explicit save | Implemented | Earlier CI and Windows user smoke; see [handoff](ENGINEERING_HANDOFF.md) | Earlier core editor milestones accepted |
| R3.4 multi-hunk review and revision conflicts | Implemented | Earlier native CI accepted; see [R3.4](docs/IDE_07_R34_MULTIHUNK_PROPOSALS.md) | Earlier Windows smoke accepted |
| R3.5 proposal inbox and editor-only staging | Implemented | Earlier native matrix passed; see [R3.5](docs/IDE_07_R35_PROPOSAL_INBOX.md) | User reported working staging flow; full hostile gates open |
| R3.6 success banner, R3.7 Find in File | Implemented | [Source CI 42069988384](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42069988384) passed for R3.7 SHA `eb0aefd` | New GUI acceptance still open |
| A1 saved-file source spans, Copy Source Reference | Implemented | [CI 42075298457](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42075298457) Linux/frontend/Windows passed for SHA `467775e` | New clipboard UX still open |
| **A2 typed Semantic Capability Registry** | **Implemented**: 19 real host operations, JSON Schema v2, self-contained nested definitions, native-argument parity tests, Capabilities UI | **Exact-source CI pending** on latest code | Windows installed UX open |
| **A3 Development Ledger and Context Read Model** | **Initial runtime implemented**: host app-profile JSONL event history, bounded project context and Continuity UI | **Exact-source CI pending** on latest code | Windows installed UX open |
| A3 bounded local artifact and toolchain-candidate inventory | **Implemented in source** | CI on latest source pending | Installed acceptance open |
| A3 persistent snapshot and compare | **Implemented in source** | Native regression fixtures added; CI pending | Installed acceptance open |
| A3 full GitHub, worktree and external toolchain reconciliation | **Not implemented** | Not tested | Open |
| Live Lu connector, worker execution, LSP/build/test/package | **Deferred/not implemented as IDE runtime** | Not tested | Open |

## A2 — Typed contracts actually implemented

- **Source:** `host/src/capabilities.rs`, `host/src/capability_schemas.json`, standalone Tauri `ide_list_capabilities`, React Capabilities screen.
- Host-created, versioned descriptors for **19 existing** project, source, review, save, proposal, continuity and inventory operations. Request/result JSON schemas are typed, with nested structures and self-contained JSON Schema 2020-12 definitions. Response includes typed error schema. **Discovery is not permission, execution or a Lu bridge.**
- UI shows concise operation input/result signatures, with full schema expansion on demand. Rust validation and frontend structural/command parity tests added.

## A3 — Bounded local evidence inventory and saved baselines

- **Implemented in source:** `host/src/inventory.rs` performs a bounded, nonexecuting scan of known folders such as `Delivered`, direct root-level installers, build output, benchmark/result folders, and selected project-local tool/bin directories. Files receive observed byte size, SHA-256 if at most 64 MiB per file and within a 128 MiB total scan budget, category, and explicit `present_unverified` status. No shell, package install, app launch, network request, or ambient filesystem search.
- **Local Git:** reads only `.git/HEAD` and loose local/remote-tracking refs through the project capability. It can report equal/different *locally stored* refs, but does **not** verify a working-tree diff, packed refs, linked Git worktrees or current GitHub HEAD. Those are marked `not_verified`/`not_queried`.
- **Durable user-triggered baseline:** `host/src/inventory_history.rs` preserves a bounded append-only history (up to 64 snapshots total, 1 MiB storage limit) of observed artifact paths, sizes and SHA-256 digests in application-profile `evidence-baselines.json`. Later scans compare with the latest snapshot and classify known files as `matched_bytes`, `changed_bytes`, `missing_in_scanned_scope`, `new_in_scanned_scope` or `unverifiable`. An incomplete scan never establishes missing evidence. Histories persist across restart, but the snapshots prove byte observations, **not executable readiness or benchmark comparability**.
- **UI:** Continuity now shows evidence files and their digests, observed Git refs, a human **Record evidence snapshot** control and baseline comparisons. The capability catalog includes two typed host operations, `inventory.capture` and `inventory.compare`. Native fixture tests and frontend parity tests added.
- **Scope limits:** only registered project roots are scanned (including direct root-level installer candidates so a manually selected `Delivered` folder can be inspected). Toolchains installed elsewhere on the user's machine, older `Delivered` folders outside the chosen root, live remote GitHub, actual dirty/untracked diffs, native installer launch and benchmark reruns remain **not inspected** without separate explicit access and worker authorization.
- **Verification:** source committed, pending native/Windows CI [run 42125275258](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42125275258). No physical Windows acceptance and no new installer claimed.

## A3 — Current, narrowly scoped first slice

- **Source:** `host/src/continuity.rs`, `host/tests/continuity.rs`, standalone Tauri `ide_read_project_context`, React Continuity screen.
- Private app-profile `development-ledger.jsonl` receives **best-effort host observations after successful actions**: project register/open/close/unregister and explicit revision-checked source save. Saved events include **validated relative source path** and saved SHA-256 revision. No project root or absolute local path stored in event.
- The read model requires an **active registered project**, returns at most **40 recent events** and twelve bounded root/`ide/`/benchmark orientation document slots with exact saved revisions and excerpts. Project-authored documents are labeled **untrusted_project_text**.
- Remote Git state: `not_queried`. Local artifacts: `bounded_scan`. Toolchain readiness: `not_verified` or `present_unverified` for scoped candidates. These describe exactly what was and was not inspected, not false missing/ready claims.
- **Limitations:** Observations are NOT crash-safe effect receipts, verified CI results, operational resource probes, Git reconciliation or authoritative build history. Failed ledger appends are logged separately and do not convert a successful file write into a retryable failure. A corrupt ledger fails closed on read. True effect journaling, verified external resources and complete local/remote reconciliation remain open. The bounded artifact inventory and persisted byte snapshots are implemented as separate services.

## Next unblocked tasks (order matters)

1. **Close exact-source CI:** Linux Rust unit tests, `cargo fmt --check`, frontend build/tests and Windows Tauri/native checks for A2/A3; fix actual failures before claiming acceptance. Then test Capabilities and Continuity screens on a Windows build. No new installer is verified.
2. **A3 follow-up:** verify exact-source native CI, fix formatting/build errors, then expand beyond bounded project roots only through explicitly registered locations. Real executable version/OS checks require supervised workers and permission; benchmark comparability and signed build/test receipts remain open.
3. **Project reconciliation:** inspect *real* local worktrees (including dirty/untracked state), accessible out-of-project `Delivered` folders, live remote HEAD/PRs, benchmarks and package hashes. Classify `LOCAL_ONLY`, `REMOTE_ONLY`, `MATCHED`, `DIVERGED` or `UNVERIFIED`; no destructive merges without human consent.
4. **Standalone editor hardening:** Find/Replace and unsaved-buffer revision identities; retain approved proposal staging, explicit Save and host revision checks.
5. **Execution security before LSP/build/test/package workers:** OS containment and user authorization gates remain mandatory; future Lu integration remains deferred.

## Handoff and artifact truth

- [Chronological changelog](CHANGELOG.md), [continuity protocol](CONTINUITY_PROTOCOL.md), [engineering handoff](ENGINEERING_HANDOFF.md), [capability-first architecture](docs/CAPABILITY_FIRST_ARCHITECTURE.md).
- Repository branch source was inspected using the GitHub connector. No local checkout or user Windows filesystem was inspected. **No R3.6–A3 Windows installer or local NSIS package has been verified in this session.**
- Do not infer that a Git commit is a tested build, that a CI success is an installed launch, or that historical resource memory authorizes execution.
