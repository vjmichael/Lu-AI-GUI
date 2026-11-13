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

### CORE-01E verified TOOLS-09 fixture-limit failure from original native log

P0-GATE full original Core native regression at earlier compiler-passing head `7f1ed05910c0114c2c3ee6965775901a7779deff` / run [#43334831228](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43334831228) failed only `native_agent_tool_turn::text_search_caps_directory_entries_even_when_all_are_directories` in the then-executed native test binary: `data["result"]["truncated"]` was `false` rather than `true`. This is an actual fixture mismatch, not evidence of unlimited enumeration. The fixture creates **600** directory entries and asserts truncation, while `src-tauri/lu-core/src/native_file_extra.rs` currently binds the explicit native entry budget to **1,024**, and an ordinary 600-directory non-recursive scan fits within that budget. The repaired original fixture creates **1,025** entries, preserving the existing expected fail-closed `truncated=true` and bounded scanned-file assertions. No runtime search caps, authorization, symlink/reparse, mount, cancellation, restart or file-open protections are modified. Both native Windows/Linux exact-head regressions must still pass to close this issue; source fix alone is insufficient.

At `5f9595f7568774b94f64bec4c84a369380d87a66`, original REPO-01 source, Ubuntu full Core/Tauri compiler, both Ubuntu and Windows P0-WIN-CHAT native and hosted Qwen/SmolVLM GGUF job had passed at the time this repair was prepared; Windows full compiler and dependent full Core native/installed acceptance were still in progress/unverified. Those outcomes cannot be transferred automatically to a new source head.

### CORE-01E original full-native failure on exact head `88e34abd04e91c7ac14c20656673d3c7a5d59a3f`

Run [#43338530675](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43338530675) **completed failure** despite source, Windows/Linux Core+Tauri compiler, P0-WIN-CHAT native on both platforms, and genuine hosted Qwen/SmolVLM jobs all passing. The original full Core native regression jobs (Ubuntu job 131609251367, Windows job 131609251370) both reached `tests/task_checkpoints.rs:40` and failed `durable_progress_survives_restart_without_forced_timeout` with `InvalidTransition`. Earlier TOOLS-09 1,024-entry fixture failure is no longer present. The failing test attempted to `resume("run-1")` after exclusive startup had reconciled its RUNNING journal to INTERRUPTED. Current P0-SEC task-store policy explicitly permits resuming PAUSED only; INTERRUPTED loses old process/authorization and must never silently restart. **Focused source/test repair:** assert old interrupted ID cannot resume, heartbeat, checkpoint, or complete; assert original 25% checkpoint and source folder remain immutable; demonstrate separate new task identity completes only after separately initiated start. Do not change TaskStore transitions, permission gates, custody, or replay protections. This is a stale security-contract fixture, not a production defect.

The original run's hosted NSIS/UIAutomation and Debian/AT-SPI/install/uninstall jobs were **SKIPPED** because their full-native prerequisite failed. The new grouped exact-head native run must pass before installed acceptance; Linux positive delegated cgroup-v2 crash/guardian/restart and the physical Windows Radeon/interactive owner tests remain independent unaccepted gates. No merge/release.

### CORE-01E Gate 1 / SEC-06 — 2026-11-13 native reconciliation

On original exact-head `159fb75fd4c3d24d8b4aaf814de61abe4ad7deb9`, [run #43455392307](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43455392307) passed the Windows/Linux P0-GATE full original native regression suites and compiler diagnostics, P0-WIN-CHAT original native model/UI tests, and independent SEC-06 source and Windows jobs. SEC-06 Linux failed, so **CORE-01E native/security closeout remains OPEN** and this does **not** authorize installation.

The original Linux SEC-06 archived case logs identify the SEC-06B harness failure: all six `native_git_inspect` tests expected `LU_TOOLS03_TEST_GIT_PATH` but its isolated Linux job omitted the exact absolute Git path set by the passing P0-GATE Ubuntu job. The repair provisions an actual canonical host Git executable in the isolated SEC-06 Linux workflow, with a source regression to prevent reintroduction; there is no reduction in authorization boundaries.

SEC-06C independently remains **BLOCKED** because hosted Linux did not provide nonroot writable delegated cgroup-v2. The first-party crash fixture returned NOT_AVAILABLE, original Git restart refused PGID substitution, and guardian/worker/ancestor negative fixtures exited 77 before execution. **Do not call an unexecuted ancestor attack a pass or escape**, and do not accept an actual surviving migrated descendant. Next: validate repaired original SEC-06B full Core results at a new exact SHA; obtain genuine delegated Linux kernel host-crash, guardian, setsid, ancestor, original Git restart, cancellation and receipts; reconcile SEC-06F at one source SHA; only then seek separate one-use installed authorization. Keep branch draft, no IDE/KoboldCpp scope, no integration into main.

### CORE-01E SEC-06F original digest mismatch — 2026-11-13

Same-commit independent Windows and Linux SEC-06 native receipts on `159fb75fd4c3d24d8b4aaf814de61abe4ad7deb9` have individually correct saved log SHA-256 hashes (Windows 14/14, Linux 22/22) but **different hash of the checked-out `source-tree.lock.json`**, despite both passing original REPO-01 source verification. This is an additional demonstrated exact-head reconciliation blocker, separate from the SEC-06B missing native Linux Git fixture path and SEC-06C unavailable delegated cgroup. The fix hashes the canonical Git blob with strict working-tree CRLF-equivalence only; an actual Git fixture tests cross-platform digest stability and refusal of modified JSON. Original SEC-06 source/Linux/Windows OS receipts must be regenerated on the repaired source before SEC-06F is reviewed; no promotion from old receipts and no installed authorization.

### SEC-06C Linux delegated-cgroup runner (2026-11-13)

Added a genuine systemd `Delegate=yes` per-run scope for the independent SEC-06 Linux case suite, with a nonroot service identity, a kernel-proven child cgroup, `user.delegate=1` and populated-zero; its preflight witness is SHA-256-linked to the original Linux SEC-06 receipt. The original worker/guardian/setsid/migration attacks still run unchanged. Lack of delegation or a successful ancestor escape continues to BLOCK acceptance; no installed/merge/release approval is issued by this code.

### CORE-01E SEC-06C original native ancestor escape (2026-11-13)

The genuine nonroot systemd-delegated Linux case reached all 23 native tests at source `53beb9ab15a1ba1afe8f14241e210f902b406b83`: 22 passed, one **security failure**. The adversarial descendant escaped into a same-UID writable ancestor and produced `FAIL_ESCAPED_DESCENDANT_MINTED_FALSE_EMPTY_RECEIPT`. The original verifier correctly rejected it. The immediate source hardening refuses writable/owned ancestor `cgroup.procs` before admission and prior to issuing a receipt; this is fail-closed protection, **not** a claim of positive containment. Next SEC-06C engineering task is an OS-owned outer cgroup broker (or equivalent real kernel-enforced user/namespace separation), followed by rerunning the *same* original host SIGKILL/guardian/setsid/ancestor kernel attack and SEC-06F cross-OS receipt reconciliation at one exact source. The installed owner gate remains unissued.

### CORE-01E exact-head Windows regression reconciliation — 2026-11-13

Original same-source [run #43467964216](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43467964216) at `53beb9ab15a1ba1afe8f14241e210f902b406b83` found two **independent Windows failures** besides the Linux SEC-06C ancestor escape. Windows SEC-06 source-contract case log (artifact #14060603417) reported 3 of 61 Node tests failing: two fixed-workflow-section assertions searched only LF and failed against genuine Windows CRLF, and a forged Linux receipt test hashed raw Windows checkout lockfile rather than the canonical committed Git blob. The same Windows security matrix's 13 other native/test cases passed. Full Windows P0-GATE original native regression (job #132045084991) passed many suites but two `e1_reviewed_model_tools` tests collided on `ansf_sessions.id` because both temp SQLite fixtures reused PID plus a coarse Windows clock tick. The exact source repair normalizes CRLF **only for parsing workflow text**, retains strict expectations, uses the canonical source lock hash in verifier fixtures, and atomically differentiates concurrent ANSF/SQLite fixture roots with a 64-thread uniqueness regression. Production ANSF, authority and security policies remain unchanged. None of these prior Windows results promotes SEC-06C or SEC-06F.

### SEC-06C OS-owned operation boundary prototype

A separately executed native `systemd-run --system` service with `Delegate=no` and `KillMode=control-group` tests the independently OS-owned unit topology against external same-UID ancestor migration, actual setsid child, original PIDFD parent SIGKILL, and original child death. This is the candidate fix for the demonstrated same-user delegated-parent escape. Production Lu worker/guardian integration has not been replaced and the original SEC-06 adversarial matrix is still mandatory. Never promote a prototype PASS to SEC-06C or SEC-06F acceptance. No installation authorization.

### SEC-06C root-owned Git service integration

Actual reviewed Git worker supervision is wired to an optional installed fixed, privileged systemd broker. The first executable in the system unit is a root-owned first-party, nonroot Git-read worker with a strictly finite command surface; runtime manager enforces non-delegated root-owned cgroup, mount protection, strict one-use GO and original digest. Lu persists immutable original call/turn/nonce identity and verifies root-owned kernel populated-zero receipts before completion and after journal restart. CI exercises the original reviewed Git task end-to-end in its own ephemeral privileged OS fixture. This is an implementation and test of the new **Git scope only**, not a positive declaration for the whole Lu worker registry or SEC-06C native containment. The full original ancestor red test and cross-OS SEC-06F are still required.

### SEC-06C first broker native failure → cancellation and crash-recovery expansion (2026-11-13)

Original [run #43483852348](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43483852348) reached independent source/native jobs. New root-owned Git integration was blocked before kernel execution by Rust E0509 in the test's move from a zeroizing `CapturedVersionOutput`; the actual read-only OS worker source compiled in the separate helper crate. SEC-06 source case failed one outdated literal error string assertion (64/65 contracts pass), and the root-owned candidate prototype used the wrong repository-relative path; these are corrected without weakening any real process-containment acceptance. Original same-UID delegated native guardian cases explicitly fail-closed on writable ancestors and remain BLOCKED.

New original Linux gates include finite reviewed-Git cancellation, verified old-authority non-replay after original host SIGKILL, and both OS-owned kernel-unit cancel/host-crash recovery with exact pidfds and descendant cleanup. They are additional cases; no original hostile-migration, native Windows Job, delegate availability or security receipt requirement has been removed. The fixed one-use broker now refuses a stale host-lifetime pipe before first worker GO and the nonroot Git worker refuses a stale pipe before exec. **All new native kernel acceptance remains OPEN** until the replacement original evidence is independently inspected. FFprobe and remaining reviewed tools cannot be classified OS-owned/accepted until their distinct finite host/OS boundary is actually integrated.

### SEC-06C independent reviewed-media/metadata OS-owned boundary (2026-11-13)

The original Git-only broker test at `982b742496821d89507b4ddaf9f466567d8d3057` compiled but returned `ProcessUncertain` before producing a root-owned Linux completion witness; its actual cause needs new *debug-only* native stderr evidence, and SEC-06C stays OPEN. Original Linux systemd-owned outer-boundary SIGKILL/cancel/host-crash positive prototypes passed, while the unchanged delegated leaf guardian/ancestor probes correctly BLOCK on writable-parent admission.

Implemented a **separate finite** OS-owned launcher and worker for FFprobe WAV/MP4, PDFInfo PDF and ExifTool PNG/JPEG, with original resource input hash, fixed typed argument schema, exact executable bytes, root-owned systemd unit, no shell or network, nonroot FD-pinned resource handling, immutable original admission journal, root-owned `cgroup.events` populated-zero receipt and independent post-restart verification. A native CI job tests all four genuine third-party operations. The existing finite Git broker remains unchanged in its command surface. This is source implementation, not accepted kernel execution.

Only these four probe kinds are enrolled; any additional worker domain requires its own reviewed argument/resource policy and separate native process-custody proof. Do not fold file-scanner, archive, browser, version or IDE processes into a privileged generic command router. Existing SEC-06C adversarial and SEC-06F same-head Windows/Linux/source gates remain mandatory and unclosed. Installer/merge/release still unauthorized.

### SEC-06F complete same-head independent native receipt audit job

The exact-head REPO-01 workflow now runs a dedicated **audit-only** SEC-06F job after unchanged source/Linux/Windows SEC-06 original native cases and root-owned Git + distinct finite probe acceptance. It rehashes original logs, checks delegated Linux kernel witness, source-lock SHA and all mandatory native case IDs on the *identical* source. It fails for any missing/failing original matrix or broker, and even an evidence-complete native audit remains UNREVIEWED pending owner and independently authorized installed acceptance. No automatic merge, installed SHA, or release is possible. Native job results remain pending until observed; do not treat CI admission as receipt success.

### SEC-06C compatibility and SEC-06F exact-head audit (2026-11-13)

Native [run #43492371261](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43492371261) demonstrated a real Ubuntu 24.04 systemd parser rejection of `ProtectControlGroups=strict` on the *separate finite probe* unit. Both reviewed OS broker launchers now use supported `ProtectControlGroups=yes` with the same required observed read-only kernel mount and root-owned non-writable ancestor boundary. The missing original `linux-root-owned-unit-cancel` and `linux-root-owned-unit-host-crash` synthetic verifier rows are restored without weakening required acceptance. The Git broker's first positive original service still requires a newly instrumented native outcome. Added SEC-06F audit job to require identical Windows/Linux/source receipts and genuine Git + probe OS tests, with an explicit non-release native-only result. Installer/merge/release remain unauthorized.

### SEC-06C root-owned kernel witness timing repair

Original exact-head #43493688913 Git and media root-owned **both failed** at post-stop `cgroup.events` verification after entering an actual systemd unit. The corrected finite systemd brokers now pin their own root-owned operation and observe genuine recursive kernel `populated 0` before stopping/removing the unit, with `--remain-after-exit`, bounded cleanup after exact `cgroup.kill` on cancellation/host loss and explicit denial on missing witness. A first-party worker exit93 never authorizes completion. The unchanged same-user ancestor-migration and original guardian tests still block; only a new exact-head native positive run can close SEC-06C and permit independent SEC-06F. Neither installed nor release authorization granted.

### SEC-06C native source correction to persistent root-owned leaf containment

Run #43498557875 demonstrated that the earlier root-owned transient service still lost its own kernel cgroup node before the parent could verify `populated=0`; Git and media probes both failed safe with `ProcessUncertain`. The replacement uses a distinct root-owned systemd delegated manager outside the separate root-owned worker cgroup. A mandatory same-UID real setsid descendant migration into the root-owned ancestor must be **denied by the kernel before one-use approval**, then actual nonroot Git/media/metadata worker executes in a private read-only-cgroup mount. The trusted manager observes root-owned leaf `cgroup.events populated 0` before deleting the leaf and signing the durable root-only receipt. Original legacy guardian and other reviewed worker domains remain open, without generic privilege. No runtime outcome is claimed until exact-head genuine Linux tests. SEC-06F original Windows/Linux/source and owner approval remain gated.
