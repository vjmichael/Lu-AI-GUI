# Lu IDE — product readiness and blocker-first execution ledger

**Created:** 2026-11-07 America/Phoenix / 2026-11-08 UTC.  
**Owner:** standalone `ide/` in `vjmichael/Lu-AI-GUI`.  
**Scope:** organize outstanding acceptance, source integration and daily-driver functionality; **no source/security/CI gate is waived**.  
**Canonical closeout phases:** [P01–P07 ledger](IDE_RELEASE_CLOSEOUT_P01_P07_20261107.md). This is a **task execution queue**, not a replacement for the phase ledger or the distinct historical PR/evidence history.  
**Baseline:** [P06-R2 #111](https://github.com/vjmichael/Lu-AI-GUI/pull/111) `3feeff48291857cd7ac68e3705eef448b369339e`. The latest `main` examined was `7460aec36b56a1b1ed5bcee70592accbf883d0d5` and **has no `ide/` tree**. Do not treat a stacked draft as merged into main.

## 1. Acceptance language and operating rules

- **OPEN-FAILED:** observed native/CI failure at a known head; source changes and exact-head reruns required.
- **OPEN-PARTIAL:** one platform, lane, or narrower scenario passed; another required lane failed, is queued, or was not exercised.
- **OPEN-PENDING:** implementation or workflow exists but the required exact-head native evidence has not completed.
- **BLOCKED:** a named prerequisite must be accepted before this task can be closed; source work may proceed without claiming acceptance.
- **SOURCE-IMPLEMENTED:** code committed; not a native or installed pass. **ACCEPTED:** all listed required OS/tests/receipts passed on the **same final task head**, reviewed with real logs/artifacts and entry in this ledger.
- Use the primary task's own draft PR and head SHA for fixes. Inspect a failure's actual OS log before changing anything. No fixture bypass, mock substitution, uncontained fallback, skipped OS lane, hidden `continue-on-error`, or broadening security grants to gain green CI.
- Preserve original historical PRs, branches and workflows until the complete P05 provenance audit and replacement native-equivalent checks justify a **separate** owner-reviewed cleanup. No mass closings. Do not silently conflate source, runner, packaged/installed, user, or release acceptance.
- Final publisher/legal redistribution review is **deferred until final shipping contents are known**; maintain notice collection and risk tracking now. Windows Authenticode identity and release signing are deferred/optional for the unsigned development pilot; no prior signed release matrix is thereby silently accepted. P07/WO10 Lu Core bridge remains deferred until after Core v1.0 and explicit reauthorization.
- Keep one fix group per working cycle, commit, request exact-head native verification, record pass/fail (with actual job URL, OS, file SHA and log stage), then choose the next independent unblocked group. Do not spend the entire development cycle polling queued jobs.

## 2. Immediate blockers — execute first

These are **independent repair tasks**, ordered by ease of isolation and evidence dependencies, not a claim that an older passing workflow applies to the newest source.

| ID | Priority / state | Specific task and ownership | Native exit evidence / done when |
| --- | --- | --- | --- |
| **BLK-01** | P0 · **OPEN-FAILED** | P06-R2 Windows case-insensitive filesystem test on [#111](https://github.com/vjmichael/Lu-AI-GUI/pull/111). [Run 42445698778](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42445698778) Windows failed `test_special_files_and_casefold_collisions_fail`: NTFS did not create the second case-only pathname. Repair the **test's OS-realistic fixture** and preserve runtime collision denial. | Current-head Windows 2022 and Ubuntu 24.04 complete source/host-filesystem test lanes pass; symlink/reparse, case-collision and mutation denial remain proved. The resulting candidate-tree inventory still has `nativeInstalledVerified=false`. |
| **BLK-02** | P0 · **OPEN-FAILED** | P05-B3 Windows source PowerShell parse error on [#100](https://github.com/vjmichael/Lu-AI-GUI/pull/100). [Run 42443088413](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443088413) Windows source matrix failed in `Fixed scope evidence` with unexpected `)`; Ubuntu 26.04 passed, Ubuntu 24.04 queued. **Do not infer B3 feature failure:** separate [installed Windows run 42443088407](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443088407) succeeded at B3 head `3edc7e4...`. | Repair exact invalid PowerShell expression without broadening fixture scope; full current-head Windows2022 / Ubuntu24 / Ubuntu26 source lanes pass, and final same-head installed B3 search safety remains green. |
| **BLK-03** | P0 · **OPEN-FAILED** | P05-B2 installed Windows copy/Undo/Save/restart on [#99](https://github.com/vjmichael/Lu-AI-GUI/pull/99). [Run 42443379591](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443379591) successfully built, installed and exercised native editor/picker/restart/conflict, then failed `installed_draft_reference_clipboard_not_verified`. Investigate actual UIA/clipboard, source identity and live editor state; do **not** fake clipboard contents or use native IPC to manufacture a pass. | New exact-head **real installed unsigned NSIS** copy reference / edit / Undo / explicit Save/rebased disk SHA / completely separate application restart with a *different* session ID passes; clipboard/restart never writes unsaved content; clean uninstall and license checks retained. |
| **BLK-04** | P0 · **BLOCKED on BLK-03, then BLK-02** | Reconcile B3 project search onto **latest accepted B2 head**, preserving **both** session draft-ref lifecycle and revision-safe saved-project search. The branches diverged in the shared real Windows UIA fixture. No overwrite-based source grafting or premature PR closure. | Real ancestry-preserving merge/reconciliation; source review confirms neither behavior was discarded; repeat current merged-head Windows installed B2+B3 and full Windows/Ubuntu B3 tests. Record PR ancestry, exact commit, receipts. |
| **BLK-05** | P0 · **OPEN-FAILED** | P04 real Linux installed GTK/AT-SPI folder chooser on [#102](https://github.com/vjmichael/Lu-AI-GUI/pull/102) and parent [#97](https://github.com/vjmichael/Lu-AI-GUI/pull/97). [Run 42442157892](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42442157892) had `active_window_unavailable`, visible chooser still open after four bounded actions; `native_folder_chooser_not_accepted`. Native `dpkg` injected failed-upgrade rollback **passed** in that job; do not roll this into an all-P04 failure. | Real installed Linux GTK chooser accepts target through OS UI, source opens, edit/save/restart/external collision and ZIP survive, genuine uninstall/recovery evidence and no AT-SPI/DOM/IPC shortcut. Include Ubuntu24 and Ubuntu26 coverage as required by the owning matrix. |
| **BLK-06** | P0 · **OPEN-PARTIAL** | P03 actual protected Windows AppContainer/SCM/Job native clangd + LLDB-DAP and Linux Bubblewrap/cgroup fixed debuggee on [#104](https://github.com/vjmichael/Lu-AI-GUI/pull/104), parent [#97](https://github.com/vjmichael/Lu-AI-GUI/pull/97). Earlier [real protected WO6 run 42437278814](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42437278814) failed; newer [42442157923](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42442157923) has no completed native OS jobs yet. Security source/containment is materially implemented; genuine fixed DAP launch/stop/thread/stack/cancel not fully accepted. | Both OSes: actual isolated normal work, deterministic denial outside approved scope, worker descendants reaped on cancel/service/host crash, no stale replay, exact-head receipts and package DLL/source pinning; never disable containment to claim success. |
| **BLK-07** | P0 · **OPEN-PARTIAL** | P06-R1 exact-head [#108](https://github.com/vjmichael/Lu-AI-GUI/pull/108) native provenance + live observer. [Run 42444691324](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42444691324) Windows 2022 passed and its newly archived license ZIP confirmed six/six original upstream bytes identical; Linux 24.04 job queued. [Run 42444691336](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42444691336) live observer still queued with no current-head JSON. | Both actual native Windows/Linux dependency artifacts and fetched upstream notice hashes independently verified at one exact head; bounded live JSON exists and correctly reports missing/pending/failed jobs, retains all release/legal/protected flags **false**. R1 **technical** evidence only. |
| **BLK-08** | P0 · **BLOCKED on BLK-05/06; partly implemented** | P02/P04 installed source/GUI and protected WO7–WO9 acceptance on [#79](https://github.com/vjmichael/Lu-AI-GUI/pull/79), [#84](https://github.com/vjmichael/Lu-AI-GUI/pull/84), [#97](https://github.com/vjmichael/Lu-AI-GUI/pull/97), [#102](https://github.com/vjmichael/Lu-AI-GUI/pull/102). Windows installed editor/NSIS smoke has passed on older exact heads, but full installed Windows/Linux protected Run/Test/Debug and Git/ZIP/DAG/recovery, installer migration and rollback are not collectively accepted at one candidate SHA. | One consolidated exact-head real Windows/Linux matrix: install → launch/picker → edit/save/restart/external conflict → approved protected build/test/debug/cancel and Git/ZIP → crash/recover → uninstall; separately verify actual upgrade/rollback and installed package SHA. Unsigned pilot and signed production gates remain distinct. |

### Gate checks for each repair

- **Identify:** copy exact SHA, CI URL, OS runner and failed step from real GitHub log. If the runner was queued/no jobs, status is **PENDING**, not failed or passed.
- **Repair minimally:** do not switch to synthetic UI, test-only protected fallback, bypass signer checks, close historical PRs or disable an entire OS matrix. A fixture-only correction must keep the OS witness.
- **Verify:** full owning exact-head matrix after the *last source change*; inspect all jobs, explicit skips, failure injection and genuine package artifacts. Capture observed artifact SHAs.
- **Close:** mark checkbox in Section 5 **only when its specific native done condition is met**, append time/commit/run to Section 6 and cross-link the owning phase ledger. A green source test alone cannot close an installed, protected or release gate.

## 3. Integration blockers — after immediate fixes

| ID | Dependency | Work | Completion evidence |
| --- | --- | --- | --- |
| **INT-01** | BLK-01/02/03/04/05/06/07, plus active P01–P06 semantic review | Create **one authoritative standalone IDE release-candidate head** from P01 with all *accepted* relevant P02–P06, B2/B3 and late repair commits. Preserve `main` Lu Core and all nonmerged histories, exclude P07/WO10. Review overlapping OS fixture changes, source files and workflow inventories, not filenames only. | Exact tree/PR ancestry report, integrated Rust+Node source tests, Linux/Windows native checks and app build from **same** SHA; no missing B2/B3 features or unverified OS fallback. |
| **INT-02** | INT-01, BLK-08 | Run complete installed-development baseline on **one candidate** (Windows physical acceptance near Lu 1.0 as previously deferred by owner; real GH OS installed runs now). Include installation, project opener, editing, crash/restart, sandboxed build/test/debug and Git, preserved source and clean uninstall. | Document full exact-SHA package SHA + individual OS acceptance transcripts. No claims of full physical-device acceptance from CI-only virtual runners. |
| **INT-03** | INT-01, BLK-07, INT-02 | P06-R2 [#111](https://github.com/vjmichael/Lu-AI-GUI/pull/111) **actual shipped tree** bridge: bind verified NSIS/deb file SHA → independently unpacked payload → real installed file hashes → source/lock/notice candidates. | Exact package/installed hash comparison, allowed intentional differences documented, platform-specific shipping binary and notice inventory. Candidate-only SHA fixture cannot close this. |
| **INT-04** | INT-01/02 and future owner approval | Reduce duplicate active IDE CI/PR operations **only after** original task coverage and independent native equivalence verified (P05 provenance). Establish required branch checks/rulesets for the final branch. | Explicit disposition and source/evidence mapping for every retained historic PR/workflow; no hidden blanket closures or superseded gates. |

## 4. Daily-driver product backlog — execute *after* the blocker tranche

These are needed for a good IDE, but they must not be described as already solved by protected fixed fixtures. Work in this dependency order; components may be designed while native gates finish.

| ID | Priority | Product feature | User-visible acceptance |
| --- | --- | --- | --- |
| **DEV-01** | P1 | **Complete editor and explorer.** New file/folder, rename/move/trash safely, quick-open, tab/split layouts, better keyboard navigation, recovery of draft buffers and large-project performance. Current CodeMirror and explicit save are foundation, not full usability. | User opens real project, creates/edits several files, Find/Replace, switches splits, restarts/crashes without lost saved source; paths and conflicts safely handled. |
| **DEV-02** | P1 · BLK-06 prerequisite | **Real cross-platform LSP.** Promote reviewed clangd/rust-analyzer/Ruff workflows beyond Windows-only feature-gated session; implement reliable automatic diagnostics, completion, hover, symbol navigation/references and actionable problems. Prior bundle manifest labels remain planned until reviewed. | Installed Windows/Linux project-specific Rust/Python/C++ language use with useful features and cancellation; stale source responses never applied. |
| **DEV-03** | P1 · BLK-06/08 prerequisite | **General approved Run/Build/Test.** Replace three fixed Cargo recipes as the only user pathway with bounded project-specific profiles, streaming output, test discovery, clickable diagnostics, cancellation and repeatability. | Run actual Rust then Python/C++/JS projects from installed GUI with reviewed execution scopes and recorded receipts, no shell-by-AI fallback. |
| **DEV-04** | P1 · BLK-06 prerequisite | **Project debugging.** Extend fixed LLDB self-test to approved project binaries; breakpoints, step, variable/watch, call-stack frames and crash recovery. | Installed Win/Linux debug an actual user fixture to breakpoint, inspect state, step/continue, cancel and leave no orphan process. |
| **DEV-05** | P1 | **Everyday Git.** Beyond reviewed one-file staging/commit, add selected hunks, branches/worktrees, meaningful diff/conflict resolution, history/blame and scoped fetch/push/PR workflows. | User can complete a normal local branch/change/commit/review cycle entirely in IDE, with explicit approval for destructive/remote effects and protected credential handling. |
| **DEV-06** | P1 · BLK-06/08 prerequisite | **Contained interactive terminal and logs.** Show executing command, owner, network/FS scope, progress and cancellable process tree; optional terminal-CE, not a required CLI-first product. | Human can inspect terminal output/run approved commands without giving repository code blanket host permissions. |
| **DEV-07** | P1–P2 | **Minimal practical package/toolchain manager.** Detect existing Rust/Cargo, Python/uv, C/C++/CMake, JS/npm tools; preview and approve one project-local environment action with reproducible lock/diff; scale to Conan/vcpkg and further ecosystems later. | Installed GUI can orient to real project dependencies without running install hooks on open; one reviewed per-project action runs, cancels and recovers. |
| **DEV-08** | P1 · DEV-01..07 + INT-02 | **Daily-driver usability and performance pilots.** Rust, Python, C/C++, JS/TS; keyboard/accessibility, no lost drafts, large repo search, measurable load/index/search/build latencies and memory use, cross-platform upgrade/recovery. | Real developer can open → edit → navigate → build/test → debug → inspect Git → commit → restart in installed apps; capture regressions, observed performance and usability failures. |
| **DEV-09** | P2 | **Polish and onboarding.** Reliable single installer, starter projects, configurable panes/keymaps, readable diagnostics and clear disabled-feature explanations; provide documented how-to and recovery paths. | A new user can accomplish the pilot without understanding internal work orders, GitHub CI or manually editing package manifests. |

## 5. Ordered execution checklist

### Group A — red native jobs and bounded protocol/UI failures (first)

- [ ] **A1 / BLK-01:** fix P06-R2 Windows case-collision fixture, obtain same-head Windows+Ubuntu source results.
- [ ] **A2 / BLK-02:** fix B3 Windows PowerShell parser, obtain three-OS source matrix and retain separately passing installed B3 check.
- [ ] **A3 / BLK-03:** debug *real* B2 installed Windows clipboard copy/Undo/Save/restart; complete native install/uninstall.
- [ ] **A4 / BLK-04:** only after A3 green, reconcile B3 onto latest B2; repeat combined native tests.

### Group B — protected OS and installed desktop blockers

- [ ] **B1 / BLK-05:** fix Linux GTK chooser focus/acceptance; full installed editor and rollback/collision matrix.
- [ ] **B2 / BLK-06:** real protected Windows clangd/LLDB and Linux LLDB stop/stack/cancel/kill/restart; no isolation weakening.
- [ ] **B3 / BLK-07:** finish P06-R1 Linux provenance and actual read-only live JSON snapshot; independently review ZIP/source hashes.
- [ ] **B4 / BLK-08:** full Windows/Linux protected installed WO7–WO9 acceptance and actual upgrade/rollback.

### Group C — source convergence and installed software

- [ ] **C1 / INT-01:** reconcile *accepted* draft source into one stable candidate, exclude P07, preserve provenance.
- [ ] **C2 / INT-02:** run full installed candidate pilot on each OS and record exact artifact hashes and failures.
- [ ] **C3 / INT-03:** bind actual NSIS/deb payload → installed files → final shipping inventory.
- [ ] **C4 / INT-04:** review GitHub PR/CI reduction and final branch protections only after equivalent coverage is proved.

### Group D — developer usability and end-to-end workflows

- [ ] **D1:** DEV-01 editor/explorer completeness.
- [ ] **D2:** DEV-02 LSP and DEV-03 general Build/Test.
- [ ] **D3:** DEV-04 project debugger and DEV-06 contained terminal.
- [ ] **D4:** DEV-05 everyday Git and DEV-07 minimal toolchain GUI.
- [ ] **D5:** DEV-08 four-language installed pilots and DEV-09 user-facing polish.

**Practical sequencing:** Groups A and B are acceptance-blocker repair; independent feature implementation in Group D may start without waiting on queued jobs, but promotion to installed acceptance must follow the resolved OS/security prerequisites. Group C should be repeated after major Group D source additions—not a one-time claim of final-release readiness.

## 6. Execution and evidence log

Append rows only after actual inspected work. Record all failed reruns as well as passes.

| Date (UTC) | Task | Source SHA / owning PR | Exact run, OS and job | Outcome, evidence, next step |
| --- | --- | --- | --- | --- |
| 2026-11-08 | BLK-01 | `3feeff4829` / #111 | [42445698778](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42445698778), Windows2022 | Native source test failed case-only collision fixture on case-insensitive filesystem. Ubuntu24 queued. **Repair test without weakening actual collision guard.** |
| 2026-11-08 | BLK-02 | `3edc7e4ee2` / #100 | [42443088413](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443088413), Windows2022 | PowerShell `Unexpected token ')'`; Ubuntu26 passed, Ubuntu24 queued. Separate [B3 installed Windows 42443088407](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443088407) passed. |
| 2026-11-08 | BLK-03 | `297e33d33b` / #99 | [42443379591](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443379591), installed Win2022 | `installed_draft_reference_clipboard_not_verified` after earlier GUI/open/save/restart checks. Still OPEN. |
| 2026-11-08 | BLK-05 | `8da7ae967d` / #104 and `6fc9932074` / #102 | [42442157892](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42442157892), Linux26 GTK | `native_folder_chooser_not_accepted` after `active_window_unavailable`; actual dpkg failed-upgrade rollback passed within job. |
| 2026-11-08 | BLK-06 | `dde157e82f` / #97 | [42437278814](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42437278814), protected Win/Linux | Genuine LLDB-DAP native failure; subsequent #104 real WO6 run [42442157923](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42442157923) pending/no jobs at inspection. |
| 2026-11-08 | BLK-07 | `4f21e93f4e` / #108 | [42444691324](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42444691324), Windows2022 and Ubuntu24 | Windows source ZIP pass; six/six remote core notice bytes match. Ubuntu24 queued; [observer 42444691336](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42444691336) queued/no current JSON. |

| 2026-11-08 | BLK-01 repair | [`455e51ebcb`](https://github.com/vjmichael/Lu-AI-GUI/commit/455e51ebcb3a56302111de8c6e324079686051f4) / #111 | [new exact-head matrix 42447297702](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42447297702), Win2022 / Ubuntu24 | **SOURCE-IMPLEMENTED / OPEN-PENDING.** Real distinct Unicode sharp-s/ss names replace ASCII NTFS-aliasing test fixture, Python casefold collision still denied; no production guard change. Native jobs queued at inspection. |
| 2026-11-08 | BLK-02 repair | [`b8a8c36f57`](https://github.com/vjmichael/Lu-AI-GUI/commit/b8a8c36f570fc0659c9f279c68de0caf857ce083) / #100 | Source run [42443088413](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443088413) original failure; [new 3-OS source 42447314284](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42447314284) and [installed Win2022 42447314275](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42447314275), both queued | **SOURCE-IMPLEMENTED / OPEN-PENDING.** Windows used pwsh to parse POSIX `case/esac`; select `shell: bash` for fixed-scope witness. Installed B3 [42443088407](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443088407) passed at **earlier** head. New head full 3-OS source + installed needed. |
| 2026-11-08 | BLK-03 attempt 1 | [`ed4e2d5875`](https://github.com/vjmichael/Lu-AI-GUI/commit/ed4e2d587593594f999b2991048bb29ef5023bee) / #99 | Original installed Windows failure [42443379591](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443379591); [new exact-head installed Win2022 42447436220](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42447436220) queued | **SOURCE-IMPLEMENTED / OPEN-PENDING.** UIA InvokePattern may lack clipboard transient user activation. Dedicated owned-WebView UIA-focus + real OS Enter action replaces copy control invokes, preserves actual OS clipboard verification and independent disk/restart checks. Adds bounded installed error diagnostics and source guard. Root cause unproven until next genuine installed run. |

## 7. Owner decisions and explicit holds

- **Defer until end of development:** publisher's final legal/redistribution review, performed against the verified *final shipped binaries* and notices. Continue maintaining existing license risk inventory now. Neither zero missing notices nor a passing test grants additional redistribution rights.
- **Defer without blocker:** Windows Authenticode purchase/identity and Linux packaging/signing identity for the unsigned development pilot. No fake signature or production-release approval. GPG key for commits is distinct from a release-distribution trust chain.
- **Defer until Lu Core v1.0:** P07/WO10 authenticated workspace/DLC interface and Lu self-development bridge. Don't resurrect speculative IPC/AF_UNIX integration in this blocker cycle.
- **Windows physical installed acceptance:** keep separate from hosted GitHub Windows runner evidence, planned closer to Lu v1.0. No claim of physical GPU/user installation from CI jobs.
- **User experience aim:** one practical non-CLI-first installer, independent IDE that opens a real project and reliably edits, builds, debugs and commits before the Lu AI bridge is required.

**Next task to execute: A1 / BLK-01 (P06-R2 Windows casefold native fixture), followed by A2 / BLK-02 and A3 / BLK-03.** If A1's OS runner is still queued, proceed on independent A2 rather than pretending A1 passed or doing idle CI polling.
