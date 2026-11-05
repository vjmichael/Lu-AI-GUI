# Lu IDE — Development changelog

This records **what changed**. Read [PROJECT_STATE.md](PROJECT_STATE.md) for what is authoritative *now*, and [CONTINUITY_PROTOCOL.md](CONTINUITY_PROTOCOL.md) before any new development session.

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
