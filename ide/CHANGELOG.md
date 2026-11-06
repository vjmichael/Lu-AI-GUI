# Lu IDE — Development changelog

## 2026-11-05 — Wave D1 execution intent protocol (no worker launch)
- Added a separate, bounded, append-only host execution-intent journal and 3 typed Tauri capabilities (`job.prepare`, `job.review`, `job.list`). D1 deliberately records **no execution authority**. Source Git HEAD, worktree cleanliness, fixed recipe and manifest SHA-256 are observed by native code, not submitted by the frontend.
- Short-lived 120-second prepared plans and human review are single-session, with native revalidation; stale, edited, expired, cross-project, replayed and corrupt-record attempts fail closed. A previous review can be withdrawn without dispatch.
- Enabled a human-facing **Job requests** screen with exact source/manifest evidence and cancellation. Explicitly states no executable process, script, model, package install or build/test worker can run.
- Updated capability catalog to 30 operations / 38 schema definitions, existing static native/UI contract tests, and new Rust persistence/replay tests plus frontend UI/schema regression tests.
- **Unimplemented by design:** OS process containment, process-tree cancellation, resource quotas, execution authority, launched worker receipts, crash-recovery reconciliation and installed Windows acceptance. D2 shall use a separate host-owned supervisor; D1 consent notes must NEVER be accepted as executable tokens.
- Branch stacks on C6 accepted native CI source `c403a8ba47d6ac1662b8627ce5ab34a9a16b591e`; D1 exact-head CI pending.

## 2026-11-05 — Wave C6 task evidence and development ledger (source implementation)
- Extended the existing private append-only `DevelopmentLedger` with typed `TaskEvent` payloads and host-backed project task summaries. Human progress edits do not rewrite or claim verified acceptance; they keep source HEAD/dirty state and required gate IDs.
- Added `ide_record_task_update` and `ide_observe_task_ci`, both typed in the semantic capability catalog (27 native commands, 36 definitions). CI is independently fetched by the native host, matched to the current human task event and source SHA, then logged. Stale updates and duplicate/unscoped task IDs fail closed.
- Added Continuity task editor, required gate toggles, task evidence list, link/test action and privacy-sanitized task checkpoint coordinates. No standalone worker, Git mutation, installer claim or Lu bridge.
- New `host/src/task_evidence.rs` Rust tests and `app/tests/task_ledger.test.mjs` Node tests cover task/non-acceptance separation, malformed states, cross-project reads and stale CI links. Exact-head CI acceptance and installed Windows interaction pending.
- **Dependency repair:** C5 source failed only Linux rustfmt after its native tests passed; GitHub formatter diff applied to C5 at `1dd4a58d84d6bc5f383f75e463b15f82baea6de8`. C6 branch starts from that repaired C5 commit. Its own formatting gate remains to be validated.

## 2026-11-05 — Wave C5 exact-source GitHub Actions receipts
- Added native read-only `git.ciExactHead` behind a human-triggered Source control action. It derives only validated public github.com origin, queries bounded fixed api.github.com endpoints for the **exact local HEAD SHA**, checks workflow file paths and required job metadata, rejects partial results and duplicate/missing/other-head jobs, and distinguishes success/failure/running/no evidence.
- Added typed JSON Schema v2 descriptor (25 genuine host operations, 31 nested types) and Tauri command. No credentials, arbitrary HTTP targets, background network access, project mutation, subprocess execution or Lu bridge.
- C4 evidence matrix now uses an explicitly cached same-project, same-SHA receipt if available. C3 checkpoint export copies only a sanitized cached summary; no external query on Copy. No test run proves local dirty draft, installed desktop, or task acceptance.
- Added `host/src/github_ci.rs` classifier unit tests and `app/tests/github_ci.test.mjs` source/schema/reconciliation regression. Exact-head CI and installed Windows acceptance pending. C5 branch stacked on C4 source `ca606dfb378ce31945a768a23e9ab6a31cbe60e8`.

## 2026-11-05 — C4 frontend registry CI repair
- C3 CI found a macro matcher regression: rustfmt moved `capability!` arguments across lines, causing an empty match set despite 24 registered operations. Fixed the JS test regex and added an explicit 24-operation assertion. Native registry/Tauri parity inspected; no host behavior or permissions changed.
- C4 exact-head CI remains pending following this source update. C3 fix also committed independently.

