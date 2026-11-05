# Lu IDE — Engineering handoff / continuation entrypoint

**Updated:** 2026-11-05 (America/Phoenix)  
**Repository:** [vjmichael/Lu-AI-GUI](https://github.com/vjmichael/Lu-AI-GUI)  
**Source branch:** `ide/workspace-development`  
**IMPORTANT:** This is a **standalone Tauri 2 / React / Rust executable**, not a panel embedded in Lu Desktop. Keep all changes within `ide/` or IDE-specific workflows; **do not modify Lu Core** for ordinary IDE milestones.

This entrypoint is intended to spare the next development conversation from rediscovering the original scope, CI, app paths, and acceptance history.

## Required first read (PCP-01)

Before changing code, read [current project state](PROJECT_STATE.md), [chronological changelog](CHANGELOG.md), and [continuity protocol](CONTINUITY_PROTOCOL.md). Confirm real local workspace and `Delivered` contents before assuming GitHub is the most advanced source. Log unknown local or installer locations as unverified, not absent.

**Latest source:** R3.6 notification banner + R3.7 editor Find have been committed, but exact-head CI, desktop smoke and installer checks remain open. **Live Lu-to-IDE connection is user-deferred.** The PCP-01 pull-request gate now requires state and changelog updates for IDE implementation changes; branch protection must make its check required to prevent bypass.

## Current user-verified Windows milestones

- **R1:** standalone Windows IDE launches, displays project registration shell and shows "Lu connection: Not connected".
- **R2:** register and browse disposable project; previews Rust and Python source without execution.
- **R3:** CodeMirror tabs; user edited/saved source, closed project and reopened with modified content intact.
- **R3.1:** user's Windows screenshots verified line-number gutters and Rust/Python/Markdown syntax highlighting after CodeMirror CSP nonce fix.
- **R3.2:** user screenshots verified registration removal to zero projects, re-registration and zero-project state persisting across a restart. **Remove from IDE** is catalog-only and never deletes the selected project directory.
- A small R3.2 card-layout regression was corrected in source using `repeat(auto-fill,minmax(275px,1fr))`, preserving regular card widths when only one project remains.
- **R3.3:** user confirmed Windows review dialog, reviewed save, stale-file conflict preserving draft, and large-preview truncation disabling reviewed save. Four Windows screenshots support the result; see `docs/IDE_07_PROPOSAL_REVIEW_FOUNDATION.md`.
- **R3.4:** four Windows screenshots show an actual two-hunk review (2 removed / 2 added at lines 5 and 40), reviewed save with Saved status, and `FILE_CHANGED_ON_DISK` preserving the unsaved draft. R3.4 multi-hunk/save/conflict **interactive smoke accepted**; separate cancel, disk-byte proof and hostile filesystem gates remain open.

These are real user-submitted Windows smoke tests, *not* full hostile-project or installed NSIS acceptance. See `docs/IDE_03_R3_EDITOR_ACCEPTANCE.md` and `docs/IDE_03_PROJECT_DELIST_ACCEPTANCE.md`.

## Historical milestone — R3.3 reviewed changes

The user wants **Lu to use Lu IDE as an authorized coding assistant**, while the IDE remains independently useful.

R3.3 adds a **read-only change-review foundation**, *not* an AI connection:
- Native `host/src/review.rs` compares a human-edited draft to the currently saved file at a specific SHA-256 revision; no writes.
- `app/src-tauri/src/lib.rs` exposes read-only `ide_review_source_change`.
- `app/web/main.jsx` adds **Review changes** and an explicit **Save reviewed changes** control, with project/draft/revision revalidation.
- Review output is at most **240 lines / 32 KiB**, with a truncation warning that disables reviewed-save. Source file preview/save limits remain 512 KiB.
- Source files, model output and project scripts remain untrusted; source content is rendered as escaped text.
- `host/tests/review.rs` checks no filesystem effect, stale revision refusal, traversal, symlink denial and size limits.
- `app/tests/standalone.test.mjs` checks command allowlists, review UI, and no executable or arbitrary patch apply path.

**R3.3 verified:** [Windows/Linux/frontend acceptance 41981163363](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41981163363) **passed** (33 Rust tests, frontend, Windows Tauri); [unsigned Windows R3.3 release 41981333831](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41981333831) **passed**. The EXE SHA-256 and portable ZIP are recorded in [R3.3 evidence](docs/IDE_07_PROPOSAL_REVIEW_FOUNDATION.md). **Interactive Windows R3.3 review-dialog smoke acceptance PASSED on 2026-11-05**; full security/agent integration remains pending.

## R3.4 accepted interactive smoke — accurate multi-hunk review + proposal schema

- The user completed the R3.3 Windows smoke gate on November 5. The milestone's **automated Windows/Linux verification, optimized Windows release and human R3.4 GUI smoke have all passed**; adversarial/edge-case acceptance remains outstanding.
- `host/src/review.rs` now uses the Similar line diff algorithm with grouped hunks and original/proposed line numbers. Limit: 240 preview lines / 32 KiB / 64 hunks / 8,000 input lines. Truncated reviews cannot be saved from the review dialog.
- `app/web/main.jsx` and `styles.css` display true separate hunks and source line-number gutters.
- `host/src/proposal.rs` defines a strict versioned, data-only JSON envelope for **untrusted** multi-file proposals. It rejects unknown effect/approval fields and never applies edits; `claimedProducer` is not authentication.
- `host/tests/proposal.rs` verifies two-file previews do not write, and a stale file causes rejection. `host/src/review.rs` tests check distant edits and newline handling.
- Acceptance record: [R3.4 multi-hunk and envelope plan](docs/IDE_07_R34_MULTIHUNK_PROPOSALS.md).
- CI: [R3.4 run 42053180326](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42053180326) **PASSED** (42 Rust tests, 13 frontend tests, Windows Tauri compile). [Windows unsigned release 42053262838](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42053262838) **PASSED**; executable and ZIP checksums in [R3.4 evidence](docs/IDE_07_R34_MULTIHUNK_PROPOSALS.md). **User R3.4 desktop multi-hunk/save/conflict smoke accepted (2026-11-05)**.

## R3.5 candidate — IDE-owned proposal inbox and explicit staging

- Implemented in `host/src/inbox.rs`, `host/tests/inbox.rs`, `app/src-tauri/src/lib.rs`, and `app/web/main.jsx`: bounded **16-proposal in-memory queue** for strict untrusted R3.4 JSON envelopes.
- The new Proposals screen imports and reviews full diffs. A human can approve **for staging only**, reject or dismiss; no action writes project files. Approvals are digest-bound, non-replayable, and revalidate every source revision before approval.
- **Stage in editor** is a separate explicit action requiring an approved current proposal. It rechecks all file revisions, confirms before replacing dirty tabs and loads proposed text into CodeMirror **unsaved editor drafts**. Only the existing human **Save** action writes disk bytes, one file at a time.
- The entire inbox is memory-only and disappears on restart; no untrusted proposal data goes into the project catalog. The `claimedProducer` field remains unverified and no Lu bridge exists.
- Tests include import/review, stale source, wrong digest, duplicate decision, incomplete preview, rejected staging, approved staging without writes, and volatile reset. Details and Windows manual acceptance: [R3.5 proposal inbox](docs/IDE_07_R35_PROPOSAL_INBOX.md).
- **R3.5 tests passed:** [Windows/Linux/frontend native matrix 42063329884](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42063329884) passed: 50 Linux Rust tests, 16 frontend tests, Windows Tauri compile. [Optimized Windows release 42063134868](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42063134868) passed. The EXE and portable ZIP hashes are recorded in [R3.5 acceptance](docs/IDE_07_R35_PROPOSAL_INBOX.md). **R3.5 user subsequently reported staging and saving work on Windows; full security and installed-release acceptance remain open.** All builds remain standalone; Lu Core unchanged.

## Source map

| Path | Purpose |
| --- | --- |
| `ide/app/web/main.jsx` | Standalone project UI; tabs; explicit saves; change-review UI |
| `ide/app/web/code_editor.jsx` | CodeMirror 6 state, syntax and per-tab undo/redo |
| `ide/app/web/styles.css` | App, project cards, editor and review modal |
| `ide/app/src-tauri/src/lib.rs` | Native Tauri command boundary; no Lu Core link |
| `ide/host/src/lib.rs` | Project registration lifecycle and catalog-only delisting |
| `ide/host/src/browser.rs` | Capability-relative and bounded source reads |
| `ide/host/src/editor.rs` | Revision-checked temporary-file and rename save |
| `ide/host/src/review.rs` | Bounded non-mutating multi-hunk change review |
| `ide/host/src/proposal.rs` | Strict data-only, unauthenticated proposal envelope validation and preview |
| `ide/host/src/inbox.rs` | Volatile human-review inbox with digest-bound approval and data-only staging |
| `ide/sandbox` | Separate experimental Bubblewrap native probes; not wired to IDE execution |
| `ide/docs/IDE_WORKSPACE_DESIGN.md` | Canonical eight-stage implementation plan |
| `ide/docs/ADR_0001_STANDALONE_APP_AND_LU_BRIDGE.md` | Separate desktop and optional authenticated Lu bridge |
| `ide/docs/ADR_0002_WORKSPACE_CHAT_BOUNDARIES.md` | IDE Coding Chat is distinct from Lu native chat and NVP chat |
| `ide/docs/IDE_07_PROPOSAL_REVIEW_FOUNDATION.md` | R3.3 safety boundaries and Windows acceptance plan |
| `.github/workflows/ide-standalone.yml` | Linux host native tests; frontend build/tests; Windows host/Tauri checks |
| `.github/workflows/ide-alpha-windows.yml` | On-demand unsigned optimized Windows executable artifact |

## Local and CI checks

```sh
cargo test --manifest-path ide/host/Cargo.toml --all-targets
cargo fmt --manifest-path ide/host/Cargo.toml -- --check
cd ide/app
npm install --ignore-scripts --no-audit --no-fund
npm run check
cargo check --manifest-path src-tauri/Cargo.toml
```

Desktop Tauri uses Rust 1.90+, native host suite supports Rust 1.88+. The Windows release workflow is purposely separate from fast native CI; edit its name/artifact only to trigger a new numbered alpha build, or dispatch it explicitly. Release outputs are **unsigned**, not a production installer.

## IMPORTANT open gates

1. **R3.3/R3.4 Windows review smoke gates closed; R3.5 inbox implemented, awaiting exact native CI and user acceptance.** Verify manual Import → Review → Approve → Stage (unsaved) → explicit Save. Confirm rejection, stale source, review truncation, and restart behavior. Full automatic Lu application remains unauthorized.
2. Native file-save hardening: root identity and Windows reparse attacks, ACL/owner fidelity, external-writer race across final compare/rename, power-loss injection, crash recovery of unsaved drafts, full diff accuracy.
3. Task 2 **OS sandbox**: initial Linux fixed probes passed, but arbitrary executable workers remain disallowed until CPU/RAM/PID quotas, process-tree cancellation, syscall/filesystem controls and **Windows native containment** pass.
4. Task 3 editor completion: search/replace, split views, accessible keyboard navigation, large tree filtering and file watchers.
5. Tasks 4–6: LSP/DAP/build/test, package manager and advanced Git; do not activate build/package scripts merely by project open.
6. Task 7: **actual authenticated Lu↔IDE bridge** and project-specific IDE Coding Chat. All model-generated edits must be data-only proposals validated by IDE and authorized per task, not shell commands or reuse of Lu's ambient permissions.
7. Task 8: disposable Lu Dev instance/worktree with human-approved merge, rollback, stable Lu untouched.
8. Real installed Windows/Linux app and eventual NSIS lifecycle acceptance.

Do not report a passed CI matrix as proof of user-installed UX, or a successful namespace probe as full sandbox acceptance. No automatic Lu connection exists yet.
