# Lu IDE — Engineering handoff / continuation entrypoint

**Updated:** 2026-11-05 (America/Phoenix)  
**Repository:** [vjmichael/Lu-AI-GUI](https://github.com/vjmichael/Lu-AI-GUI)  
**Source branch:** `ide/workspace-development`  
**IMPORTANT:** This is a **standalone Tauri 2 / React / Rust executable**, not a panel embedded in Lu Desktop. Keep all changes within `ide/` or IDE-specific workflows; **do not modify Lu Core** for ordinary IDE milestones.

This entrypoint is intended to spare the next development conversation from rediscovering the original scope, CI, app paths, and acceptance history.

## Required first read (PCP-01)

Before changing code, read [current project state](PROJECT_STATE.md), [chronological changelog](CHANGELOG.md), and [continuity protocol](CONTINUITY_PROTOCOL.md). Confirm real local workspace and `Delivered` contents before assuming GitHub is the most advanced source. Log unknown local or installer locations as unverified, not absent.

**Latest source:** R3.6 notification banner + R3.7 editor Find have been committed, but exact-head CI, desktop smoke and installer checks remain open. **Live Lu-to-IDE connection is user-deferred.** The PCP-01 pull-request gate now requires state and changelog updates for IDE implementation changes; branch protection must make its check required to prevent bypass.

## AI-first philosophy and operational resource memory

Read [AI-first IDE design](docs/AI_FIRST_IDE_DESIGN.md) and [IDE-specific contributor orientation](AGENTS.md) before adding features. The Lu v1.0 `LU-AIW-01` reconciliation requires shared precise source references, discoverable host-validated capabilities, verified resource inventories, bounded provenance-aware context, typed semantic actions, real outcome evidence and recovery. [Lu Core draft PR #28](https://github.com/vjmichael/Lu-AI-GUI/pull/28) provides a source-only resource-memory model, **not a merged IDE or desktop service**. Do not assume it is callable or that old resource paths still exist.

New `AIW-IDE-01` documentation is **design and orientation only**. Next independent gates include reliable line/span+revision identity (including unsaved buffers) and IDE-owned resource/artifact verification; no bridge activation is permitted yet.

## A3 human Source control (read-only)

Source control is now a real sidebar section using typed `git.status` and native `ide_read_git_status`. Human-visible modifications, staged/untracked paths and locally stored upstream divergence come from libgit2, with an explicit distinction from live GitHub (not queried). The capability directory exposes 20 native operations; parity was structurally reviewed but acceptance remains open. **No Git write or network mutation available.**

## A3 Git worktree baseline transitions

Inventory baselines now store the observed Git worktree clean/dirty state (with backwards-compatible defaults for older snapshots). Comparisons distinguish becoming dirty from merely having a matching commit hash; an unchanged commit **never** proves the local source is unchanged. Native fixture regression included. Exact-head CI [42127486856](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127486856) queued; installed GUI and live GitHub reconciliation unverified.

## Wave C — Read-only development truth (in development)

Feature branch `ide/wave-c-development-truth-20261105` adds `git.diff` (bounded staged and worktree patch), `git.history` (30 local commits), and `resource.probe` (current file bytes/digest at a scoped relative path) to the standalone host, typed capabilities and human Source control/Continuity panels. A file digest does not establish executable readiness. Local history/tracking and public GitHub observations retain separate provenance. No executable worker, arbitrary filesystem access, repo mutation or Lu integration. [Design/limits](docs/IDE_WAVE_C_DEVELOPMENT_TRUTH.md). Source implemented; native CI and Windows installed acceptance open.

## Wave B1 human Find and Replace

The standalone editor now has Ctrl+H and a Find/Replace bar: bounded literal case-sensitive/insensitive matching with correct CodeMirror UTF-16 offsets, Replace One or confirmed Replace All. Replacement is an unsaved **single CodeMirror transaction**, not a host write; saved file revision checks remain unchanged. The operation checks expected draft and consumes its nonce so remounting a tab does not replay it. New JS tests live in `app/tests/find_replace.test.mjs`.

**Source implemented, not accepted yet.** See [CI 42130813013](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42130813013), pending at last inspection. Earlier native Git/GitHub checks may be cancelled by same-branch concurrency. Separate feature branches would allow future work to continue without superseding an acceptance run.

## A3 live GitHub evidence and native CI repair

New `host/src/github_live.rs`, typed `git.compareLive`, and explicit **Check GitHub** UI compare observed public GitHub origin branch head against local HEAD and the locally stored tracking ref. It is a bounded, unauthenticated, fixed-origin, read-only HTTPS request; no automatic network call, Git mutation or Lu connection. Different commits do not establish ancestry. Private repositories remain unsupported without separate explicit authorization.

Real Windows CI E0505 from [42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) was repaired by dropping the Git fixture's borrowed tree before repo. [CI 42128928605](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128928605) Windows host tests + Tauri check passed, Linux/frontend queued at last inspection. **Full gate and installed Windows acceptance remain open.**

## A3 Git status host reader (new source, unverified)

`host/src/git_status.rs` uses read-only libgit2 for real local dirty/staged/untracked changes, restricted to exact registered project root and separately labeled locally stored upstream. It neither calls Git CLI nor contacts GitHub. The Continuity UI and JSON Schema now carry the local status; native Git fixture tests exist. CI [42127100529](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127100529) pending; no execution/installer/user GUI gate passed for this change.

## A3 inventory acceptance and honest scope (latest)

The app can inspect selected, registered project roots, including direct root-level installer candidates when an existing `Delivered` directory is registered through the native picker, observe hashes for bounded output/benchmark files, detect local loose Git reference differences, and create durable user-requested snapshots for comparison across restarts. It now includes nested benchmark manifest excerpts with provenance, total 128 MiB digest budget, and two typed inventory capabilities (19 native catalog operations total). **No live GitHub check, no dirty-worktree verification, no global Windows package search, no executable toolchain readiness, no installer acceptance.** Latest source CI [42125275258](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42125275258) queued at last check. Inventory history now preserves up to 64 observations without overwriting previous baselines; comparison targets the latest observed baseline, and a full store fails explicitly instead of discarding evidence. Native/Windows runtime verification remains required.

## A3 inventory and baseline continuation (2026-11-05)

`host/src/inventory.rs` enumerates limited trusted project-relative folders and hashes up to 64 MiB per file without running any binaries. It distinguishes discovered toolchain *candidates* from tested usable toolchains, and reads only selected local Git reference metadata. `host/src/inventory_history.rs` records a user-triggered, app-profile artifact baseline and compares future bounded scans. The Continuity UI exposes discovered hashes and changes; the typed capability directory now advertises `inventory.capture` and `inventory.compare`. **No proof of live GitHub state, dirty working-tree status or packages outside the registered project root.** No permission to execute files is conferred. Latest source acceptance pending [run 42123874954](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42123874954); tests/Windows physical acceptance remain open.

## A2/A3 continuation: typed contracts and project continuity

The latest A3 event format adds `sourcePath` (validated relative file path) alongside `sourceRevision`; project-root and `ide/` orientation files can be surfaced as **untrusted, revisioned excerpts**. A2 results embed JSON Schema type definitions for independent resolution and display compact human summaries. Most recent exact-source CI [42122003384](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42122003384) was queued at last inspection; do not claim host native tests or Windows runtime passed on these new changes until evidence appears.


- `host/src/capability_schemas.json` is the reviewed typed schema source for 17 real Tauri commands, with nested record definitions and schema version 2. `host/src/capabilities.rs` loads those bundled contracts; no arbitrary repo schema may authorize actions.
- `host/src/continuity.rs` is an app-profile event log and bounded read-only context view. `ide_read_project_context` supplies scoped project observations and revisioned (untrusted) orientation-document excerpts, with Git/artifact/toolchain current readiness explicitly unknown.
- Project lifecycle and saved-file operations append best-effort host observations after native success. Logs are not security grants, effect receipts, or proof of package/build success. Corrupt logs fail closed on read; a log append failure never turns a completed write into a retryable failure.
- Human UI sections: Capabilities (typed contracts) and Continuity (project context). **Native CI, exact-head tests, installed Windows UX and real local artifact enumeration remain open until verified.**

## Wave A2 capability registry continuation

`host/src/capabilities.rs` holds the versioned static host-authored catalog of **existing** project, source, save and proposal commands. `ide_list_capabilities` returns that metadata only, with no dispatch or permission grant. The UI now has a Capabilities directory, and a static frontend test compares all registered native operation names against catalog descriptors. Do **not** list future Git, LSP, package or worker actions as available until real guarded implementations exist. No Lu connection. Native exact-head CI and physical Windows acceptance must be checked before closing Wave A2.

## Latest Wave A1 continuation

The source editor now exposes a `Copy source reference` button that verifies an exact saved revision/line through the native span operation, then copies a structured reference. It blocks dirty tabs rather than claiming an unsaved edit is part of a saved revision. `CodeEditor` tracks cursor line by key/mouse. Linux CI formatter failure from run 42074135705 was addressed in code; fresh exact-head CI and physical Windows clipboard acceptance are open. No Lu bridge activated.

## Wave A1 saved-source identity (new code, unverified)

`host/src/source_span.rs` adds `read_source_span` backed by the existing capability-safe text preview. The standalone Tauri command `ide_read_source_span` requires project ID, relative path, exact saved file revision and bounded one-based line range; returns line text, UTF-8 byte offsets and line-ending identity. Rejects stale disk SHA and invalid ranges. `host/tests/source_span.rs` contains native regression cases. **This source has not yet passed native CI or Windows installed acceptance, and cannot reference unsaved editor buffers.** No Lu bridge activated. See [current state](PROJECT_STATE.md).

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