## 2026-11-05 — Wave C4 evidence reconciliation (source implementation)
- Added a read-only evidence matrix to Continuity. Uses existing native host results to classify local Git cleanliness, stored tracking refs, live remote (not queried), bounded artifact hashes/baselines, host events, project-note task claims, CI, installed desktop acceptance and toolchain usability **without promoting guesses to verified status**.
- Explicitly preserves partial scans, truncated change lists, contradictory dirty counts, absent baselines and unknown remote state. The development checkpoint JSON now includes the same bounded conservative reconciliation, but excludes untrusted source/document excerpts.
- Added `app/web/development_reconciliation.js` and `app/tests/development_reconciliation.test.mjs`; source-only, no new host command, Lu bridge, Git mutation, package action or effect grant.
- C4 branch is independently created from Wave C3 exact SHA `2572fa445d31d265b0ef27ef52a1dd4a09156326` to avoid cancelling C3 acceptance CI. Full C4 CI/Windows installed acceptance remains open; local checkout is not inspected.

## 2026-11-05 — Wave C3 bounded development checkpoint export
- Implemented human-initiated **Copy development checkpoint**, with fresh read-only host context + inventory comparison, strict project-ID matching, stale-request invalidation, versioned bounded JSON and explicit non-atomic/provenance limits.
- Exports local Git state, source revision IDs, known artifact bytes/digests, baseline transitions and host event IDs, **not** source text, untrusted document excerpts, event summaries, absolute project roots, passwords, live GitHub contents or execution authority.
- Added `development_checkpoint.js` and Node tests for missing/mismatched project evidence, bounded output, private content exclusion and UI action wiring; local pure tests passed, integration CI and Windows installed acceptance pending. Project-switch invalidation also clears busy-state flags so an older in-flight read cannot leave the new project permanently disabled.
- Isolated Wave C PR #47; predecessor HEAD `26040e8331f5a5b8053f4af8248a88d9d551f04f`. Do not call clipboard/installer accepted before exact-head checks and installed UI acceptance.

This records **what changed**. Read [PROJECT_STATE.md](PROJECT_STATE.md) for what is authoritative *now*, and [CONTINUITY_PROTOCOL.md](CONTINUITY_PROTOCOL.md) before any new development session.

## 2026-11-05 — Wave C first native CI repair (pending recheck)
- First source matrix [42143131954](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42143131954) confirmed frontend success but failed Linux Rust E0308 in `resource_probe.rs`: `cap_std::time::SystemTime` differs from `std::time::SystemTime`.
- Fixed explicitly with `into_std()` before Unix timestamp conversion; implementation SHA `dbaa04ff41a6c1c20367f88f6c2455af47843370`. New exact-head [CI 42143391542](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42143391542) pending; not yet accepted.
- No runtime execution or Lu connection enabled.

## 2026-11-05 — Wave C read-only development truth (source implemented)
- Added scoped native Git staged/unstaged patch and local commit history with typed `git.diff`, `git.history`, explicit Source control UI, 64 KiB/80-file patch cap and 30-commit history cap. Untracked file contents and any remote fetch remain excluded.
- Added `resource.probe` under the registered project directory capability, producing a SHA-256 of inspected bytes or an explicit missing, changed, non-file, or oversize state. No claim that executables/installers work; no tool execution.
- Added native fixture tests, frontend typed-contract checks, human UI and scoped evidence documentation. PR and CI pending; Windows installed acceptance not claimed. B source and acceptance PRs remain separate.
- Design and gates: [IDE_WAVE_C_DEVELOPMENT_TRUTH.md](docs/IDE_WAVE_C_DEVELOPMENT_TRUTH.md).

## 2026-11-05 — Wave B1 Find and Replace (source committed)
- Implemented Ctrl+H Find and Replace in human editor with literal case-sensitive/insensitive search, Replace One, confirmation-gated Replace All, and an explicit scan cap that refuses incomplete bulk edits.
- Changes apply only to the active unsaved CodeMirror buffer in one undoable transaction, with an expected-document check and consumed nonce to avoid accidental reapplication on remount. Explicit human Save and revision conflict handling remain authoritative; no new native file-write API.
- Added pure-JS tests for literal regex metacharacters, Unicode UTF-16 offsets, case, literal dollar signs and all-or-nothing scan limits; updated frontend structural assertions and CSS.
- **Verification:** code committed; [source CI 42130813013](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42130813013) pending at last inspection. No installed Windows GUI acceptance or installer produced. Note that branch-wide CI concurrency can supersede older native Git runs; a separate feature branch is preferable next time.

