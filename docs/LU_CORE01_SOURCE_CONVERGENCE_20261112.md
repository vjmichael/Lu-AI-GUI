# CORE-01 — Lu Core source convergence (IDE and KoboldCpp excluded)

Status: **started, source baseline established; integration/acceptance NOT complete**. Owner direction: finish Lu Core backlog, keep llama.cpp as the sole production inference runtime; no IDE, no KoboldCpp, no merge to main or installed release.

## Exact convergence base and demonstrated ancestry (2026-11-12)

New branch: `lu/core01-source-convergence-no-ide-no-kobold-20261112`.
Frozen initial parent: `9b14dde7a01ad36208642be7536159ccb1f42e93` (P0-WIN-CHAT PR #224). GitHub compare shows it is an exact descendant of:
- R00 Lu Core convergence PR #200, `e61d4bf998bccf51074b01c864df208c233eb2f8` (268 commits, 132 changed files to #224; no KoboldCpp-named paths in that differential);
- installed chat UX PR #205, `378ee5273201a6fa451ff511c0051d76bd1ea70c`;
- PLAN-01 PR #207 `e18d92c62baeb5d5d49d6c1744fc25bbd68bfd50`;
- PLAN-02 PR #208 `7e9fa26d20147e3a7bbc6ffa3c2fec8e7808ab83`;
- PLAN-03 PR #209 `8070b69ca98163c3d00843252d307c38b0157e0e`;
- PLAN-04 PR #210 `ea8ea8ecde2bfbb9ccb5429ac0543d9a5f2a91dd`;
- PLAN-05 PR #213 `223d35e65ad58f7aeaabac5c9d1f1d97d87c714f`;
- PLAN-06 PR #216 `67a7c8938427aff4b03854c53a5137bebe858ef5`;
- PLAN-07 PR #219 `fb336d3868a7dccb751ca93027956219e8d8147e`;
- P0-GATE PR #223 `e422d18cef10fdb1092578631b5fec692b3d7a4f`.
MEMORY-LOOP-03E PR #198 (`74dbdfe1f8457631f1992efed2c79b37629715aa`) is also an ancestor of R00; it does not require replay.

**No claim is made that these draft features all pass end-to-end integrated native or installed acceptance.** Ancestry proves inclusion in this candidate's history, not behavioral completion.

## Explicit exclusions — preserve at every CORE-01 commit

1. Do not merge, cherry-pick, copy or edit IDE-owned `ide/`, IDE workers, debugger/LSP source, IDE acceptance tests, or IDE workflows as part of this convergence. Existing historical/shared files in the base are not silently deleted.
2. Do not merge P7L branches, `p7_koboldcpp_backend.rs`, test Kobold server, any `p7l_*` files or the KoboldCpp workflow. Do not register an alternative inference provider; production llama.cpp remains authoritative.
3. Do not merge to `main`, start an installed release or change signing policy. Avoid per-commit CI fanout.
4. Preserve host approval, OS custody, ANSF integrity, fail-closed UNCERTAIN and local-model privacy.

## Remaining divergent Core candidates

- PERF-01 / P0 security and TOOLS-09 branch lineage: the P7L-1 tip is **346 commits ahead of #224**, including legitimate non-Kobold Core fixes mixed with experimental KoboldCpp additions. NEVER merge this tip as a whole. Reconcile required changes selectively with original source comparison, dependency review and scoped tests. Freeze experimental P7/P7L and optional P6 compute-node work.
- Observe & Learn PR #61 head `9b6a886caddaa221e13933446fe0100afe77cf90` is **diverged** from #224 (15 ahead, 1690 behind, merge base `d191c56ecf96c2a2e32de47665852e46a3553b61`). Compare individual remaining source changes; never merge the old full branch wholesale. Maintain OFF-by-default capture and explicit operator consent.
- Core source identity and model-to-tool integration on #224 are present in branch history, but current installed failures in issue #204 remain unresolved without actual run evidence.

## CORE-01 next implementation gates

A. Run a source/ownership allowlist comparison against the chosen parent, identify only Core-specific missing changes, record exact file IDs and dependencies.
B. Integrate indispensable **non-Kobold** Core changes in coherent file groups: P0-SEC, TOOL-09 recursive search, PERF-01 P0–P4 (llama.cpp path only), and Observe & Learn required v1.0 components when genuinely missing. Do not add experimental backend source or IDE artifacts.
C. Rebuild production normal chat: real tool discovery/authorization, browsed evidence, MMProj and time/path tools; compare source with issue #204's actual failures.
D. Verify ANSF task/provenance and PLAN integration, supervisor recovery and GUI state on the same source tree. Preserve source manifest/lock; source-only checks are not native acceptance.
E. After coherent implementation, run **one** grouped Windows/Linux native and installed acceptance and repair original failures. Physical AMD Vulkan / real Qwen vision remain separate acceptance.

## Gate definitions
- **INHERITED_SOURCE:** present in ancestry, NOT automatically accepted.
- **NEEDS_INTEGRATION:** missing or divergent, verified by file/trait and route-level diff.
- **NEEDS_NATIVE:** source integrated, native OS compiler/fixture result absent.
- **NEEDS_INSTALLED:** actual installed workflow/owner observation absent.
- **ACCEPTED:** exact source and appropriate independently observed original runtime evidence.

This branch exists for convergence only; it does not close CORE-01 yet.

## CORE-01A — strict source identity and isolation gate (2026-11-12)

The original path-only gate is replaced by an exact HEAD Git-tree/blob inventory check plus a baseline-to-HEAD delta review. Scope rejects changed IDE paths and all P7L/KoboldCpp-named paths, including deletions and renames represented by deleted/added tree entries. Executable Rust/JS/C/C++/Protobuf and dependency manifests added or modified since the reviewed baseline are scanned for experimental provider references and IDE imports. Existing pre-baseline source is not silently deleted. The check requires a clean checkout, reviewed ancestor, ordered exact tree inventory, and no missing or extra lock entries. It is intentionally stricter than a filename-only test, but is not a complete dependency supply-chain review or a behavior proof.

Negative Node tests cover forbidden paths, excluded provider registration, allowed llama.cpp source, documentation nonfalse-positive, and corrupted/extra lock entries. A stale manifest is rejected until refreshed from the exact Git tree. CORE-01A is **source-only**; Rust, Windows/Linux kernel custody, installed apps, actual GGUF and physical Radeon acceptance remain OPEN. No IDE/P7L feature branches, merges, permissions or runtime providers are introduced.

## CORE-01B — P0-SEC and TOOLS-09 selective source integration (2026-11-12)

**Integrated candidate commit:** `96031bcf07601da27fb582d898b5df3700eaa7d8`.

- Original security/search source lineage: `lu/tools09-bounded-workspace-search-20261112` at `d27c31ad5592999fdbb0f7056009988cedcdc836`, which descends from P0-SEC06 `50a6f68ba322b3fed5959af2af49f75388c2905c`.
- Compared each source blob against immutable P0-WIN-CHAT baseline `9b14dde7a01ad36208642be7536159ccb1f42e93` and current convergence head `cd9fa51b724de06300627335134d9e189482d602`. Exactly **31 non-lock changed paths**, **0 current-source overlap**, **0 prohibited IDE or P7L/KoboldCpp paths**. Imported exact reviewed blobs, without merging the donor commit history, replacing unrelated Core/PLAN files or modifying production llama.cpp provider registration.
- Included P0-SEC guardian/negative-process fixtures, per-turn cancellation and authorization revocation, host recovery and ANSF reconciliation, task security tests, Lua desktop visibility, Linux/Windows custody evidence tooling, and the TOOLS-09 bounded recursive file search and typed Protobuf fields with adversarial fixtures.
- Independently inspected resulting Git tree: **964 non-lock tracked blobs, 964 manifest entries, zero missing or mismatching blobs**. Source static searches in imported runtime/fixture code found no excluded backend or IDE imports.
- These results prove **SOURCE_INTEGRATED** only. Actual Node contracts, native Windows/Linux Rust compile/regression, real guardian-death/cgroup/Job fixtures, Tauri/React normal-chat search and installed Windows GGUF acceptance remain **NEEDS_NATIVE / NEEDS_INSTALLED**. No CI pass, hardware result, main merge, release or autonomy expansion is claimed.
- Next CORE-01C: selectively reconcile the llama.cpp-only PERF-01 P0–P4 changes. Preserve CPU deterministic admission, limited foreground priority, real streaming/restart semantics and fail-closed GPU accounting; exclude speculative P5/P6/P7 and all KoboldCpp/P7L.

## CORE-01C — PERF-01 P0–P4 llama.cpp-only source convergence (2026-11-12)

**Source commit:** `8b2026847c0b6e02df36679236b6c5a4cb78f117`. Donor source `lu/perf01-p4-interactive-priority-20261112` at `ae3739d7485ee4205f75d1f85456c1ba35949d45`, compared against its exact ancestor TOOLS-09 `d27c31ad5592999fdbb0f7056009988cedcdc836` and protected CORE-01B `5a76c7e1a77f6cd9dcff82dde16940f333dea873`. Exactly **36 non-lock source paths**, **zero overlapping Core edits**, and **zero prohibited IDE/P7L/KoboldCpp paths**; no full donor-branch merge or runtime-provider substitution.

**Integrated source candidates:** original CPU-first deterministic host routing and telemetry (P0/P0D); warm-model residency (P1); foreground inference queueing, latency/first-token observations (P2); prefix/KV-cache identity and invalidation boundaries (P2 cache); durable bounded partial-output streaming and reconnect/restart behavior (P3); conservative interactive resource admission, fairness and human-first queue lane with fail-closed unmeasured GPU/VRAM capacity (P4). Source includes `persistent_llama_backend.rs`, `warm_model_residency.rs`, `prefix_kv_cache.rs`, `stream_delivery.rs`, `generation_queue.rs`, and host/UI/negative fixtures. Production llama.cpp remains the only selected inference backend.

**Independent Git-tree source audit at integration head:** 983 tracked non-lock blobs matched 983 lock entries, zero missing/mismatched entries; no forbidden source-delta paths. Executable donor files checked for excluded backend names and IDE module imports with no matches. This is static **SOURCE_INTEGRATED** evidence, not executed Node tests, Windows/Linux Rust builds, actual TTFT, installed GGUF benchmark, or guardian hardware validation.

**Still OPEN:** exact-head Core/Tauri compiler, Node contracts, actual native Windows/Linux scheduler/process/stream recovery fixtures; physical Windows/Radeon Vulkan TTFT p50/p95 and GPU contention measurements; browser/model/tool/vision normal-chat acceptance; supervised cancellation and restart receipts; native and installed application UI acceptance. P5 inference-tuning/speculative decoding, P6 multi-GPU/node scheduling and P7/P7L alternate backend evaluation are NOT included in CORE-01C. No merge to `main` or release.

**Next CORE-01D:** reconcile the one-source normal-chat path, per-turn tool discovery, ANSF/PLAN/task-state integration, authorized citations, GUI queues and recovery. Then one grouped exact-head native/installed acceptance with actual original failure logs.

## CORE-01D — Normal Send and native-clock authorization boundary (2026-11-12)

Source trace of the existing consolidated Lu desktop confirmed that ordinary `lu_generate` retains host-originated clock, read-only CPU/ANSF, separately authorized Browser, and active GGUF paths. PLAN-01–07 and ANSF/context/recovery source modules are already present in this baseline; inclusion is not an installed or end-to-end pass. Browser manifest remains `not-packaged`, so public URL browsing is not advertised as operational without its approved native dependency. MMProj, citations, task evidence and per-turn tools retain their independent host gates.

A concrete defect was identified and repaired: Rust `host_clock::explicit_current_time_request` and UI `isHostCurrentTimeQuestion` used `starts_with` / `startsWith` to recognize a no-model time question. That incorrectly consumed compound requests such as "What time is it and inspect my files?" as if they were a standalone clock request. Both now require a complete normalized question (optional final question marks only), and host-side Rust remains the sole authority. Added negative tests for chained, newline, attachment and tool-token requests plus UI/host source contract. No permission, model/provider, IDE or KoboldCpp source was added.

**Acceptance:** source-level fix and static Git inventory only. New Node fixtures and Rust tests must actually execute on the exact head. The complete normal-chat native Windows/Linux and actual installed GGUF/browser/vision/PLAN/restart matrix remains CORE-01E. Do not imply OS custody, GUI behavior, true model-to-tool generation, valid citations, or physical AMD Vulkan passes from this source change.

### CORE-01D source-status reconciliation

Native `lu_generate` routes the original saved human chat through CPU search/count/status, OS clock, separately approved browser/Git/filesystem/reader/media specialist paths, and the normal model path with ANSF/PLAN/procedural context. The existing CPU ANSF literal search correctly labels capped scans as partial in its answer, but its performance receipt still recorded a blanket `COMPLETED`. The receipt now records `PARTIAL` whenever `original.complete` is false. This prevents performance and task-status evidence from implying exhaustive original-source search when only bounded lines/pages were checked. Source-contract coverage pins that conditional. Actual native and installed behavioral tests are not yet evidenced.

## CORE-01E grouped exact-head native and installed acceptance — admitted candidate (2026-11-12)

CORE-01E reuses the existing REPO-01 workflow's **P0-GATE compiler → native → genuine NSIS/Debian installed jobs** and its separately scoped **P0-WIN-CHAT native and real small-GGUF/SmolVLM pixel jobs**. Those original build/test/independent-witness commands have not been weakened. Only the three outer exact-SHA admission conditions are extended to the new protected CORE-01 branch, with original markers and one additional CORE-01E marker mandatory; no per-commit fanout, new backend, IDE, or main merge. On opening the dedicated draft PR with exactly one matching marker of each kind, both original test families run against **the SAME PR head SHA**, with compiler/native prerequisites before installed jobs.

CORE-01A's frozen baseline ancestry check previously ran in a shallow `actions/checkout` despite requiring `merge-base --is-ancestor`. REPO-01 source checkout now fetches full history for this exact source check. Git full-tree/lock check remains independent, and the scoped CORE-01A/01D source contract tests run before native admission.

**Admission is not completion.** The original jobs must produce actual Windows/Linux Core and Tauri compiler/test receipts, OS Job/cgroup guardian and authorization races, React/ANSF/PLAN/TOOLS-09/PERF tests, installed Win32 UIAutomation and Debian AT-SPI/restart/uninstall evidence, hosted Qwen Git/FFprobe and SmolVLM MMProj pixel output. A hosted test-only small model is NOT the physical Windows Radeon Qwen3.8 vision/model memory/VRAM acceptance; those remain distinct. Browser `manifest.json` still says `not-packaged`; absent an independently reviewed Chromium/Node/Playwright payload and signed provenance, *Browser installed acceptance stays OPEN*, even if other hosted tests pass. No simulation or placeholder may satisfy actual installed/model gates.

On failure, inspect original job-step logs and fixtures, repair only demonstrated defects and readmit **one new exact head**, never certify a queued or skipped job. No main merge/release.

### CORE-01E first admission — original failing source policy fixture

At exact head `26d2210dec1d16886e6259a554bf87829abcfdb7`, REPO-01 workflow run [#43333587507](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43333587507), original verify-source job `131584029593`: source lock **984/984 PASS**, CORE-01A tests **5/5 PASS**, CORE-01D tests **2/2 PASS**. Grouped CI policy tests **8/9**, one failure: `HEAVY_WORKFLOW_AUTOMATIC_FANOUT` from the older `.github/workflows/lu-perf01-p0-p0d-exact-head.yml` still retaining a branch `push` trigger. Compiler, native, installed and model jobs were SKIPPED as dependents, not independently failed.

**Focused repair:** remove only the stale `push` trigger from that PERF-01 acceptance workflow; preserve `workflow_dispatch` and all original test commands/OS matrices. Regenerate source lock and admit one new exact-head PR-triggered consolidated run. This is an original-log repair, not a relaxation of the CI policy.

### CORE-01E second admission — exact native compiler error in PERF-01 P1

At `a8a5ac8bc2478179a7cafc9210b0957138ebc2e2`, run [#43334008545](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43334008545) passed REPO-01 source verification, then both original Ubuntu compiler and Ubuntu/Windows P0-WIN-CHAT native jobs reported Rust parser error: `src-tauri/lu-core/src/warm_model_residency.rs:58:1 unexpected closing delimiter`. Original checked-in source contained three **literal** `\\n` sequences inside a `//` comment, which commented out the measured-pressure `if` branch and left its close brace unmatched. The source repair converts only those three escapes to real line breaks. A CORE-01A source regression now guards pressure-branch visibility and rejects the escaped-line defect. No warm-residency thresholds, model authority, eviction policy, backend or supervised-worker behavior changed.

The previous status is COMPILE_FAILED, not native-accepted. Require a new exact-head Windows/Linux compiler, native, hosted installed and model run; real OS receipts and physical owner acceptance remain independent.

### CORE-01E original integrated native and real-model run #43334831228

At exact head `7f1ed05910c0114c2c3ee6965775901a7779deff`, original REPO-01 source gate passed 984/984; CORE-01A Node 6/6, CORE-01D Node 2/2, CI-policy Node 9/9. Ubuntu P0-GATE Core + Tauri compiler/test-binary job passed. The hosted pinned Qwen3-0.6B Git/FFprobe real model-selection and SmolVLM-256M Q8-MMProj red/blue pixel test job **passed**. This is not installed/physical Qwen3.8 acceptance. Windows P0-GATE desktop compiler was in progress at the time of this repair.

The original P0-WIN-CHAT Ubuntu and Windows jobs both failed in the identical JavaScript source-contract assertion at `lu-desktop/tests/p0-physical-ux-contract.test.mjs:93`. This test demanded the obsolete direct `model_prompt.push_str(&host_clock::fresh_local_clock_context())`; actual PERF-01 P2 source now binds fresh host clock to `clock`, appends the same observation, preserves a `cache_only_clock_suffix=Some(clock)`, and excludes that suffix from the reusable prefix. A focused test-only correction checks the real human-origin block and noncached suffix. Verified the exact regular expressions against current Rust source text before committing. No production model, security or cache semantics were changed. Windows/Linux tests must run again on the new exact source head, and old run statuses cannot count as new-head acceptance.

CORE-01E remains OPEN for independent Windows/Linux native regression, actual hosted NSIS/Debian installed tests, and physical owner Radeon/real Qwen3.8. Browser still not packaged.
