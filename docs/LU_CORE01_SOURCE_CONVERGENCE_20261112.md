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
