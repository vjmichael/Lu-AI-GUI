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
| **P05 — GitHub operations reduction** | Audit all earlier IDE PRs and CI, preserve unique work, decide safe operations without weakening native gates | **SOURCE/REVIEW COMPLETE — all retain decisions documented; exact-head CI and P03/P04 dependent cleanup pending** | Account for every IDE PR and corresponding source/evidence, document why superseded or retained; no history loss or premature mass closure |
| **P06 — Governance and reproducibility** | Lockfiles, dependency review, rulesets and required checks, artifact provenance/retention, single machine-readable native acceptance matrix | **SOURCE STARTED — actual lock receipts and exact-head CI pending** | Reproducible exact-head packages; trusted release evidence; documented mandatory branch checks; no unreviewed promotion |
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

**P04 implementation checkpoint (recorded before P05 started):** Signed-only Windows NSIS clean reinstall, separate two-version sequential replacement/injected recovery, and independently GPG-signed Linux Debian two-version native GUI acceptance workflows are committed **as test infrastructure**. Release dependencies now require exact reviewed DLL hashes and notice inventory; actual Windows Job requests no-project React render crash repaired with tests, and Linux GTK folder chooser must finish real native dialog confirmation. Native installed Win/Linux tests still require exact-head passes; no signing keys, signed candidates, true broken-installer rollback or release acceptance have been supplied. P03 remains open; P05 has not started. See the P04 closeout report. The new P04 license collector and native rollback engine now ship a bundled unsigned verification notice inventory, signed release legal-review preflight, and isolated crash-recoverable retained-version transaction tests; a real unsigned Linux dpkg postinst failure/old-version restore test is integrated into installed Linux CI, **all subject to native results**.

## P05 — GitHub PR/CI consolidation (source audit committed; see [P05 source and CI report](IDE_P05_PR_CI_CONSOLIDATION_20261107.md))

- [x] **P05-A — Complete exact GitHub inventory:** Current GitHub returned **78 open PRs**, of which **30 are IDE branches** (older planning snapshot was 26). Pin each open IDE PR number, source SHA, base, historical IDE file count, paths absent from P04 snapshot and differing existing blob count. **Exact snapshot:** `bc4fd78685c342675e4a80556970e934125a0e20`.
- [x] **P05-B — Retain genuine nonported exceptions:** #43 buffer identity, #45 installed-buffer acceptance, #46 Find in Project each have missing source/test files; preserve all original blob identities, retain drafts and plan explicit semantic port or defer. #76 WO10 bridge has nine deliberately omitted paths; **defer until after Lu Core v1.0** and never silently merge.
- [x] **P05-C — Preserve CI until accepted replacement:** All **38 `.github/workflows/ide-*.yml`** at the exact candidate head were inventoried, Git blob SHA-pinned and retained. Build a source validator and Windows/Linux negative tests refusing premature PR closures, fake native acceptance, CI file changes/deletion and WO10 import. **Final exact-head CI verification pending.**
- [x] **P05-D — Historical semantic/source reconciliation:** Audited 15 actual WO9 ancestors, seven divergent late patchsets with current source witnesses and full PR #70 prior-head WO6 evidence appendix, retained three B2/B3 unported feature PRs and excluded WO10. Runtime/native equivalence still requires independently passing P03/P04 OS gates.
- [x] **P05-E — Deliberate GitHub disposition:** Reviewed all individual old PRs and **decided to retain** pending required P02–P04 native/release replacement evidence. No candidate can be honestly retired yet. **Authorized PR closures: zero; actual closure operations intentionally blocked until accepted replacements.**
- [x] **P05-F — CI reduction disposition:** Evaluated all **38** workflows and assigned a machine-reviewed P02/P03/P04 retention owner. None has independently accepted equivalent native/installed/signing coverage yet; preserve all, including source and manual signing gates. **Current authorized workflow removals and trigger reductions: zero.** Revisit the actual administrative deletion only after native acceptance.

**P05 status:** Source and review work completed with 30 PR decisions, 38 workflow decisions and explicit blocking of unsafe operations. Exact-head Windows/Linux source integrity CI remains pending; actual PR closures and CI reduction remain blocked by prior native phase gates. Draft branch `ide/release-closeout-p05-20261107` is stacked on P04 draft #84. P03/P04 remain draft/unmerged; P06 unstarted; P07 still deferred. No PR closed, retargeted, merged or security CI disabled.


**P05 exact-head follow-up (Nov 7, 2026):** Windows and Linux source/semantic/retention audit passed for commit `85cf29180cf14cc33178429c4e9699966e5b4051` in [run 42414577131](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42414577131). Two independent Windows E2 unsigned installed runs on that SHA then failed **before NSIS compilation**: Cargo's loose `tauri = 2.11.5` selected Rust `2.12.5`, conflicting with pinned JS API `2.11.1`. Changed native Cargo declaration to exact `=2.11.5` and introduced eight negative/positive Tauri alignment regression cases, preserving full installed smoke steps. This newer source head requires new actual Windows/Linux verification; neither installer run is accepted yet. Release still default-deny.