## 2026-11-05 — Live GitHub comparison and native Git CI repair
- Implemented typed read-only public GitHub branch comparison and explicit Source control Check GitHub action. Compares local HEAD, locally stored tracking SHA, and time-stamped remote branch SHA separately; does not infer ancestry or execute Git, use credentials, or connect Lu.
- Fixed a genuine Windows Rust E0505 fixture lifetime failure in [42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) by releasing the borrowed Git tree before the repository.
- Source `b39c6267`; [CI 42128928605](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128928605) Windows host/native desktop checks passed; Linux and frontend queued at last inspection. No installed Windows acceptance.

## 2026-11-05 — Git status native fixture stabilization
- Changed tracked-file test fixture to have a different byte length after editing, preventing false negatives from timestamp/stat-cache resolution. This is a regression reliability change, not a new runtime feature.
- Source `27e6dde8cf6bd12e6ea2fbe19b27c2fb8f74c89f`; [CI 42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) queued at last inspection. No latest exact-SHA CI conclusion or installed Windows smoke proven.

## 2026-11-05 — Human-first Source control pane (source implemented, CI open)
- Replaced sidebar's placeholder Source control section with a read-only local Git status view driven by `ide_read_git_status` Tauri command and versioned `git.status` registry contract.
- Displays branch, local SHA, locally stored upstream tracking status and ahead/behind, real dirty/staged/untracked counts, and up to 120 changed paths; no fetch, push, staging, commit or project-code execution.
- Native-to-schema parity audit confirms 20 operations with 20 matching descriptor schemas; native Git fixture tests and JS static checks added. Latest source `4d1a559d2f37afb062524827a6d23aff6e62f9b5`; **exact-source CI/physical Windows GUI acceptance not yet established.**

## 2026-11-05 — A3 native Git snapshot transitions
- Persist local libgit2 worktree state with each explicit inventory baseline; historical stores remain readable via optional/default fields. Compare the newest baseline against a fresh native status and classify clean → dirty, dirty → clean, both dirty (not equivalent), or unverifiable.
- Real Git fixture now checks baseline cleanliness followed by a modified tracked file and a new untracked file. Source commit `a37be15b808b2d238ac2457aee9e357d056be13f`, [CI 42127486856](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127486856) queued at last inspection.
- The operation is read-only except for user-requested IDE-private baseline storage; GitHub remote HEAD is not fetched, and stored upstream refs can be stale.

## 2026-11-05 — A3 native Git worktree reader (source implemented)
- Added `git2` and `host/src/git_status.rs` for actual read-only local Git branch, staged/modified/untracked paths, and locally configured upstream comparison and ahead/behind. Only the exact native-registered project root may be inspected; parent repository discovery is refused. No shell, Git hooks, network operations, fetch or Git mutations.
- Included true worktree status in the Continuity read model, typed JSON Schema and human UI. Added real native Git fixture tests and frontend source/security assertions.
- **Evidence:** source committed; [exact-source CI 42127100529](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127100529) pending at last inspection. No Windows installed acceptance or live GitHub comparison established.

## 2026-11-05 — A3 historical evidence retention
- Snapshot capture now preserves multiple observations instead of overwriting the previous baseline. Comparisons use the latest snapshot; the UI displays how many baselines exist for the active project. History is bounded to 64 snapshots/1 MiB and refuses further captures rather than silently deleting evidence.
- Added native regression for a second snapshot and newest-baseline comparison. Source commit `d7fd8cfac1d91a1ddc3073e631a249c1f5b1f9a7`; [CI 42125275258](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42125275258) queued at last inspection. No Windows-installed GUI acceptance or verified global paths.

## 2026-11-05 — A3 root-folder discovery and current CI (source only)
- Root-level installers/archives are now discoverable with SHA-256 when the user selects a standalone `Delivered` folder through the existing native project picker. Added host-native regression fixture for this route; no external scanning or execution rights added.
- Source commit under verification: `7409a3cb54fb6c71d42150b9ab362c3b1b5b7096`. [CI run 42124780433](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42124780433) queued at last inspection. **Do not claim this runtime accepted until Linux/frontend/Windows results complete.**

## 2026-11-05 — A3 evidence snapshot and benchmark context refinement
- Added 128 MiB total SHA-256 scan budget on top of 64 MiB per-file limit; over-budget files remain visible but explicitly unhashed. Local toolchain candidates are `present_unverified`, never assumed usable.
- Extended revisioned, untrusted project context to include four recognized benchmark manifest/result paths; stored source revisions, not rerun results. Baseline tests cover matching, changed, new and missing observed artifact bytes after restart.
- Typed capability directory now advertises **19 real host commands**; schema and native signature parity audited structurally. Latest source CI [42124467605](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42124467605) queued at last inspection.
- Out-of-project files, actual Git working-tree diff and live GitHub are NOT inspected by this first A3 implementation. Installer execution / toolchain version checks require future supervised and approved native workers.

