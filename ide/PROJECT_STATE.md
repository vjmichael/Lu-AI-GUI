# Lu IDE — Current project state (authoritative handoff)

## Wave C6 — Task evidence and development ledger integration (2026-11-05)

- **Source branch:** `ide/wave-c6-task-ledger-20261105`, stacked on exact C5 source `1dd4a58d84d6bc5f383f75e463b15f82baea6de8` (C5 PR #49). Earlier C3/C4 are passing native/frontend/continuity GitHub CI on their own heads, but no physical installed Windows acceptance. The standalone IDE and Lu Core remain independent.
- **One private ledger:** C6 extends existing `development-ledger.jsonl` events with an optional, strictly typed task payload and host-owned task materialization. Legacy events remain readable; a task change appends a new event and never rewrites old history. Snapshot of local Git HEAD and dirty state is captured by the host at task update time.
- **Human task claims:** native `task.update`/`ide_record_task_update` supports bounded task IDs/titles/next actions, status `planned | in_progress | blocked | ready_for_review`, and user-selected gates `source_review | exact_head_ci | installed_windows | human_approval`. No `completed` input is allowed. Maximum 64 tasks per registered project.
- **CI-linked evidence:** `task.ciObserve`/`ide_observe_task_ci` independently invokes the trusted exact-HEAD public GitHub inspector, binds its bounded summary to the *latest* human task event ID and current source commit, and appends a separate observation. The frontend never supplies the CI verdict, workflow IDs, credentials, Git SHA or any acceptance claim. An intervening task edit or changed HEAD invalidates the old CI link.
- **Evidence gates:** host projects per-task gate statuses. Successful commit CI can produce `commit_ci_passed_not_task_accepted`; it never proves unsaved buffers, installed Windows UI, a human source review, or final task acceptance. The Continuity UI supports task updates and explicit CI observation, while the portable checkpoint exports only task IDs, statuses, commit/evidence coordinates and explicit unknowns (no task title or private next-action text).
- **Security:** app-profile-only append under one mutex, no project file mutation, no arbitrary process execution, no Lu runtime bridge. Native test fixtures and Node frontend contract/privacy tests added. **Verification:** exact-head C6 Linux, Windows, frontend and continuity CI not yet accepted; physical installed Windows UI and actual live public CI lookup in an installed app remain open.


## Wave C5 — Exact-head CI receipts (2026-11-05)

- **Implementation branch:** `ide/wave-c5-exact-ci-evidence-20261105`, stacked on C4 commit `ca606dfb378ce31945a768a23e9ab6a31cbe60e8`. C3 PR #47 and C4 PR #48 remain independent/open pending acceptance; do not merge C5 before dependencies.
- **Source implementation:** `host/src/github_ci.rs` read-only public GitHub Actions inspector and typed `git.ciExactHead` host capability/`ide_read_exact_head_ci` Tauri command. An explicit user click queries only `https://api.github.com` for the active registered Git project's validated origin and exact local HEAD SHA. No cookies, tokens, shell, Git fetch/push, source writes or background polling.
- Workflow receipts check the *specific* `ide-standalone.yml` (required: host-linux, frontend, desktop-windows) and `ide-continuity.yml` (required: continuity) workflow paths, matching run head SHA, matching job head SHA when present, completion and success. Missing/duplicate jobs, partial responses, failure/pending runs or local HEAD changes never count as passing. Unknown origin, private repo/rate limits, absence of runs and incomplete evidence remain explicit.
- Human Source control view has **Check exact HEAD CI**. C4 evidence matrix and C3 Copy development checkpoint accept only cached, explicit, current SHA-bound CI receipt metadata; copying has **no** additional network request. Dirty worktree/unsaved drafts, installed Windows application, toolchain usability and individual task completion **remain unverified** by CI.
- **Implementation verification status:** Rust pure fixture tests and Node regression tests added; exact-head Linux/frontend/Windows CI and installed Windows UI acceptance **PENDING**, not passed at authoring. No new installer claimed; local machine's Deliverables/resources uninspected.

### C4 frontend CI regression repair (2026-11-05)
- Earlier C3 exact-head run 42145804942: Linux native and Windows desktop succeeded, frontend failed a **test-only** capability-registry regex expecting same-line Rust macro arguments. Updated the contract assertion to support multiline rustfmt and require 24 nonempty commands. Direct catalog/Tauri parity: 24=24, no missing or duplicate commands.
- Wave C3 source was separately repaired at `efe731d874e11309f4646d150827a723ed9932f7`. This C4 stacked branch carries the same test repair independently. Latest C4 exact-head CI, installed Windows UI acceptance and toolchain execution remain open.

## Wave C4 — Development evidence reconciliation (2026-11-05)

- **Implemented in source on independent branch** `ide/wave-c4-evidence-reconciliation-20261105`, based on exact Wave C3 SHA `2572fa445d31d265b0ef27ef52a1dd4a09156326`. No Lu bridge or executable worker.
- A pure `development_reconciliation.js` derives ten human-readable evidence classifications from the existing native project context + inventory comparison. The Continuity screen labels local Git, stored upstream, live remote (not queried), bounded artifacts, historical baseline, project task claims (unverified), host events, native CI, installed app, and toolchain usability separately.
- A clean observed Git tree or matching SHA never implies source build success or installer acceptance. Unknown/mixed project IDs fail closed; incomplete scans and truncated paths remain explicit. Project-authored document text is never used as authoritative task completion.
- The human Copy development checkpoint export embeds the conservative matrix; no new Tauri capabilities/permissions or project writes. JS tests cover contradictory Git state, changed/missing bytes, incomplete scan, absence of CI evidence, privacy and UI wiring.
- **Verification:** source committed but full latest-head native Linux/frontend/Windows CI and installed Windows GUI acceptance pending. No package or CI receipt claimed for C4 at authoring. Worktree/Delivered on the user's Windows device remains uninspected.


## Wave C3 — Portable development checkpoint (2026-11-05)

- **Implemented in source on draft PR #47:** Human **Copy development checkpoint** in Continuity performs fresh parallel native context and inventory comparison reads for the exact currently selected registered project and copies bounded versioned JSON. Project IDs must match both host responses; switching projects invalidates pending requests and any stale continuity UI. Clipboard failure is surfaced, not treated as success.
- Captures local Git HEAD/branch/dirty counts and stored tracking evidence, bounded artifact SHA/size/status, baseline changes, recent event IDs/saved revisions, and orientation document revision IDs. Omits source and note excerpts, event summaries, private absolute roots, credentials, executable work and live GitHub content. Limits and unknown states are explicit. **Two reads are not an atomic transaction**; copy time is not evidence time.
- New pure helper and Node tests: `app/web/development_checkpoint.js`, `app/tests/development_checkpoint.test.mjs`. Local isolated Node helper tests passed (3) before repo integration; full frontend/Windows CI and installed Windows copy UI still pending at PR head.
- Latest verified predecessor commit before this checkpoint: `26040e8331f5a5b8053f4af8248a88d9d551f04f`; new source SHA must be taken from GitHub after publication, not inferred. Local user's checkout/Delivered still not inspected. Lu bridge remains deferred.

**Observed date:** 2026-11-05 (America/Phoenix)  
**Repo:** `vjmichael/Lu-AI-GUI` · **Working branch:** `ide/wave-c4-evidence-reconciliation-20261105` (stacked on Wave C3 PR #47; unmerged). Wave C3 remains on `ide/wave-c-development-truth-20261105`.  
**Last inspected implementation source SHA:** Prior C3 source commit `d6bdca75953ab63579be5d071ec706effe4c6771`; inspect latest [draft PR #47](https://github.com/vjmichael/Lu-AI-GUI/pull/47) HEAD for the current source and exact-head validation.
**Current source validation:** Wave C3 source and tests committed; exact-head Linux/frontend/Windows checks and installed Windows clipboard acceptance **pending**. First C3 matrix [42145655561](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42145655561) queued after prior source. A subsequent race-state cleanup supersedes it; inspect the final PR HEAD, not prior runs.
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
| **A2 typed Semantic Capability Registry** | **Implemented**: 21 real host operations, JSON Schema v2, self-contained nested definitions, native-argument parity tests, Capabilities UI | **Exact-source CI pending** on latest code | Windows installed UX open |
| **A3 Development Ledger and Context Read Model** | **Initial runtime implemented**: host app-profile JSONL event history, bounded project context and Continuity UI | **Exact-source CI pending** on latest code | Windows installed UX open |
| A3 bounded local artifact and toolchain-candidate inventory | **Implemented in source** | CI on latest source pending | Installed acceptance open |
| A3 persistent snapshot and compare | **Implemented in source** | Native regression fixtures added; CI pending | Installed acceptance open |
| A3 native local Git dirty/untracked inspection | **Implemented in source** | Exact-head CI pending | Windows UI open |
| Wave C1 local staged/unstaged Git diff/history | **Implemented on isolated branch** | CI pending | Installed UI open |
| Wave C2 targeted resource SHA probe | **Implemented on isolated branch** | CI pending | Installed UI open |
| A3 full live GitHub and external toolchain reconciliation | **Not implemented** | Not tested | Open |
| Live Lu connector, worker execution, LSP/build/test/package | **Deferred/not implemented as IDE runtime** | Not tested | Open |

## A2 — Typed contracts actually implemented

- **Source:** `host/src/capabilities.rs`, `host/src/capability_schemas.json`, standalone Tauri `ide_list_capabilities`, React Capabilities screen.
- Host-created, versioned descriptors for **21 existing** project, source, review, save, proposal, continuity, inventory and Git/GitHub operations. Request/result JSON schemas are typed, with nested structures and self-contained JSON Schema 2020-12 definitions. Response includes typed error schema. **Discovery is not permission, execution or a Lu bridge.**
- UI shows concise operation input/result signatures, with full schema expansion on demand. Rust validation and frontend structural/command parity tests added.

## Wave C — Development truth (independent source implementation)

- [Draft PR #47](https://github.com/vjmichael/Lu-AI-GUI/pull/47), source repair SHA `dbaa04ff`. First-run [42143131954](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42143131954) frontend passed, Linux failed E0308 in resource timestamp conversion. Converted cap-std time to std time before Unix offset; new exact-source [42143391542](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42143391542) pending. Native Rust/Windows launch and user acceptance remain open.


- Isolated feature branch `ide/wave-c-development-truth-20261105` while Wave B PRs remain in their own GitHub acceptance queues. **Do not mark the completed Wave B source work as Windows-accepted yet.**
- Implemented `host/src/git_evidence.rs` to inspect staged/unstaged local Git patch text and 30 local commits, bounded to 64 KiB of patch and 80 changed files. It refuses parent repository inspection, excludes untracked *contents* from patch, performs no Git command or mutation, and labels remote state as `not_queried`.
- Added typed `git.diff` and `git.history` and an explicit human **Inspect diff and history** action in Source control.
- Implemented `host/src/resource_probe.rs` for exact relative-path SHA-256 and metadata under the registered project. Missing/oversized/not-regular/changed files are not mislabeled as verified. **Executable readiness remains not tested**; no shell, package install or Lua/Lu bridge.
- Added typed `resource.probe` and a **Recheck file bytes** button in Continuity. The registry now describes **24 native operations** with typed schema parity verified structurally (24 descriptors / 24 schemas / 25 registered handlers including discovery).
- Native fixture tests cover Git patch separation, bounded history, root isolation, symlink and traversal, resource SHA and missing/over-limit cases. Frontend contract tests and isolated PR CI trigger added. Full semantics: [Wave C development truth](docs/IDE_WAVE_C_DEVELOPMENT_TRUTH.md).
- **Acceptance status:** source implemented; exact-head Linux/frontend/Windows CI and installed Windows UI remain open. No global executable toolchain inspection, authenticated private remote comparison, or real worker execution claimed.

## Wave B1 — Human Find and Replace (source implemented, exact-head CI open)

- `app/web/find_replace.js` is a pure bounded literal match-and-replace planner for the **active unsaved CodeMirror draft only**. Matches preserve UTF-16 offsets, escape regex metacharacters and optionally match case. All-or-nothing replacement refuses scans above 10,000 matches; no partial bulk edit or direct disk write.
- `app/web/main.jsx` adds Ctrl+H, human Find/Replace controls, Replace One and confirmation-gated Replace All. All edits enter the existing draft/dirty state and still require explicit Save and revision-checked host writes.
- `app/web/code_editor.jsx` dispatches a planned set of changes as **one undoable CodeMirror transaction**, checks the expected draft before applying and acknowledges/consumes the nonce to prevent replay when tabs switch. No Lu model or file/host tool added.
- New `app/tests/find_replace.test.mjs` includes UTF-16 Unicode offsets, literal metacharacter search, case-sensitivity, literal replacement of dollar strings, match selection, over-limit refusal and empty query. Updated existing frontend regression and editor styles.
- **Verification:** implementation source committed; CI pending. The previous native Git/GitHub CI run for `b39c6267` is no longer an exact-head check for this new source. CI concurrency may cancel superseded branch runs. No new Windows installer or physical acceptance claimed.

## A3 live GitHub evidence and native Git CI repair

- `host/src/github_live.rs` provides an explicit read-only check against public GitHub origin HEAD, exposed through typed `git.compareLive`, Tauri `ide_compare_live_github`, and a human **Check GitHub** button. The catalog contains **21 actual native operations**.
- Origin is syntactically restricted to `github.com`, with fixed `https://api.github.com` REST, no redirects, credentials, shell commands or Git mutation. TLS query has short timeouts and a 48 KiB response bound.
- Separately reports time-stamped GitHub branch SHA, local HEAD, stored tracking SHA, local dirty/untracked status and **SHA equality or difference without assuming ancestry**. Private/rate-limited/unavailable GitHub remains unverified; no automatic request or Lu connection.
- CI [42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) exposed E0505 dropping a Git repository before a borrowed Git tree during test teardown. Fixed by releasing the tree first, in source `b39c6267`.
- CI [42128928605](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128928605): **Windows host and Tauri checks passed**; Linux/frontend queued at last inspection. No physical installed Windows UI acceptance or local user filesystem inspection.

## A3 native Git worktree reader — implementation committed, CI pending

- Native Git dirty-status regression now alters file byte length to avoid a same-size timestamp/stat-cache race. Latest source commit `27e6dde8cf6bd12e6ea2fbe19b27c2fb8f74c89f`; [CI run 42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) queued; no recent compile or physical Windows acceptance result for this SHA.


- Activated the **Source control** sidebar page (previously 'Soon'), backed by new read-only `ide_read_git_status` Tauri command and typed `git.status` capability. UI shows exact local branch, modified/staged/untracked counts, local commit and upstream difference, limited change list and explicit no-live-GitHub disclaimer.
- New native operation increases capability registry to **20** descriptors, matching 20 implemented non-discovery Tauri commands. Structural audit found no argument/schema mismatch. Source revision `4d1a559d2f37afb062524827a6d23aff6e62f9b5`; full Linux/frontend/Windows acceptance remains open.


- Latest implementation SHA `a37be15b808b2d238ac2457aee9e357d056be13f`, source-specific verification [run 42127486856](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127486856) queued. Tests include native Git fixture + baseline transition; Linux build and Windows installed acceptance remain open.

- `host/src/git_status.rs` uses native `git2` / libgit2 to inspect the **exact registered project root** without launching `git` or executing project code. Refuses accidental parent-repository discovery.
- Returns branch, local commit ID, locally configured upstream tracking commit, locally computed ahead/behind counts, true modified/staged/untracked file counts and at most 120 changed-path summaries. The Continuity screen displays these separately from older loose-ref evidence.
- **Not a live GitHub comparison**: no fetch, push, network access, remote PR inspection or authority to mutate Git. Worktree inspection can be unverifiable, and none of its observations grant permission to write.
- Added native Git fixture tests for clean -> dirty/untracked and parent-repo refusal, plus frontend/schema checks. A recorded inventory baseline now includes the observed clean/dirty worktree state, and comparison reports a transition such as `became_dirty` rather than assuming the same commit means no local changes. `git2` now a standalone host dependency; native Windows/Linux CI must compile and run before marking this accepted.
- CI: [run 42127100529](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127100529) pending at last check; previous build `42125275258` had Windows success, but its Linux/frontend jobs were cancelled as source evolved.

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
