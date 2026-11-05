# Lu — Canonical development ledger

> **START HERE for Lu project status.** This file on **`main`** is the authoritative *development index*, not a guarantee that draft code is merged, installed, or accepted. See [CHANGELOG.md](../CHANGELOG.md) for accepted milestones and [v1.0 design](LU_V1_0_DESIGN_RECONCILIATION_20261103.md) for requirements. Historical branch-local task logs and conversations are evidence, **not competing current status boards**.

## 1. Status rules (never collapse these)

| Evidence level | Meaning |
| --- | --- |
| **Designed** | A requirement or interface exists; no runnable implementation is asserted. |
| **Draft source** | Exact code exists on a named branch/PR but may not be present on `main`. |
| **Compiled/tested** | Specific CI checks passed on the exact head SHA; **list failing/skipped jobs too**. |
| **Desktop integrated** | The production Tauri host/UI can reach that implementation at an exact compiled revision; static registration is not sufficient. |
| **Model exercised** | Real local GGUF/model used the tool in a representative authorized task. |
| **Installed accepted** | Actual installed Windows build executed the owner-approved acceptance scenario with recorded binary/source identity. |
| **Released** | An immutable tagged/distributed release with traceable artifacts, separate from development acceptance. |

No branch status, passing isolated test, code comment, diagram, or prior chat can promote an item to another level. Record **what was tested, where, and at which exact SHA**. Mark unknown results unknown. A successful earlier alpha does not confer acceptance on later binaries.

## 2. Accepted reference and current scope

- **Functional development alpha:** `0.1.0-alpha.2`, owner-accepted after physical Windows tests of GGUF/MMProj, chat, persistence, streaming, cancellation, queueing and Retry. See [acceptance record](LU_ALPHA_01_ACCEPTED_BASELINE_20261103.md). It is **not** an accepted native-tools v1.0 build, and its exact installer-to-`main` source equivalence is not asserted here.
- **Repository baseline observed at initial reconciliation (2026-11-05):** `main` SHA `6645f7939b60700c4012d3a2a18b7553480980a6`. This is a reference for the snapshot, **not** a permanent current-main claim.
- **Owner-approved order (2026-11-05):** consolidate and finish **general-purpose native tools** first; then connect ANSF/SQLite, operational memory, task checkpoints and host-owned relevant context assembly; finally complete v1.0 whole-app acceptance.
- **Out of Core scope:** specialized IDE debugging/GitHub workflows, NVP/Film video production, a duplicate research engine, and bespoke video/transcription engines. TOOLS-10 already owns native browser and research. Specialized engines/DLCs can use approved Core supervision later.

## 3. Consolidated feature map — code existence versus acceptance

This is an initial **source-grounded reconciliation of inspected branches**, not a claim that every operation or every branch passed CI. Follow the linked PRs and pinned head snapshots below; revisit this manual feature table whenever a PR is integrated or scope changes.