## P06 — Governance checklist (source phase active; native release not accepted)

- [x] Implement fail-closed exact-head Rust/npm lock resolution, SHA provenance and upstream license bundling on Windows/Linux CI. [ ] Inspect real receipts, commit approved lockfiles, apply locked builds on every OS and verify actual reproducibility.
- [ ] Require CI checks and human approval before promotion; protect source/RC/release branches. Verify repository rulesets and permissions directly.
- [x] Add machine-readable default-deny P02–P07 acceptance matrix and hostile tests. [ ] Bind exact head + OS + signed payload + installed process + kernel witness + durable receipt to independently accepted release evidence.
- [ ] Retain trusted release artifacts, independently verify provenance and publish rollback/uninstall support.

## P07 — Deferred until after Lu Core v1.0

No implementation or PR merge permitted. Evaluate shared host/CE/Workspace contract with Cognitum and NVP and user approval after Lu Core v1.0, including native AF_UNIX where suitable and Windows parity, typed protocols, logic pathways and permission lifecycle. WO10 PR #76 remains explicitly **DO NOT MERGE**.

## Operational notes

- GitHub is a repository and CI evidence source, not an alternative development authority or automatic release signer.
- Keep the standalone IDE independent of Lu Core until post-v1.0 review. No Docker dependency, no automatic code execution on project open.
- Source hash and negative-test status are recorded separately from real installed acceptance. Lack of evidence is `PENDING`, not `PASS`.
- The candidate branch is a *staging/integration source of truth*, not an approved signed release.

## Nov 7 continuation: actual transitive Tauri native failure and upstream notice recovery

- Exact-head OS builds at `9d28f22` reached the Rust native compiler and failed due **Tauri 2.11.5 ↔ 2.12.x runtime family skew**, beyond the original JavaScript version check. Aligned exact Rust `tauri = "=2.12.5"`, JS API `2.12.4`, CLI `2.12.4`; added resolved Cargo.lock runtime-family guard and adversarial tests. Only a genuine installed OS run can prove the repair.
- User requested upstream licenses in **every verification submission**. Both E2 Windows NSIS workflows now retrieve complete original license texts for LLVM, CPython, Tauri (both MIT and Apache-2.0 at tauri-v2.12.5), CodeMirror, and esbuild directly from their upstream pinned GitHub tags. They compare exact text with the reviewed repo copies (Windows CRLF normalized); unexpected changes, missing bytes, network failure, symlink or oversized responses are denied.
- A source-SHA-bound `upstream-fetch.json` with six URLs, revisions and SHA-256s is copied **into actual `verification-notices/` installer resources** and included in CI artifacts alongside full texts, resolved npm/Cargo notice files and the exact native package SHA receipt. `verify_p04_license_bundle.py --require-upstream` now rejects a missing/mismatched receipt. Additional package legal gaps remain explicitly unresolved and no release/legal approval has been asserted.
- Both prior E2 workflow SHA pins and review history were preserved and updated, the new E2 source blob IDs and 38-workflow retention decisions remain audited. No prior GitHub PR or safety CI retired. Exact-head CI and installed artifact acceptance now pending.

## Native verification package coverage extension — November 7

The upstream notice provenance policy is now implemented beyond both initial E2 Windows smoke jobs in WO8 installed Windows NSIS, WO8 installed Linux Debian and both WO9 installed Windows/Linux CI jobs. Each job fetches all six pinned original upstream licenses and submits actual resolved npm/Cargo notices, a source-SHA-bound provenance record inside the package's `THIRD_PARTY_LICENSES` directory, and an exact package SHA receipt. Each job tests the current Tauri runtime family before compilation and uses the current aligned Tauri 2.12 CLI. Installed Windows/Linux checks require the upstream receipt file in the real native package. CI retains all real installed edit/ZIP/Git/rollback tests. Three historical workflow original SHA identities are preserved separately from the repaired current blob SHA; no original PR/CI retired. Exact-head native results pending. Signed release and upstream legal reviews remain default deny.

**P06 initial implementation checkpoint:** Branch `ide/release-closeout-p06-20261107` on P05 current source, separate exact native Windows/Linux lock provenance workflow and real upstream license notices, 11 hostile source unit tests, default-deny release matrix, and [governance ledger](IDE_P06_GOVERNANCE_REPRODUCIBILITY_20261107.md). No lockfile or native release acceptance inferred; branch-protection API access denied despite empty rulesets list. Do not close P05/P06 until evidence exists.