## 2026-11-05 — A3 bounded artifact/resource inventory and durable baseline (source implemented, CI open)
- Added `host/src/inventory.rs`: read-only bounded scan of registered project subfolders `Delivered`, `dist`, `benchmarks`, `results`, selected tool/bin folders and NSIS output locations. Captures file size and SHA-256 (up to 64 MiB); skips symlinks and never executes or downloads project code.
- Added read-only local `.git/HEAD` and loose ref/tracking-ref observation. **Not a live remote comparison or authoritative working-tree status**; packed refs/worktree pointers require later support.
- Added `host/src/inventory_history.rs`: explicit persisted evidence baseline, restart recovery and comparison reporting matched/changed/new/missing/unverifiable inspected bytes; no forced cleanup or destructive Git operation.
- Added human Continuity evidence display, `Record evidence snapshot` control, and two typed A2 capabilities. Added native persistence/change/symlink/toolchain/Git fixture tests and frontend contract checks.
- **Not accepted yet:** exact-head native CI [42123874954](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42123874954) still queued at last check; Windows installed UI, out-of-project resources, live GitHub and executable toolchain/installer acceptance remain open.

## 2026-11-05 — Wave A2 typed schemas and Wave A3 project continuity (source implementation)
- Refined the capability directory for human readability: concise typed signatures, expandable full request/result/error schemas, and nested types self-contained using JSON Schema 2020-12 `$defs`. Added reference integrity tests.
- Expanded project orientation to support both project-root documents and `ide/` subdirectory documents. Recorded successful source writes now include a structured safe relative path alongside revision digest, rather than an anonymous save.
- Latest source CI [42122003384](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42122003384) was queued at last check; no exact-source pass/fail has yet been observed.

- Replaced loose input names/result labels with fully typed JSON Schema contract definitions for native request/result shapes, including nested project/file/span/review/proposal records and explicit error schema. Descriptor schema version increased from 1 to 2; contracts remain discovery-only.
- Implemented host-owned bounded `development-ledger.jsonl` in IDE app data, recording successful native project lifecycle and explicit source save observations with revision when available. Added `ide_read_project_context` and human Continuity screen for recent events and revisioned project note excerpts.
- Context distinguishes human/AI-readable project docs from trusted host observations, labels project text untrusted, and marks remote Git, artifacts and toolchains uninspected/unverified. No Lu bridge, shell execution, automatic save or package operations.
- Added native ledger restart/corruption/access tests and JS typed schema/command-parity/context checks. **Source written; exact-head CI and installed Windows acceptance still open.** Ledger is a best-effort orientation record, not an effect journal.

## 2026-11-05 — Wave A2 Semantic Capability Registry (implemented source; acceptance pending)
- Added bounded, versioned, host-authored descriptions of 16 real standalone IDE commands in `host/src/capabilities.rs`; effect class, input names, preconditions, evidence and error/continuation documentation remain separate from actual execution authorization.
- Exposed read-only `ide_list_capabilities` and a human Capabilities directory; added Rust validation and frontend command-parity assertion.
- Capability discovery is never a model bridge, grant, executor, or claim that proposed future Git/toolchain/build services already exist.
- **Verification:** source committed; exact-head CI and installed Windows GUI checks remain open. No new package or local artifact claimed.

## 2026-11-05 — Wave A1 CI repair and human Copy source reference (unverified)
- Investigated [native CI 42074135705](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42074135705): Windows desktop and frontend passed; Linux Rust host tests completed, then formatting check failed on the new span files. Applied rustfmt-suggested formatting changes.
- Added editor toolbar's `Copy source reference`: verifies currently saved SHA revision and selected cursor line using native `ide_read_source_span`, then copies a machine-readable reference with project ID, relative path, full SHA, line and UTF-8 byte offsets.
- Unsaved drafts are explicitly refused for this saved-disk operation. New UI tests were added; no live Lu connection or file writes.
- **Verification and packages:** exact-head CI after repair remains open; native clipboard UX not physically tested; no new Windows installer generated.