| Area | Evidence of implementation | Current release/integration classification | Actual remaining work |
| --- | --- | --- | --- |
| **Filesystem (TOOLS-01 / WRITE-01)** | Existing read/list/search, approved create/edit/rollback; test-budget repair [#32](https://github.com/vjmichael/Lu-AI-GUI/pull/32) merged into development [#24](https://github.com/vjmichael/Lu-AI-GUI/pull/24) at `3ceb00c7c60f29a56a9af4a709fb90892fb6ce3d` | **Draft integration**; repaired #32 exact SHA native-core, Windows-host and source-identity passed | Whole Core/desktop branch integration, real-model/installed approval; do not rebuild existing editors |
| **Archive (TOOLS-04)** | Approved ZIP inspection under [#15](https://github.com/vjmichael/Lu-AI-GUI/pull/15), lock repair [#33](https://github.com/vjmichael/Lu-AI-GUI/pull/33) (`7efa41870daf1bb191e30f9196a2bd4a8f99aeb7`) | **Draft source**; actual Cargo-resolved 33-hunk Windows desktop lockfile now committed, source identity + Core tests + lock audit green; Windows-host build pending | Integrate inspect-only archive tool/approval; extraction/creation are separate decisions |
| **Controlled execution** | Core ExecuteCode/typed CeExecute contracts on main; host supervisor from [#27](https://github.com/vjmichael/Lu-AI-GUI/pull/27) now includes one-shot model-start/cancel over a host-prepared reviewed worker plan from [#30](https://github.com/vjmichael/Lu-AI-GUI/pull/30), exact modules ported into its development base | **Native CI at #30 exact head passed Windows/Linux; source port to #27 in progress**, not Tauri model-admission or installed acceptance | Connect authenticated real user-turn scoped grants to job-kind registry; never expose arbitrary shell/executable/PID |
| **Background tasks (TOOLS-11)** | Existing host-run start, cancellation, durable status/progress/event recovery in [#27](https://github.com/vjmichael/Lu-AI-GUI/pull/27); scoped model status/result and host-bound start/cancel ported from [#30](https://github.com/vjmichael/Lu-AI-GUI/pull/30) into #27's base | **#30 exact head Windows/Linux CI passed; combined #27 CI underway**, no installed acceptance or fabricated artifact manifests | Complete Tauri authenticated model tool dispatch and actual verified artifact receipt; run physical Windows |
| **Universal Reader** | Existing text/DOCX/XLSX/ODS/PPTX/ODP/basic PDF in [#27](https://github.com/vjmichael/Lu-AI-GUI/pull/27); bounded nested PDF page-tree addition [#31](https://github.com/vjmichael/Lu-AI-GUI/pull/31) **merged into #27 development branch** at `13a58f0af9af4b651f0c10c1d3d5c31758ee1b8f` | **Source and Windows/Linux native tests passed on #31; combined #27 CI underway**, no installed acceptance | CMaps, scanned PDFs/OCR, parser worker limits, actual Qwen Reader and Windows NSIS tests |
| **Local Git (TOOLS-03)** | Read-only status/log/diff in [#14](https://github.com/vjmichael/Lu-AI-GUI/pull/14) | **Draft source** | Cross-platform integration/authorization; advanced GitHub operations belong to Lu IDE |
| **JSON / validation (TOOLS-02/05)** | Internal JSON boundary/repair [#12](https://github.com/vjmichael/Lu-AI-GUI/pull/12) and separate schema validation [#16](https://github.com/vjmichael/Lu-AI-GUI/pull/16) | **Draft source** | Converge codecs, backend/tool dispatch and end-to-end model checks |
| **Environment / OCR / search** | Environment discovery [#17](https://github.com/vjmichael/Lu-AI-GUI/pull/17), vision-first OCR fallback [#18](https://github.com/vjmichael/Lu-AI-GUI/pull/18), recursive scoped search [#24](https://github.com/vjmichael/Lu-AI-GUI/pull/24) | **Draft source** | Verify exact intended scope, host integration and safe behavior |
| **Loop guard / backend control** | Loop-progress guard [#19](https://github.com/vjmichael/Lu-AI-GUI/pull/19); llama/vLLM finish/cancel design [#20](https://github.com/vjmichael/Lu-AI-GUI/pull/20) | **Draft source / partial design** | Real backend cancellation and exact-head model tests |
| **Browser + research (TOOLS-10)** | Source-backed planning, evidence/citation ledger, sufficiency assessment, native browser worker/consent/recovery in [#26](https://github.com/vjmichael/Lu-AI-GUI/pull/26) | **Draft source**, not yet a packaged accepted browser | Pin/install verified browser runtime; host/model dispatch and physical Windows/Linux acceptance. **Do not create another research toolkit.** |
| **Memory — scheduled second** | ANSF/FTS/revisions and continuity APIs on `main`; Cognitum search bridge [#21](https://github.com/vjmichael/Lu-AI-GUI/pull/21); operational resource ledger [#28](https://github.com/vjmichael/Lu-AI-GUI/pull/28) | **Core components + draft extensions**, no default desktop preflight | After tool integration: connect verified resource probe, cross-chat provenance and task checkpoint/context assembly |
| **AI-usability audit** | Findings and tool-first decision [#29](https://github.com/vjmichael/Lu-AI-GUI/pull/29) | **Draft report**, not implementation acceptance | Use for v1.0 scope/verification and source-reference contracts |

### 2026-11-05 actual closeout work — TOOLS 1–5 (accepted states only when proven)

| Scope | Code branch / exact source head | Verified result | Next unclosed gate |
| --- | --- | --- | --- |
| 1 — Filesystem/search | [PR #32](https://github.com/vjmichael/Lu-AI-GUI/pull/32), `69360585f07d326c300260290cd1d2f05143e5fe` (stacked on #24) | Repaired outdated 300-vs-512-entry truncation test to exceed real budget; **native-core and source identity green**. This changes a regression fixture, not filesystem semantics | Remaining Windows-host/other matrix checks and tool integration; no installed acceptance |
| 2 — Archive | [PR #33](https://github.com/vjmichael/Lu-AI-GUI/pull/33), `c4ada5ac3da9354bcdc26c1fca339b75d6ecd108` (stacked on #15) | Fixed missing `zip` edge in desktop Cargo.lock after inspecting actual [Windows failure](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41895206776/job/126808737558); **source identity green**. No parser rewrite | Exact-head native/Windows build and approved ZIP inspection desktop path; physical Windows acceptance |
| 3 — Controlled execution | On-main CE `CeExecuteRequest/Result` and `ExecuteCode` authorization; [#27](https://github.com/vjmichael/Lu-AI-GUI/pull/27) host worker | Existing source/contract verified by inspection only **in this closeout** | Approved typed job-kind model/host operation, no arbitrary shell, end-to-end tested execution receipts; no new general exec advertised |
| 4 — Background tasks | [PR #30](https://github.com/vjmichael/Lu-AI-GUI/pull/30), `aec0ac378dcb1cf5b42be75f2d19e2bb819bc008` (stacked on #27) | **Windows and Ubuntu scoped task-status/receipt matrix passed**; real native status/result are model-schema-advertisable, workspace-gated, with no private checkpoint/artifact claims | Host-owned authorized start/cancel model route, actual task receipt/artifact verification and physical Windows acceptance |
| 5 — Universal Reader | [PR #31](https://github.com/vjmichael/Lu-AI-GUI/pull/31), `9b4269badd49a66a5229ee6b2501c96150faa002` (stacked on #27) | **Windows and Ubuntu Reader matrix passed** for bounded nested classic-PDF tree traversal and existing Reader module regressions | Real-producer/CMap/scanned PDF/OCR/parsing hardening, model+installed desktop acceptance |

**Limitations:** Original feature PRs remain draft. Reader #31 and search #32 were subsequently **merged into their existing development base branches only**, not main. Background #30 code was ported byte-identically into #27's development branch, with whole-base CI underway. CI successes above are exact-commit observations, not evidence these five coexist in one shippable Lu desktop. This table is the current source-of-truth handoff; do not reopen finished implementations or count the old archive lock issue as a missing Windows Job method. Broad integration and actual physical Windows v1.0 tests remain separate gates.

### 2026-11-05 tool closeout integration receipts

- **Reader #31**: merged into development PR #27 at `13a58f0af9af4b651f0c10c1d3d5c31758ee1b8f` after exact-head Windows+Linux native pass. This is **not** a merge to `main`.
- **Background/controlled execution #30**: original commit `d16d681890e309531bc508a260cf4b76b760cf24` passed native Windows+Linux; both added Rust modules were copied **byte-identically** into the existing Reader/background #27 development branch and registered in Core/workflow at `5fef956d6a81fb2c9843e116465847fa1251f690`. Combined-source Linux CI succeeded; Windows CI underway. The #30 PR remains open pending final consolidation receipt.
- **Filesystem/search #32**: exact `69360585f07d326c300260290cd1d2f05143e5fe` native-core, Windows-host, source identity and approved-dispatch tests passed; merged to development PR #24 at `3ceb00c7c60f29a56a9af4a709fb90892fb6ce3d`, not `main`.
- **Archive #33**: generated the real Windows Cargo.lock difference via a fail-closed diagnostic, applied exactly 33 hunks, and confirmed the resulting Git blob `4314c65c2758f1d68cc84aedb60a47a438c41bd8` matches the runner's expected regenerated lockfile. Exact branch source revision `7efa41870daf1bb191e30f9196a2bd4a8f99aeb7`; Cargo.lock check, source identity, Core and portable tests green; full Windows-host still in progress.
- **No release promotion:** none of the above proves all 5 tool families are merged into one installer, or that GGUF models execute them in a tested installed Lu desktop.

### Recorded selective CI evidence (not exhaustive)

- [#24 head `b5face4d0abed8ccc56411e068bf420fe8b7b87a`](https://github.com/vjmichael/Lu-AI-GUI/commit/b5face4d0abed8ccc56411e068bf420fe8b7b87a): `windows-host` succeeded; `native-core` failed. **No whole-app pass.**
- [#15 head `f1caf91ef88beacb063c0c6348035ad12b0e0a6e`](https://github.com/vjmichael/Lu-AI-GUI/commit/f1caf91ef88beacb063c0c6348035ad12b0e0a6e): ZIP-specific Windows/Linux checks succeeded; `windows-host` failed. **Do not call ZIP desktop integrated.**
- [#27 head `c716876aed0276e98cb372ccd2cd16c916c5c00d`](https://github.com/vjmichael/Lu-AI-GUI/commit/c716876aed0276e98cb372ccd2cd16c916c5c00d): scoped task-status checks succeeded on Windows and Ubuntu; **no verified full Tauri/installed acceptance**.
- Other PR-specific CI/physical results were **not exhaustively checked for this snapshot**. The source branch's own documentation and exact GitHub run are evidence only for the executed scope.

## 4. Active PRs and branch topology (machine-refreshable)

**The table below is only an inventory of open GitHub PR metadata.** Draft/open does not mean code is merged, accepted, or verified; closed does not necessarily mean merged. The explicit **base** column exposes stacked branches. Keep this inventory on `main` and refresh it with the repository script rather than asking each conversation to reconstruct all branches.

<!-- LU_OPEN_PRS_START -->
_This section is a GitHub metadata snapshot, not proof that an implementation is integrated. Refresh with `node scripts/lu-ledger.mjs --update`. Open PR count: **23**._

| PR | Proposal / scope | Head branch | Exact head | PR base | Review state |
| --- | --- | --- | --- | --- | --- |
| [#33](https://github.com/vjmichael/Lu-AI-GUI/pull/33) | TOOLS-04 closeout: reconcile desktop Cargo.lock for ZIP native dependency | `lu/tools-04-desktop-lock-repair-20261105` | [`7efa41870daf`](https://github.com/vjmichael/Lu-AI-GUI/commit/7efa41870daf1bb191e30f9196a2bd4a8f99aeb7) | `lu/tools-04-archive-inspection-20261104` | Draft |
| [#30](https://github.com/vjmichael/Lu-AI-GUI/pull/30) | TOOLS-11: scoped task status/results plus host-bound one-shot start/cancel | `lu/tools-11-model-observation-20261105` | [`d16d681890e3`](https://github.com/vjmichael/Lu-AI-GUI/commit/d16d681890e309531bc508a260cf4b76b760cf24) | `lu/tools-11-background-task-status-20261104` | Draft |
| [#29](https://github.com/vjmichael/Lu-AI-GUI/pull/29) | LU-AIW-AUDIT-01: foundational AI-usability audit across Lu Core, desktop and draft PRs | `lu/aiw-foundational-audit-20261105` | [`7d1f965d10b4`](https://github.com/vjmichael/Lu-AI-GUI/commit/7d1f965d10b407851fa5ddec2f059b806a9d10df) | `main` | Draft |
| [#28](https://github.com/vjmichael/Lu-AI-GUI/pull/28) | LU-OPMEM-01A: durable operational resource memory and AI repo orientation | `lu/aiw-operational-memory-20261105` | [`c67d5f7abdaa`](https://github.com/vjmichael/Lu-AI-GUI/commit/c67d5f7abdaa1427102d0c9ad8ae98f19cf0bc5a) | `main` | Draft |
| [#27](https://github.com/vjmichael/Lu-AI-GUI/pull/27) | TOOLS-11 + universal Reader: DOCX, XLSX/ODS, PPTX/ODP, basic PDF, paged selections | `lu/tools-11-background-task-status-20261104` | [`5fef956d6a81`](https://github.com/vjmichael/Lu-AI-GUI/commit/5fef956d6a81fb2c9843e116465847fa1251f690) | `main` | Draft |
| [#26](https://github.com/vjmichael/Lu-AI-GUI/pull/26) | TOOLS-10A–E source implementation: verifiable research, browser worker and durable recovery | `lu/tools-10-citation-integrity-20261104` | [`19af8fe3e411`](https://github.com/vjmichael/Lu-AI-GUI/commit/19af8fe3e411e4a4175e285ce1c6c2130c36fe4d) | `lu/tools-03-git-inspection-20261104` | Draft |
| [#24](https://github.com/vjmichael/Lu-AI-GUI/pull/24) | TOOLS-09: extend existing scoped search with bounded recursion and suffix filters | `lu/tools-09-recursive-search-20261104` | [`3ceb00c7c60f`](https://github.com/vjmichael/Lu-AI-GUI/commit/3ceb00c7c60f29a56a9af4a709fb90892fb6ce3d) | `main` | Draft |
| [#22](https://github.com/vjmichael/Lu-AI-GUI/pull/22) | CI-only replay: TOOLS-04 archive exact-head Windows/Linux acceptance | `lu/tools-04-archive-ci-replay-20261104` | [`cb04fe12b75f`](https://github.com/vjmichael/Lu-AI-GUI/commit/cb04fe12b75fd75334049cd78f41bfbb135032cf) | `lu/tools-03-git-inspection-20261104` | Draft |
| [#21](https://github.com/vjmichael/Lu-AI-GUI/pull/21) | Lu ANSF: host-authorized Cognitum C06 retrieval via existing Rust FTS5 | `lu/c06-c07-ansf-fts5-bridge-20261104` | [`5afe669aa9a4`](https://github.com/vjmichael/Lu-AI-GUI/commit/5afe669aa9a43dfe30cba11c583f1649f0a39ae3) | `main` | Draft |
| [#20](https://github.com/vjmichael/Lu-AI-GUI/pull/20) | BACKEND-CTRL-01: llama.cpp finish semantics and llama/vLLM control design | `lu/backend-control-cancellation-design-20261104` | [`ed24da5d0004`](https://github.com/vjmichael/Lu-AI-GUI/commit/ed24da5d00047f846989d9fc54fd0c4939e980bd) | `lu/tools-08-loop-guard-20261104` | Draft |
| [#19](https://github.com/vjmichael/Lu-AI-GUI/pull/19) | TOOLS-08: verified progress guard and streaming token-loop stop (no 8-call ceiling) | `lu/tools-08-loop-guard-20261104` | [`62992c9ac20b`](https://github.com/vjmichael/Lu-AI-GUI/commit/62992c9ac20bed96e815ba732c94ebf186899a1e) | `lu/tools-02-json-validation-20261103` | Draft |
| [#18](https://github.com/vjmichael/Lu-AI-GUI/pull/18) | TOOLS-07: original vision-first image read with internal Tesseract OCR fallback | `lu/tools-07-ocr-fallback-20261104` | [`ad7bf9f48371`](https://github.com/vjmichael/Lu-AI-GUI/commit/ad7bf9f483713de1e94d51205041a9cd500ba606) | `lu/tools-02-json-validation-20261103` | Draft |
| [#17](https://github.com/vjmichael/Lu-AI-GUI/pull/17) | TOOLS-06: Jan §6A ambient environment discovery and scoped native check_env | `lu/tools-06-environment-discovery-20261104` | [`a3dca4b968cf`](https://github.com/vjmichael/Lu-AI-GUI/commit/a3dca4b968cf72a4e699da3311d47035de5145d2) | `lu/tools-02-json-validation-20261103` | Draft |
| [#16](https://github.com/vjmichael/Lu-AI-GUI/pull/16) | TOOLS-05: native offline JSON Schema validation (original Jan validate domain) | `lu/tools-05-schema-validation-20261104` | [`7a77bbdb8aed`](https://github.com/vjmichael/Lu-AI-GUI/commit/7a77bbdb8aed3bf46f4ed68f87e940761c240d67) | `lu/tools-02-json-validation-20261103` | Draft |
| [#15](https://github.com/vjmichael/Lu-AI-GUI/pull/15) | TOOLS-04: original-first native ZIP manifest inspection (read-only) | `lu/tools-04-archive-inspection-20261104` | [`f1caf91ef88b`](https://github.com/vjmichael/Lu-AI-GUI/commit/f1caf91ef88beacb063c0c6348035ad12b0e0a6e) | `lu/tools-03-git-inspection-20261104` | Draft |
| [#14](https://github.com/vjmichael/Lu-AI-GUI/pull/14) | TOOLS-03: basic read-only Git Core + original tool design; advanced gh reserved for IDE | `lu/tools-03-git-inspection-20261104` | [`b2c5e10ba7a3`](https://github.com/vjmichael/Lu-AI-GUI/commit/b2c5e10ba7a3fbad095d9c86194bdf87ae298433) | `lu/tools-02-json-validation-20261103` | Draft |
| [#12](https://github.com/vjmichael/Lu-AI-GUI/pull/12) | TOOLS-02: full original bounded MBRL JSON guard, recovery and desktop boundary checks | `lu/tools-02-json-validation-20261103` | [`383a30fae490`](https://github.com/vjmichael/Lu-AI-GUI/commit/383a30fae49092ebaaa150118e24885c7681a692) | `lu/write-01e-approved-edit-gate-20261103` | Draft |
| [#11](https://github.com/vjmichael/Lu-AI-GUI/pull/11) | DLC-API-00: generic first-party Lu Host Services RFC and typed bidirectional Protobuf contract | `lu/dlc-api-first-party-host-contract-20261103` | [`5eca23b61f41`](https://github.com/vjmichael/Lu-AI-GUI/commit/5eca23b61f41a34b813d854577078b4fbe4d41c6) | `main` | Draft |
| [#10](https://github.com/vjmichael/Lu-AI-GUI/pull/10) | DLC-IPC-01: common Windows/Linux Workspace inference contract (source-only) | `lu/dlc-interop-01-cross-platform-contract-20261103` | [`9cc090490487`](https://github.com/vjmichael/Lu-AI-GUI/commit/9cc0904904870c3860057f8130dda3872c0403d9) | `lu/write-01e-approved-edit-gate-20261103` | Draft |
| [#6](https://github.com/vjmichael/Lu-AI-GUI/pull/6) | Lu/NVP source-only inference interop contract and Windows containment source blocker | `lu/nvp-independent-inference-contract-20261103` | [`80c1a5aa33da`](https://github.com/vjmichael/Lu-AI-GUI/commit/80c1a5aa33dac5839985f7e297ea860d6d3fa922) | `main` | Draft |
| [#5](https://github.com/vjmichael/Lu-AI-GUI/pull/5) | WRITE-01E: journaled existing-file edit, recovery and explicit rollback | `lu/write-01e-approved-edit-gate-20261103` | [`2a9a135500b6`](https://github.com/vjmichael/Lu-AI-GUI/commit/2a9a135500b6e142564b7dec753f52f5184fd9bb) | `lu/win-load-01-host-trace-20261103` | Draft |
| [#4](https://github.com/vjmichael/Lu-AI-GUI/pull/4) | WIN-LOAD-01: host-side bounded model-load lifecycle diagnostics | `lu/win-load-01-host-trace-20261103` | [`e794d9bb8dc2`](https://github.com/vjmichael/Lu-AI-GUI/commit/e794d9bb8dc2dc7180e6003753343c3697d8f3ab) | `main` | Draft |
| [#3](https://github.com/vjmichael/Lu-AI-GUI/pull/3) | REPO-01: fail-closed source identity verifier and unit CI | `lu/repo01-source-identity-gate-20261103` | [`45f4db62d1f4`](https://github.com/vjmichael/Lu-AI-GUI/commit/45f4db62d1f47c1d070fddd6565612d61c6d2e56) | `main` | Draft |
<!-- LU_OPEN_PRS_END -->

## 5. Integration constraints and next handoff

1. **One integration head at a time.** Choose a source-locked integration branch anchored to an exact `main` commit; do not blindly merge all draft branches. Track intersecting files (`src-tauri/lu-core/src/lib.rs`, `core.proto`, desktop `lib.rs`, React `main.jsx`, Cargo manifests and lockfiles).
2. **Batch closely related tool functionality, not redundant implementations.** Verify a coherent tool family with full native checks and desktop dispatch before promoting it. Preserve source-only branches for provenance; safely close superseded PRs **only after** their unique code/evidence is accounted for.
3. **Current technical blockers:** existing Windows worker Job API discrepancy, mixed CI results, absent pinned browser runtime, Reader PDF/OCR limitations and unsynchronized desktop integrations. These are not grounds to declare all tools unbuilt.
4. **Next development gate:** tool branch/source/proto reconciliation → exact-head CI → real GGUF authorized tool loop → Windows installer acceptance; **then** memory integration per the owner decision. The accepted functional alpha remains unchanged.
5. **No faux release:** Update [CHANGELOG.md](../CHANGELOG.md) only when an owner-accepted milestone/release is established. A source commit or draft PR goes in this ledger, not into released-version notes.

## 6. Change-control workflow (mandatory for future Lu work)

- **Before writing:** read this `main` ledger, [CHANGELOG.md](../CHANGELOG.md), the affected PRs and current **exact head SHAs**; check the target branch and PR base. A conversation summary alone is not the branch status.
- **For each feature PR:** put a compact implementation receipt in the PR description: `LU-ID`, changed paths, original/base/head SHA, source/runtime scope, tests **passed/failed/skipped**, integration and installed-state truth, dependencies and next gate. Use [PR template](../.github/pull_request_template.md). Prefer independent PRs against `main`; use stacked branches only with an explicit dependency and plan to rebase/retarget.
- **When code changes:** update the relevant PR receipt and explicitly state what remains untested. Do **not** record `DONE` merely because a new source file exists or isolated tests pass.
- **At integration/merge:** update this feature map and acceptance/CI evidence on `main` in the same integration pass; add to release changelog **only** after acceptance. Avoid editing the central narrative independently in 20 competing feature branches: keep pending progress in the PR receipt and auto-generated inventory.
- **To refresh PR table:** `node scripts/lu-ledger.mjs --update`; `--check` detects drift. An optional GitHub Actions scheduled refresh updates **only** the marked table, never the manual evidence/status sections. GitHub API metadata cannot certify correctness of code.
- **End of each development session:** report current PR/commit, actual tests, failures, what's now in main, and next concrete gate. This is the handoff to the next conversation.

## 7. Brief development change log (not a release log)

| Date | Decision or confirmed observation | Evidence / follow-up |
| --- | --- | --- |
| 2026-11-03 | Owner accepted functional Windows `0.1.0-alpha.2` baseline | [Alpha decision](LU_ALPHA_01_ACCEPTED_BASELINE_20261103.md), with specified defects |
| 2026-11-04 | TOOLS-03–11 and Reader implementations accumulated in separate branches, often stacked | [Open draft PR inventory](#4-active-prs-and-branch-topology-machine-refreshable); awaiting coherent integration |
| 2026-11-05 | Owner chose **finish tools first, integrate memory second**; future DLC-specific media/IDE work deferred | [v1.0 requirements](LU_V1_0_DESIGN_RECONCILIATION_20261103.md), [AIW audit PR #29](https://github.com/vjmichael/Lu-AI-GUI/pull/29) |
| 2026-11-05 | Reconciled five allegedly missing tool areas: code exists, but acceptance differs by scope | Feature map and selective CI above |
| 2026-11-05 | Added central main-branch development ledger and machine-refreshable PR inventory to eliminate branch/conversation status drift | This document; repository maintenance tooling |
| 2026-11-05 | Ledger drift detector and fixture tests passed in GitHub Actions at `b4ea23e0c9d51834c24a3a3b50341c327600ce73` | [Verified CI job](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42071776300/job/127401333053): generator unit tests and live open-PR snapshot check passed. Scheduled auto-update and future write permissions have **not yet been proven**. |
| 2026-11-05 | Began tool closeout 1–5: reconciled existing sources and added narrowly scoped PRs #30–#33; recorded exact Win/Linux gates and unresolved integration | See the new closeout table above. No v1.0 installed acceptance or version bump. |
