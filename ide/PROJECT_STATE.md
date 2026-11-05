# Lu IDE — Current authoritative project state

**Updated:** 2026-11-05 (America/Phoenix). **Project:** standalone Lu IDE, not Lu Core.
**Repository:** `vjmichael/Lu-AI-GUI`
**Development branch:** `ide/workspace-development`
**Last inspected implementation HEAD before continuity documentation:** `eb0aefdb1f4e954044b353e64b8e308d13b0e6f6`
**Working tree / local delivery folder:** UNKNOWN — **not inspected in this session**.
**Local ahead/behind remote:** UNKNOWN — never assume a remote checkout is the most complete copy.
**Live Lu integration:** DEFERRED by user; contract work may proceed independently, but the IDE must run without Lu.

## Evidence grades

| Area | Implemented | GitHub source | Automated tests | Native Windows GUI | Installer/artifact |
| --- | --- | --- | --- | --- | --- |
| R3 editor, project lifecycle | Yes | Yes | Prior CI reported; see handoff | User accepted earlier R3 milestones | Prior artifacts in linked evidence; availability must be rechecked |
| R3.4 diff/review | Yes | Yes | Prior CI passed; see handoff | User accepted interactive multi-hunk smoke | Prior unsigned Windows artifact documented |
| R3.5 proposal inbox | Yes | Yes | Prior CI passed; see handoff | User reported functional flow; hostile gates still open | Prior build references in handoff; reverify availability |
| R3.6 success banner | Yes | Yes | Regression assertions added, **exact-head run unverified** | **Not yet tested by user** | **No R3.6 package verified** |
| R3.7 in-file Find | Yes | Yes | [Exact-source CI 42069988384](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42069988384) **passed** at `eb0aefdb` | **Not yet tested by user** | **No R3.7 package verified** |
| Wave A1 disk source spans | Source implemented | Yes | Native tests added, **not yet run** | **Not yet tested by user** | None |
| Find and Replace | No | No | No | No | No |
| Live Lu↔IDE connection | Deferred | No | No | No | No |

**WARNING:** The ledger describes observed evidence at this checkpoint; Git commits alone never establish local build success.

## Wave A1 source-reference UI and CI repair — 2026-11-05

- CI [42074135705](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42074135705): Windows desktop **passed**, frontend **passed**, Linux native job **failed due to `cargo fmt --check` formatting differences**. Rust tests had completed before formatting failed. This was not a source-span behavioral failure.
- Formatting corrections applied to `host/src/source_span.rs` and `host/tests/source_span.rs`, based on actual Rust CI formatter diff; **new exact-head CI still required**.
- Human-facing **Copy source reference** added to the editor toolbar. It tracks the active cursor's line, rejects dirty buffers, calls `ide_read_source_span` against the saved SHA-256 before copying an exact project/path/revision/line/byte span, and reports a human-readable stale-file conflict. It performs no file write and does not enable the Lu bridge.
- New frontend regression assertions accompany the UI. **Clipboard behavior on installed Windows remains unverified**. The operation refers to the **saved disk file**, not any unsaved CodeMirror draft.
- Current completion classification: **source implemented; verification pending exact-head CI and physical Windows UX**. No new installer identified.

## Wave A1 — first host implementation (source only)

- [GitHub Actions 42074135705](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42074135705) started for source commit `15f4b99c`: frontend passed; at last check Windows desktop still running and Linux host queued. **No complete Wave A1 acceptance claim.**

- Added `host/src/source_span.rs`: bounded 120-line, 32 KiB read-only disk source spans using existing capability-relative file preview and mandatory SHA-256 revision match.
- Each reply includes project ID, normalized relative path, saved revision, one-based line numbers, precise UTF-8 byte offsets and LF/CRLF/none terminator types. The last blank line after a final newline is addressable.
- New `ide_read_source_span` Tauri command and `host/tests/source_span.rs` native regression fixtures. This command is **not yet a displayed human code-reference UI, not unsaved buffer snapshots and not an AI bridge**.
- Tests authored for Unicode, CRLF, trailing blanks, stale file, invalid range/path/revision, closed projects and symlink refusal. **Execution and CI status unknown/not run.**
- Follow-up work: compile and verify on Linux/Windows; add a human-friendly “Copy source reference” interaction based on this same host API; extend snapshots to unsaved CodeMirror buffers without treating UI text as trusted disk state.

## Capability-first redesign — IDE-ARCH-02 (design-only)

