# Lu IDE — Release Closeout Phase Ledger (P01–P07)

**Established:** 2026-11-07 · **Owning project:** standalone `ide/` in `vjmichael/Lu-AI-GUI`.  
**Policy:** A source commit, unit test, or CI start is not installed-app/native OS acceptance. Exact SHA, OS evidence, and independent verification are required. Do not quietly promote unknown, skipped or failed evidence.

**Authoritative source before P01 consolidation:** `ide/wo9-advanced-workflows-20261107` @ `0d271606c8bcaeabe799cb970e0b6d4aefd25089` (draft [WO9 PR #75](https://github.com/vjmichael/Lu-AI-GUI/pull/75)).  
**Current core main at planning:** `88ab1f10e1b1a4d6c6187bfb5ad7c5d8a1606e39` (must recheck before changes).  
**WO10 exclusion:** `ide/wo10-optional-lu-bridge-20261107` and draft [PR #76](https://github.com/vjmichael/Lu-AI-GUI/pull/76) are **DEFERRED until after Lu Core v1.0**; no Lu bridge, peer pairing, AI model sharing, or coding chat integration in the release-candidate baseline.

## Phase ownership and gates

| Phase | Goal | Current state | Acceptance / exit gate |
| --- | --- | --- | --- |
| **P01 — Authoritative baseline** | Reconcile current `main` and the WO9 standalone IDE source, establish *one* release-candidate branch, review historical draft PR dependencies and possible unported files; explicitly exclude WO10 | **SOURCE BASELINE COMPLETE** — [draft PR #78](https://github.com/vjmichael/Lu-AI-GUI/pull/78), with retained exceptions E01–E05 | Candidate branch and draft review PR exist; main Lu Core untouched; source provenance and divergence inventory recorded; candidate not promoted or merged before source/integration validation |
| **P02 — Deterministic native build and test repair** | Resolve frontend/icon ordering, JavaScript allowlists, Linux AT-SPI helper arguments, Windows accessibility/selector mismatch; exact-head Win/Linux CI | **SOURCE REPAIR IN PROGRESS — exact-head CI/GUI pending** | Repeatable locked source build; native tests and OS GUI automation scripts pass at the candidate exact SHA, with real failure logs reviewed |
| **P03 — WO4–WO6 native security acceptance** | Linux cgroup/Bubblewrap, Windows AppContainer/Job/SCM, protected clangd, native LLDB-DAP, protected cancellation and durable host/service crash recovery | **SOURCE REPAIRS IN PROGRESS — exact-head OS acceptance open** | Independent actual OS denial, normal operation, process-tree kill, kernel containment and reopened durable receipts; no weaker fallback |
| **P04 — WO7–WO9 installed acceptance** | Signed Windows NSIS and Linux Debian package, authentic release dependencies/notices, native install/edit/Git/ZIP/build/test/debug/restart/uninstall/upgrade/rollback | **IN PROGRESS — native source harnesses; OS release gates open** | Installed signed artifact hashes, reviewed licenses/dependencies, native per-OS fixtures with exact receipts; release manifests independently accepted |
| **P05 — GitHub operations reduction** | Review older stacked and parallel IDE PRs; preserve all unique code; close/retarget only after verified consolidation; remove redundant CI without losing required security gates | NOT STARTED | Account for every IDE PR and corresponding source/evidence, document why superseded or retained; no history loss or premature mass closure |
| **P06 — Governance and reproducibility** | Lockfiles, dependency review, rulesets and required checks, artifact provenance/retention, single machine-readable native acceptance matrix | NOT STARTED | Reproducible exact-head packages; trusted release evidence; documented mandatory branch checks; no unreviewed promotion |
| **P07 — Lu ↔ DLC interface (post-v1.0)** | Reevaluate shared Lu Core, Cognitum, NVP and all CE/Workspace interfaces, IPC/AF_UNIX/Protobuf/native logic placement, then reopen WO10 if instructed | **DEFERRED** | **Lu Core v1.0 complete + cross-DLC architecture review + explicit user restart authorization**; no integration now |

**P01 completion checkpoint:** Draft [PR #78](https://github.com/vjmichael/Lu-AI-GUI/pull/78) now contains an independently checked `main`+IDE source snapshot, with WO10 excluded and exceptions E01–E05 retained in the [P01 source audit](IDE_P01_INTEGRATION_AUDIT_20261107.md). **P01 source consolidation is complete; P02–P06 native/release acceptance remains open.**

**Stop rule:** By explicit owner direction P03 can advance while recorded P02 verification remains open; do not upgrade P02 to PASS. Work on phases sequentially. Report what changed, which subtasks remain, and whether the next phase is safe. Do **not** begin the next phase automatically after closing the current one.

## P01 — Work checklist

- [x] **P01-A** — Pin `main` and WO9 exact source heads; compare root trees and preserve current Lu Core/Desktop, Lu documentation, GitHub governance files and Lu CI.
- [x] **P01-B** — Create one clearly named `ide/` release-candidate branch based on the current `main`; import the last nondeferred IDE source tree without WO10. Keep `main` untouched.
- [x] **P01-C** — Preserve original Lu `.github/workflows` plus the existing IDE workflows without overwriting either; commit traceable source identities and open a **draft** integration PR.
- [x] **P01-D** — Audit the complete IDE PR dependency chain; identify diverged, older independent work not yet included in the candidate. Do not mark them merged just because the newest stacked branch compiles.
- [x] **P01-E** — Verify the candidate tree’s source hashes and absence of WO10 bridge code; record unresolved source coverage and CI status honestly.
- [x] **P01-F** — Summarize remaining blockers. Treat P01 as complete only when unported material is explicitly reconciled or tracked for subsequent phase work with an approved disposition.

### Source comparison findings (reconciled against draft PR #78)

- `main` does not contain `ide/`. `main` and WO9 have matching `lu-desktop` and `src-tauri` tree hashes, so importing `ide/` must not replace Lu Core.
- `main` has `AGENTS.md`, root `CHANGELOG.md`, `scripts/`, updated `docs/`, and `.github/pull_request_template.md` absent or older in WO9. **Preserve `main` versions**.
- The `main` workflow set includes `lu-development-ledger.yml`; WO9 includes 32 `ide-*.yml` workflows and no filename collision. Preserve both until P05 assesses reduction.
- Independent early PRs include [#43 — buffer identity](https://github.com/vjmichael/Lu-AI-GUI/pull/43), [#45 — installed buffer test](https://github.com/vjmichael/Lu-AI-GUI/pull/45), [#46 — Find in Project](https://github.com/vjmichael/Lu-AI-GUI/pull/46). They **diverged** from WO9 (respectively 30, 35, 32 unique commits reported by compare); `ide/app/web/buffer_identity.js`, `ide/docs/IDE_BUFFER_REFERENCES.md`, and `ide/host/src/project_search.rs` are **absent** in WO9. Do not silently declare these optional/parallel features carried forward. Investigate their relation to modern editor and source browser; preserve any unique accepted behavior before retiring PRs.
- Some intermediate work branches diverged by one or two post-stack commits, e.g. C3, D3, D6, D7, WO6–WO8. Verify whether those are tests, documentation or missing code before historic PR closure.
- The latest completed WO9-head CI `0d271606` ([native](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42350479148), [installed](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42350479273)) was **red**: Tauri asset-build ordering and JS allowlists, installed native GUI missing accessible controls. A release-candidate import does not erase those failures.

## P02 — Deterministic repair checklist (source repairs committed; see [P02 repair record](IDE_P02_DETERMINISTIC_REPAIR_20261107.md))

- [x] Repair WO9 asset-generation ordering; add exact-head Windows/Linux matrix for default, `native-build`, `native-lsp`, `native-debug` and combination. **CI verification pending.**
- [x] Repair two strict frontend contract assertions for existing privileged native receipt verifier without expanding any capability or permission grant. **Node test verification pending.**
- [x] Correct Linux WO8 folder picker plus WO9 AT-SPI `activate(timeout=...)` call signatures, with an AST regression test. [ ] Run installed GTK/WebKit real interaction and record the result.
- [x] Expose explicit accessible `Execution requests` heading; on a missing native Windows UIA control, record a bounded actual accessibility-tree snapshot while continuing to fail closed. [ ] Prove the installed UI interactions really work and repair any OS-verified selector/control failure.
- [ ] Freeze P02 exact SHA, run all Windows/Linux native/installed checks, inspect real failures and repair them; do not extrapolate from prior heads.

## P03 — Native security checklist (started by owner after P02 source work; see [P03 native security evidence](IDE_P03_NATIVE_SECURITY_ACCEPTANCE_20261107.md))

- [x] Implement exact systemd delegated native WO4 child/grandchild quota + cgroup.kill + `populated 0` witness and independent hostile Bwrap network/PID namespace/read-only workspace CI. [ ] Observe both actual exact-head kernel positive tests and crash-safe/rejected-host cases; compiling a witness is not acceptance.
- [x] Windows nonidentifying loader/access categories; Linux bundle/root ownership categories and real prior-head kernel/LSP initialize observed. [x] Add debug-only bounded Linux clangd stderr/OOM/PID-limit witness. [ ] Resolve protected Windows `clangd` exit 1; native Linux hover/definition/completion/diagnostics, cancellation and restart PASSED on earlier P03 SHA in E2 run 42391111181; final exact-head revalidation remains required.
- [x] Repair DAP initialize/launch/configurationDone sequencing with strict negative tests; fix old supervisor test order. [x] Preserve kernel ASLR with fixed LLDB-DAP `disableASLR=false` from upstream recommended sandbox behavior. [x] Add bounded Windows missing-DLL preflight and explicit fixed binary PE direct-import audit. [ ] Prove Windows DLL package closure plus actual AppContainer debuggee and Linux fixed launch/stop/thread/stack/cancel on current exact head.
- [ ] Independent protected Cargo/run/test/LSP/DAP normal flow, explicit authorization, child reaping, SCM/service crash, restart, durable quarantine and no silent replay; Windows real D7D crash/recovery/Cargo passed on run 42387149941 and Linux debugger host-crash/replay passed on WO6 42391111210; native debugger normal/cancel and final exact-head gates remain OPEN.

## P04 — Installed release checklist (started; see [P04 release/installed evidence](IDE_P04_INSTALLED_RELEASE_CLOSEOUT_20261107.md))

- [ ] WO7 independently reviewed signer, signed desktop/service/installer, PE dependency audit and licenses; manifest currently `releaseApproved: false`.
- [ ] WO8 genuine Windows NSIS UI native picker, editing/unsaved draft, external-file conflict, save/reopen/restart and clean uninstall; Linux Debian X11/GTK/AT-SPI equivalents.
- [ ] WO9 reviewed local Git only, verified portable ZIP and fingerprinted multi-project DAG, per-step protected consent, cancel/crash and dependency block.
- [ ] Reinstall/upgrade state migration and rollback, not just fresh install. No feature or release admission from source-only test harness.
- [ ] Verified independent OS artifact chain and exact SHA before moving release evidence manifests out of default-deny.

**P04 implementation checkpoint:** Signed-only native Windows reinstall and registered-project catalog preservation witness, source-only native PowerShell parser/negative migration checks, cross-version rollback evidence gate, and corrected WO7 desktop signer+hash inputs are committed on the P04 branch. Real WO8/WO9 installed Windows WebView UIA and Linux AT-SPI tests are failing as documented in the P04 closeout report. No signed candidate/signer, actual upgrade/rollback or release acceptance. P03 remains open independently. P05 has not started.

## P05 — GitHub PR/CI checklist (not started)

- [ ] Inventory all 26 IDE-specific open PRs (among 65 open shared Lu PRs at audit); compare source blob hashes and unique file/behavior coverage against candidate.
- [ ] Review branch bases and stacked parents; explicitly retarget, close or preserve individual PRs after their changes have been independently accounted for. Keep WO10 draft unmerged.
- [ ] Limit CI triggers on superseded IDE draft branches only after P02–P04 gates have an accepted replacement; never disable vital containment or license checks to make a build green.

## P06 — Governance checklist (not started)

- [ ] Pin Rust and npm dependency graphs for all IDE crates/features, reproducibly build on both OSs and verify license notices.
- [ ] Require CI checks and human approval before promotion; protect source/RC/release branches. Verify repository rulesets and permissions directly.
- [ ] Exact head + OS + signed payload + installed process + kernel witness + durable receipt in machine-readable acceptance; never infer from source or skipped CI.
- [ ] Retain trusted release artifacts, independently verify provenance and publish rollback/uninstall support.

## P07 — Deferred until after Lu Core v1.0

No implementation or PR merge permitted. Evaluate shared host/CE/Workspace contract with Cognitum and NVP and user approval after Lu Core v1.0, including native AF_UNIX where suitable and Windows parity, typed protocols, logic pathways and permission lifecycle. WO10 PR #76 remains explicitly **DO NOT MERGE**.

## Operational notes

- GitHub is a repository and CI evidence source, not an alternative development authority or automatic release signer.
- Keep the standalone IDE independent of Lu Core until post-v1.0 review. No Docker dependency, no automatic code execution on project open.
- Source hash and negative-test status are recorded separately from real installed acceptance. Lack of evidence is `PENDING`, not `PASS`.
- The candidate branch is a *staging/integration source of truth*, not an approved signed release.