## 2026-11-05 — IDE-ARCH-02 / Wave A1 saved-source span (implemented, unverified)
- Implemented `host/src/source_span.rs` bounded read-only exact-revision source spans: one-based line numbers, UTF-8 byte positions, LF/CRLF terminators and blank/trailing lines. Reuses trusted `preview_source_file` and rejects stale revisions; maximum 120 lines and 32 KiB selected text.
- Added `ide_read_source_span` to standalone Tauri command registry and `host/tests/source_span.rs` native tests (Unicode, CRLF, stale SHA, traversal, invalid range, closed project, Unix symlink).
- **Implementation is disk-only.** Unsaved CodeMirror state is not yet covered; no human-facing reference control, AI connection, execution or implicit Save added.
- **Verification:** source committed; tests added but **not executed in this session**. No new installer, exact-head CI result or physical Windows acceptance established. Local resources / `Delivered` not inspected.

## 2026-11-05 — IDE-ARCH-02 capability-first redesign (design only)
- Added [capability-first architecture](docs/CAPABILITY_FIRST_ARCHITECTURE.md) defining one authoritative Rust IDE host, shared human/AI semantic operations, revision-aware source identity, development ledger, verified resource/installer inventory, evidence journal and a gated worker supervisor.
- Replaced the old strictly linear stage implementation order with Waves A–G, preserving eight canonical stage identifiers and existing safety gates. Start Wave A1 source-span API, A2 capability descriptors, A3 continuity read model; defer live Lu bridge.
- **Verification:** design/source commits only; no native/Windows acceptance or local artifact inventory performed. These planned services are **not runtime implementations**.

## 2026-11-05 — AIW-IDE-01 architectural alignment (design-only)
- Read the canonical Lu v1.0 reconciliation's `LU-AIW-01` R1–R7 and draft PR #28's operational-memory design/source. PR #28 remains a source-only, unmerged Lu Core feature, not an IDE dependency.
- Added [AI-first IDE design](docs/AI_FIRST_IDE_DESIGN.md) and IDE-specific `AGENTS.md`. Updated the canonical design to mandate precise source identity, semantic capability discovery, verified tool/artifact resources, task context, evidence and recovery.
- **Implemented:** documentation and contributor orientation. **Not implemented:** source-span host APIs, IDE operational inventory, context assembler, capability registry, receipts, any live Lu integration.
- Acceptance: design inspection only; no exact-head CI run or Windows installation test for this change. Local filesystem and `Delivered` package locations not inspected.

## 2026-11-05 — PCP-01 continuity adoption
- Added `CHANGELOG.md`, `PROJECT_STATE.md`, `CONTINUITY_PROTOCOL.md`, and `scripts/check_continuity.py`.
- Added `.github/workflows/ide-continuity.yml` pull-request gate requiring changelog/current-state changes alongside IDE implementation changes. Branch protection required-check configuration and passing workflow run are **not verified**.
- This records real remote state but **does not inventory inaccessible local workspaces or certify package paths**.

## 2026-11-05 — Standalone IDE R3.7 source (not release-accepted)
- Added in-file Find UI: Ctrl+F, active-buffer matching, next/previous navigation, match count, Escape/close. Changed: `app/web/main.jsx`, `app/web/code_editor.jsx`, `app/web/styles.css`, `app/tests/standalone.test.mjs`.
- Last known R3.7 source commit before documentation: `eb0aefdb1f4e954044b353e64b8e308d13b0e6f6` on `ide/workspace-development`.
- Verification: code committed, tests added; **not yet confirmed by a passing exact-head CI run or user Windows test**. No installer claimed.
- Added dismissible green staging banner and preserved amber saved-file notification (R3.6 source, earlier commits on same branch). User requested the notification; updated behavior has **not** received Windows acceptance.
- **Lu-to-IDE bridge deferred by user:** no live connection, no dependency on Lu being ready.

## 2026-11-05 — R3.5 interactive milestone
- Proposal inbox, review, approval-for-staging, editor-only staging, and human Save remain separate operations.
- User reported the staging and save workflow functioning on Windows; user requested improved green staging feedback. This is **interactive user feedback**, not a full native adversarial-test or installer acceptance.
- Earlier CI/release and acceptance references remain in [ENGINEERING_HANDOFF.md](ENGINEERING_HANDOFF.md) and [R3.5 evidence](docs/IDE_07_R35_PROPOSAL_INBOX.md).

## Earlier milestones
- R3.4: multi-hunk review and data-only proposal schema, automated/interactive evidence recorded.
- R3.3: reviewed save and stale-file handling, automated/interactive evidence recorded.
- R3.2: registered-project removal and persisted catalog state confirmed by user.
- R3.1: code editor line numbers and syntax highlighting confirmed by user.
- R3: editor tabs, editing, explicit save, reopening source verified by user.
- R1–R2: standalone Windows shell, projects and read-only source browsing.

See the engineering handoff and linked acceptance documents for exact evidence. Do not use these summaries as substitute for exact commit or artifact verification.