- Approved as development direction: [Capability-first architecture](docs/CAPABILITY_FIRST_ARCHITECTURE.md). This supersedes the old strictly linear stage order but preserves eight canonical stage acceptance IDs.
- Seven service boundaries planned: workspace registry, revision-aware source snapshots, host semantic capability registry, resource inventory, durable development ledger/context index, effect/evidence journal, gated execution supervisor.
- Human UI uses the same authoritative host operations that a future approved AI client could use. **No Lu runtime bridge is active or required.**
- Wave A first: **A1 source-span/revision API → A2 bounded capability descriptors → A3 continuity read model**, then B human editor hardening, C read-only Git and inventory, D worker security, E/F development workflows, G deferred AI integration.
- **Status:** architecture redesign committed; **Wave A1 saved-disk span service has now been written as source**, while remaining planned services/APIs are **not implemented**. Source tests, native Windows validation and installer acceptance **not run for redesign**.

## AI-first architectural reconciliation — AIW-IDE-01

- **Designed / documented, not implemented:** [AI-first IDE requirements](docs/AI_FIRST_IDE_DESIGN.md), [IDE contributor orientation](AGENTS.md) and links in the canonical design.
- Read verified sources: Lu [v1.0 reconciliation](../docs/LU_V1_0_DESIGN_RECONCILIATION_20261103.md) LU-AIW-01 and [draft PR #28](https://github.com/vjmichael/Lu-AI-GUI/pull/28), `LU-OPMEM-01A`. PR #28 is **draft / source-only**; its SQLite ledger is not integrated into the IDE or current Lu desktop preflight.
- Highest-value standalone gates: revision-aware human/AI source spans, a host-owned verified resource/artifact inventory, concise task context/ledger indexing, and typed host capability descriptors. **These are future gates**, not completed R3.x features.
- User decision preserved: live Lu↔IDE connection **DEFERRED**. Design contracts independently; retain all review, staging and explicit Save boundaries.
- Exact-head CI / native Windows acceptance for this new documentation: **not run**. Local project/package inventory remains UNKNOWN.

## Continuity enforcement state

- PCP-01 docs and CI PR gate added on this branch. **Not yet verified by a passing workflow run.**
- To prevent bypass on protected branches, set the `continuity` job as a required status check (repository administrator operation); simply adding a workflow does not make a check mandatory for merges.
- CI compares the full PR base-to-head diff and requires substantive entries in `ide/CHANGELOG.md` and `ide/PROJECT_STATE.md` for source changes. This is syntactic enforcement; reviewers must check accuracy of evidence and artifacts.

## Work to resume next

1. **Verify Wave A1 host span implementation** on Linux/Windows, including native host tests, Tauri build and exact-head CI. Then connect human source-reference UI without weakening explicit Save. A1 does not yet represent unsaved buffer revisions.
2. **Reconcile branch and local copies**: identify all checkouts / `Delivered` folders / generated Windows packages, capture source roots, hashes and output locations. This step requires access to the relevant local filesystem; no remote connector can certify unseen files.
3. **Verify R3.6/R3.7**: run `cd ide/app && npm run check`, native host tests and Windows Tauri build. Review exact-head CI outcomes and test Find/navigation + banner dismissal on Windows. Resolve failures before calling either release-accepted.
4. **Next standalone editor feature: Find and Replace**, with explicit editor-buffer edits, undo support and no implicit Save. Keep all Lu bridge activation deferred.
5. Work on source control and build/test UX only under the documented authorization/security gates. No untrusted execution on project-open.

## Artifact manifest — do not infer missing paths

| Artifact | Source or expected location | Verified existence now? | Verification |
| --- | --- | --- | --- |
| Repository source | GitHub, `ide/workspace-development` | Yes, connector read | Fetch exact HEAD before editing |
| Windows R3.5 unsigned release | GitHub Actions release reference in `ENGINEERING_HANDOFF.md` | Prior evidence only | Re-fetch run, artifact, checksum before offering a download |
| R3.6 or R3.7 installer | UNKNOWN | **No** | Do not claim an installer exists |
| Local NSIS tooling / `Delivered` directory | UNKNOWN | **No** | Enumerate real filesystem; record absolute path and checksum |
| Windows installation used for user screenshots | User machine, path unknown | User screenshot confirms application UI, path not verified | User Windows acceptance only |

## Safety and scope

- Human-owned edits; approval is not authority to save; proposals are data, not commands.
- Editing, reviewing and staging do not execute project code.
- Do not change Lu Core for standalone IDE milestones.
- Never equate GitHub, local workspace, CI artifacts, or user-installed build versions without evidence.

## Continuation links

[Chronological changes](CHANGELOG.md) · [Engineering handoff](ENGINEERING_HANDOFF.md) · [Continuity rules](CONTINUITY_PROTOCOL.md) · [Implementation gates](docs/IMPLEMENTATION_GATES.md).
