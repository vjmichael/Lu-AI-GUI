# ChatGPT Work — Lu Project-Wide Outstanding Engineering Backlog
**Date:** November 9, 2026  
**Main repository:** https://github.com/vjmichael/Lu-AI-GUI  
**Scope:** Lu Core/desktop, its execution/memory/tool infrastructure, and the **standalone Lu IDE** as a separate workstream. NVP, Single-Galaxy and Cognitum are **separate repositories/products** and are not implementation assignments here; only their future Lu-facing interfaces are noted.  
**Explicit exclusions from this work order:** hands-on installation on the owner's local physical PC; a physical local-PC test that loads a **7B+ GGUF**; final owner-operated Windows interactive sign-off. These remain deferred **release** gates, not completed or waived. The worker may still run Windows/Linux CI runners, controlled test hosts, small real GGUFs in CI, and ephemeral installer/packaging smoke tests.

## Operating instructions — do these before implementation
1. Read `docs/LU_DEVELOPMENT_LEDGER.md`, `docs/LU_KIAF_ACTIVE_EXECUTION_LEDGER_20261108.md`, `docs/LU_V1_0_TASKS.md`, `docs/LU_NATIVE_TOOLS_IMPLEMENTATION_LEDGER.md`, and `docs/LU_CENTRAL_DESIGN.md`. For IDE, read `ide/PROJECT_STATE.md`, `ide/ENGINEERING_HANDOFF.md`, `ide/AGENTS.md` and `ide/docs/AI_FIRST_IDE_DESIGN.md` on its **actual IDE branch**, not an unrelated Core branch.
2. Refresh the complete open/closed/draft PR and branch inventory; inspect bases/heads and exact CI runs. `docs/LU_DEVELOPMENT_LEDGER.md` contains an older snapshot; **do not treat it as current** without refreshing. Preserve accepted `0.1.0-alpha.2`. Identify what is on `main`, only on stacked PRs, source-only, compiled, exercised with a real model, or installed accepted.
3. Use **Foundational / Utility / Specialist** tool classifications, and **K/I/A/F** = Knowledge (model), Interfaces (installed capabilities), Authority (current human/host-granted effects), Feedback (independent source/outcome checks and recovery). The user supplies goals; retrieved memory never grants permission. Freeze speculative tool breadth. Reuse existing tools and mature specialist software, do not build unnecessary mini-apps. No mandatory MCP, Docker or universal sandbox.
4. For **every** item below, report: current file/PR, exact head SHA, source state, actual Windows/Linux build/test results, genuine behavior evidence, failed/skipped checks, remediation commit, and remaining acceptance. A passed test on an ancestor is **not** a current-head pass. A queued/pending/cancelled CI run is **not** a pass. Use actual logs/artifacts; repair root causes, not assertions merely to obtain green.
5. Keep distinct: **I = implementation still needed**, **G = gaps/reconciliation/integration**, **A = acceptance gates that can run without the owner's physical 7B+ PC**. Many items have source candidates; do not recreate them. Every work area has a closed-scope deliverable. P0 is core correctness/security and P1 is useful hardening.
6. Maintain exact Git source manifests (`source-tree.lock.json`) when applicable, sorted by bytewise/JS lexicographic relative path. Follow the actual branch's manifest format. Merge/rebase only after inspecting ancestry and obtaining explicit authorization where needed. Do not automatically merge all drafts to `main`.

### Reference branches at handoff (refresh immediately)
- Lu MEMORY-LOOP-03B: [draft PR #188](https://github.com/vjmichael/Lu-AI-GUI/pull/188), `lu/memory-loop03b-procedural-belief-ccm-20261109`, known SHA `cbca41f79a1ede41d66a251196b834d3ed18eb62`. Its 03B, desktop and source CI were **queued/pending**, not accepted at last observation. Parent 03A [#187](https://github.com/vjmichael/Lu-AI-GUI/pull/187), parent 02C [#185](https://github.com/vjmichael/Lu-AI-GUI/pull/185), 02B [#182](https://github.com/vjmichael/Lu-AI-GUI/pull/182).
- Lu IDE protected debugger: [draft PR #189](https://github.com/vjmichael/Lu-AI-GUI/pull/189), known SHA `83b02c4948b0626a1830209a30f9639568fa41da`, stacked on #186/#184/#181. Latest related CI **queued/pending**, not accepted at observation.
- General tool integration: [draft #61](https://github.com/vjmichael/Lu-AI-GUI/pull/61) stacked over #59 → #57 → #44, with source candidates across Browser/Research/Observe, normal-chat tools and archives. Older evidence noted Cargo.lock divergence and stale source-contract tests; **refresh current state before fixing**.
- Other important workstreams include E1-P03–P15, AGENT-01A–L, UX-CHAT01/02, M2–M5, IDE B1–B11, IDE DEV-02–04, and Windows/Linux custodians.

## WORK AREA 00 — Repo topology, source convergence, dependency CI and evidence [P0]
**I — Implement/finish:**
- [ ] **R00-I1** Generate a machine-refreshable PR/branch/dependency graph for **all** open PRs (including older ones not returned on a first 100-result page); map what is in `main` and what is draft-only. Group by Lu Core, tools waves, memory/agent/E1 and IDE.
- [ ] **R00-I2** Reconcile source snapshots and ancestry; establish precise source identities, release/alpha hashes, and `Cargo.lock`/npm lock consistency; repair genuine merge conflicts and source-tree drift.
- [ ] **R00-I3** Prune/disable obsolete CI fanout and superseded runs **without suppressing relevant gates**; preserve honest per-head evidence and artifacts.
**G — Known gaps:** draft feature branches should not be mistaken for integrated `main`; older ledgers and PR descriptions may be stale; some ancestor Cargo/npm/test-code defects had been repaired only on descendants; REPO-01 original unpublished local snapshot equivalence is not automatically established by a Git tree manifest.
**A — Non-physical gates:** exact-head Win2025 + Ubuntu24.04 compile, `cargo test/check --locked`, Node tests, source-tree lock verification; branch-by-branch diff/convergence audit; deterministic dependency/reproducibility check; all actual failing or skipped jobs listed.
**Exit:** current machine-readable PR stack and gate matrix, zero unexplained source identity/lockfile drift, and real exact-head CI for the intended integration branch.

## WORK AREA 01 — Local model engine and backend lifecycle [P0/P1]
**I:**
- [ ] **R01-I1** Complete host-measured GGUF load/unload progress, heartbeat and hang diagnostics; never display a fake percentage. Keep model status and picker in sync.
- [ ] **R01-I2** Finish reliable local model import/catalog/profile selection, optional MMProj association, per-model folders and auto-load rules, crash/timeout/cancel state transitions.
- [ ] **R01-I3** Finish clean backend selection/availability diagnostics for supported llama.cpp CPU/Vulkan/ROCm/CUDA paths; treat vLLM/alternate backend as gated optional integration rather than pretending it works.
- [ ] **R01-I4** Verify memory/VRAM pressure handling, stop/unload cleanup and no incompatible simultaneous model sessions.
**G:** Windows Not Responding during model load, spinner/progress visibility, engine-backend mismatch, model-profile inference differences, resource release. Do not broaden to unsupported model architectures based only on GGUF metadata.
**A:** unit/host tests for GGUF metadata, MMProj admissibility, import cancellation, model-session changes, backend faults, watchdog timing, fake GPU/device claims, hostile paths; **small pinned real GGUF** inference and stop/restart on CI or controlled host; stress tests for resource cleanup. No 7B+ physical-PC test required.

## WORK AREA 02 — ANSF migration, durable conversations and original-source continuity [P0]
**I:**
- [ ] **R02-I1** Complete M2 canonical ANSF/SQLite cutover: immutable human/assistant originals, original turn IDs, append-only revisions and source refs, with no lossy JSONL replacement.
- [ ] **R02-I2** Complete migration/backup/restore/export, profile isolation, interrupted-write reconciliation and source-correct history paging (including long conversations).
- [ ] **R02-I3** Complete M3 indexed lexical/semantic retrieval, cross-chat scoped source IDs, page continuation and read-only operational checkpoint recall; keep retrieval bounded and visibly attributed.
- [ ] **R02-I4** Complete local-only embedding worker lifecycle, quality/freshness/invalidated-source handling, opt-in and bounded indexing; no unapproved network API.
**G:** ANSF provenance in actual prompt context, source edits/removals, old versions, correctness of migrated media/document refs, recall of missing chat turns, hot-history performance, failed model swaps and restart.
**A:** original-source checksum and immutable revision tests; cross-chat/cross-profile denial; 1000s-turn paging; corruption/migration/restart and rollback fixtures; measured retrieval precision, truncation, semantic ranking and memory/latency. Native Windows/Linux SQLite and React/host tests.

## WORK AREA 03 — Memory reasoning loop, belief/procedure revision and user clarification [P0]
**I:**
- [ ] **R03-I1** Close MEMORY-LOOP-01/02/02B/02C native source/host acceptance: original plan handoff; independent historic feedback; safe continuation classification; operator-visible original-source recovery provenance.
- [ ] **R03-I2** Close 03A/03B exact-head native implementation checks and genuinely exercise the already-added current-goal, pending/deferred clarification, recovery gate, version-pinned procedures, belief-state correction, and CCM in the **real Lu generation path**.
- [ ] **R03-I3** Implement **MEMORY-LOOP-03C**: independent model-comprehension and K/I/A/F failure-attribution bench for real **small** GGUF with/without memory. Compare correct safe next decisions, false verification, citations, wasted rediscovery, tokens/latency/resources.
- [ ] **R03-I4** Link user corrections/retractions and context changes to selective re-retrieval and inert plan revision; a model must not act on superseded, contradicted or deferred assumptions.
- [ ] **R03-I5** Make the 02C recovery provenance UI honest and usable on a controlled Tauri build: separate historic source verdict, current freshness, OS custody, missing originals, and manual reconciliation.
**G:** a model can misunderstand supplied status (previous tiny Qwen baseline 0/18 strict judgments); source evidence absent/unknown does not equal permission; procedure verification must match exact original effect receipt + independently checked method; no old grant/replay. 03B was source-only and inherited 03A/02C gates at handoff.
**A:** native real SQLite tamper/reopen, wrong-chat, corrected/expired beliefs, no-event vs uncertain effect, source revision mismatch, competing claims, context-change blockers, model ignoring host safety classification; snapshot privacy, bounds and no authority. Headless small-GGUF tests allowed; **do not require user's 7B+ model**.

## WORK AREA 04 — K/I/A/F planning, task orchestration and controlled multi-step execution [P0]
**I:**
- [ ] **R04-I1** Consolidate AGENT-01A/B/C/D/E/F/G/H source chain: saved human-origin goal → strict JSON model draft → immutable plan revision → explicit human plan review → fresh human **per-step** authorization → bounded host scheduler/claimed owner → reviewed broker → independent deliverable receipt → conversation progress.
- [ ] **R04-I2** Expand the current source-level **single independently witnessed read-only step** to sequential, dependency-checked, individually approved steps; never carry over a grant from the preceding step.
- [ ] **R04-I3** Close AGENT-01I/J/K/L fresh child-plan lineage, host-safe recovery and OS custody mapping, including durable task-to-conversation timeline and failure/restart reconciliation.
- [ ] **R04-I4** Implement exact plan cancel, pause, interrupts, step claim expiry and changed human-goal invalidation without worker double-dispatch; model's proposal stays non-authoritative.
**G:** step/revision ownership after host restart; one-claim-per-plan scheduler; partial writes across ANSF, task, effect and plan DBs; unsupported plan types; historical approval mistaken for permission; native tool selection vs canned plan. Do not activate unrestricted autonomous shell.
**A:** two+ sequential source-backed steps, dependency refusal, user denial/cancel, stale goal, changed resource, wrong step/claim/revision, extra tool call, worker crash at each commit boundary, restart replay denial, independently verified source and child-OS witness, current-head CI on Win/Linux and an actual controlled small-GGUF agent tool-choice test.

## WORK AREA 05 — Current human authority, approvals and Protobuf/JSON boundary [P0]
**I:**
- [ ] **R05-I1** Finish human-turn authentication, exact resource/effect/operation grants, native approval UX, expiry, revocation, cross-chat isolation, and separate network/credential scope.
- [ ] **R05-I2** Enforce original MBRL on all model/worker/provider boundaries: strict schema parsing → **one deterministic normalization** → up to **two permitted regenerations** → typed failure; bounded Protobuf repair, no effect replays.
- [ ] **R05-I3** Ensure normal Send, Reader, workspace, Git, browser, agent, background, memory and image paths cannot bypass common host authority or inherit untrusted instructions.
**G:** discovery ≠ installation ≠ authority; JSON injection/syntax repairs may accidentally change semantics; old approval rehydration; filesystem path race/reparse points; Protobuf↔JSON repair reliability under real backend streams.
**A:** property/fuzz and malicious-provider-output tests; wrong-target grant, revoked permission, expired turn, cited/untrusted prompt injection, duplicate IDs, JSON unknown keys/nesting, damaged Protobuf, disconnected browser origin, cancellation while approval pending; signed native receipt with no sensitive paths in model output.

## WORK AREA 06 — OS worker custody, cancellation, recovery and resource guardian [P0]
**I:**
- [ ] **R06-I1** Close E1-P03/P08/P08C/AGENT-01L actual Windows Job Object and Linux PGID/delegated cgroup-v2 custody for workers **only where protection is promised**; distinguish cooperative vs escape-capable descendants.
- [ ] **R06-I2** Finish start/suspend/assign/resume, timeouts, kill/reap of all descendants, guardian startup/loss handling, cleanup after host/service crash and forced termination.
- [ ] **R06-I3** Reconcile durable PREPARED/EXECUTING/APPLIED/FAILED/UNCERTAIN records with **real OS evidence**, not merely SQLite state transitions. Add bounded task and resource admission, progress and terminal result acknowledgements.
**G:** historic issues include Windows Job assignment API/worker-start failures and cgroup escape/guardian-loss edge cases; current branch state must be rechecked. A process-group kill does not prove escaped descendant cleanup.
**A:** native Windows Job/AppContainer/SCM and Linux cgroup-v2 tests on runners with required OS privileges; forced host death, setsid/escape grandchild, orphan reaping, wrong PID/job, early guardian death, timeout/revoke/cancel races, future OS witness absent → fail closed, no duplicate effects.

## WORK AREA 07 — Reviewed external application adapter and independent outcome oracles [P0]
**I:**
- [ ] **R07-I1** Complete E1-P01–P07 reviewed app identity/version, scoped worker launch, real Git/FFprobe use, bounded stdin/stdout/stderr, model tool choice and durable original turn/effect correlation.
- [ ] **R07-I2** Close E1-P09–P13 source-revision custody and independently checked Git HEAD/branch/status/packed history and WAV/MP4 source oracles; method-specific VERIFIED/UNSUPPORTED/CONFLICT results.
- [ ] **R07-I3** Close E1-P14/15 truthful per-turn audit/progress/recovery projection, checkpoint-to-effect links, safe next-action proposal; no auto-resume on UNCERTAIN or a missing independent witness.
- [ ] **R07-I4** Extend only justified specialist adapters using *existing* host execution boundaries; no new mini Office/Git apps in Core.
**G:** source identities may change between authorization, worker start, and oracle check; status of packed-object and MP4 parser historical fixes may be branch-specific; `TaskStore::resume` alone is not safe execution continuation.
**A:** real Git repositories (clean/dirty, packed refs/objects, merge DAG), WAV/MP4 fixtures, changed source while running, deliberate receipt mismatch, actual FFprobe subprocess and Git command choice by a small genuine GGUF, dual-journal restart fixtures and truthful UI badges; method-dependent bounded outputs and no invented verification.

## WORK AREA 08 — File discovery/read/write/rename/trash/restore [P0/P1]
**I:**
- [ ] **R08-I1** Finish existing first-party scoped read/list/stat and indexed/recursive search integrations with source freshness and source ID links.
- [ ] **R08-I2** Integrate existing approved create-new, exact edit/diff, undo/rollback, copy/move/rename, trash/restore and durable receipts with ordinary or explicitly reviewed desktop flow.
- [ ] **R08-I3** Close missing bounded directory/compound transactions **only if** required by practical workflows; do not expand into a general shell/IDE.
**G:** Windows reparse points/ACL/file IDs, cross-volume moves, source-revision race and crash after partial publish/unlink; missing current-authority normal-chat adapter in some stacks.
**A:** actual OS filesystem tests on Win/Linux with symlink and NTFS cases, no-clobber/atomic writes, concurrent mutators, ACL denial, crash between staging/publish/rollback, repeated restore, wrong chat/target; independent hash/contents oracle.

## WORK AREA 09 — Archives and package formats [P1]
**I:**
- [ ] **R09-I1** Reconcile existing ZIP/TAR gzip/xz/bzip2/zstd, CPIO, AR/DEB, RPM, 7z/RAR and ISO adapters with current native worker/approval/recovery layer.
- [ ] **R09-I2** Finish split/multivolume and encrypted/unsupported response UX; only implement complex ISO/UDF if test evidence justifies it. Preserve explicit refusal otherwise.
- [ ] **R09-I3** Add transaction integrity for multi-entry extract/create and cancellation cleanup, without accidental source overwrite.
**G:** helper executable authenticity, archive bomb/symlink/zip-slip, multivolume mismatch, rollback across partial publish and cancellation, format edge-case handling.
**A:** real archive samples each supported type; truncated/encrypted/split, symlink traversal, duplicate members, expansion quotas, separator normalization, race/tamper, helper crash, split-volume reorder; verify extracted exact byte hashes.

## WORK AREA 10 — Universal Reader, OCR, Office, spreadsheets and document creation [P1]
**I:**
- [ ] **R10-I1** Reconcile unified Reader coverage for text, PDF, Office, images, presentations, spreadsheets and paged/large documents; retain original-page/source identities.
- [ ] **R10-I2** Finish scanned-page OCR fallback and Tesseract/provider packaging plus multilingual/layout accuracy and cancellation where supported; do not invent text from inaccessible scans.
- [ ] **R10-I3** Stabilize bounded CSV/XLSX/DOCX/PPTX creation/edit and typed semantic verification; expand rich charts/styles/template fidelity only to measured needs or hand off to specialist Office apps.
- [ ] **R10-I4** Complete format-specific memory/quotas/streaming for large pages, document citations and deliberate invalid-format failures.
**G:** PDF object-streams, complex layout/fonts, spreadsheet formula recalculation, detailed style preservation, slides fidelity and semantic model-output truth.
**A:** actual multi-format source corpus, cross-language fonts, scanned docs, malformed OOXML/PDF, page offsets and large-file ceilings, independent ZIP/XML or PDF semantic oracle, zero-clobber publishing and deterministic SHA artifacts on Win/Linux.

## WORK AREA 11 — Browser, shared transfers and research/citation verification [P1]
**I:**
- [ ] **R11-I1** Finish current host-owned/pinned Chromium runtime discovery and packaging, scoped HTTPS origin grants, headed CAPTCHA user handoff, browser process cancellation/restart and verified browser-to-model return path.
- [ ] **R11-I2** Integrate stable limited research tools: OpenAlex leads, Crossref DOI verification, bounded PDF/HTML passage extraction, redirect identity checks, source quotes and citation audit. Prevent fabricated citations.
- [ ] **R11-I3** Implement reliable multi-step, chat-scoped campaign continuation and terminal notice/export, including up to 120 DOI *leads* without implying 120 verified papers.
- [ ] **R11-I4** Connect shared downloads/transfer manager, concurrency/rate controls, scoped target selection and checked resource ownership only where justified.
**G:** publisher/full-text identity, redirect DNS rebinding, challenge UI, stale page/citation, long campaign interruption, engine route on Wave 4 vs old Browser PR, genuine model synthesis.
**A:** controlled localhost/HTTPS fixtures and real reachable publisher samples with recorded URLs/hashes, no redirected-origin leakage, CAPTCHA pause, forged webpage prompt injection, no citation lacking source passage, in-flight cancel/restart across 5 pages, 120-lead dedupe, offline refusal when unavailable, OS process cleanup.

## WORK AREA 12 — Observe & Learn, privacy and multimodal image work [P1]
**I:**
- [ ] **R12-I1** Close OFF-by-default consent controls, pause/stop, scoped event retention, provenance taint, redaction, deletion and external-data egress denial.
- [ ] **R12-I2** Complete Windows WGC worker→private hashed BGRA→PNG→current-chat imageRef→local GGUF multimodal path; prevent cross-chat image contamination. No raw screenshots in SQLite/WebView.
- [ ] **R12-I3** Stabilize basic image workspace preview, crop, resize, save, history and source provenance; defer professional masks/layers/AI image editing unless justified.
- [ ] **R12-I4** Validate reliable MMProj vision and OCR fallback; do not count image admission as semantic visual recognition.
**G:** OS capture/sensitive windows/consent restoration, actual foreground capture identity, redaction proof, model attach performance, image history missing after restart.
**A:** native Win capture/permission tests on GitHub Windows with controlled visible window when possible, in-memory hash and zeroization, malformed BGRA, wrong owner/failed reaping, no network egress, privacy retention; deterministic PNG and source ID tests; small genuine multimodal model only where CI resources allow (physical 7B+ owner test excluded).

## WORK AREA 13 — Background scheduling, task progress and application feedback [P1]
**I:**
- [ ] **R13-I1** Finalize shared durable schedule/priority/dependency queue, per-chat worker reservations, pause/cancel and terminal notices; not all background tasks should auto-resume.
- [ ] **R13-I2** Connect results to the actual conversation/task progress UI and independently verified completion; preserve partial streaming and per-chat message order.
- [ ] **R13-I3** Reconcile hot-restart adoption, uncertain effects, leaked child work and concurrency while keeping host supervisor distinct from model intentions.
**G:** original 11-task tool suite and separate agent scheduler may duplicate status; worker terminal handshake, watchdog losses, resource pressure, sealed-memory caps.
**A:** many simultaneous chats, duplicate terminal ACK, crash/restart at each transition, CPU/RAM reservation tests, FIFO fairness, progress overflow, partial output persistence, revocation and no unauthorized background turns.

## WORK AREA 14 — Chat, conversation UX, accessibility and model usability [P1]
**I:**
- [ ] **R14-I1** Finish UX-CHAT01 reactions, copy, user-requested safe replay, bubbles/background color and UX-CHAT02 append-only edits, reply links, provenance, changed-message threading, original ANSF history preservation.
- [ ] **R14-I2** Finish editable-message-derived **fresh authenticated** new turns separately from editing a historical display; never replay past tool effects.
- [ ] **R14-I3** Fix streaming/queue behavior during chat switching and cancel, restart-missing responses, input box sizing/persistence, drag/drop/paste image and saved preview restoration, keyboard accessibility.
- [ ] **R14-I4** Improve model load/error cards, source/recovery evidence visibility and accessible color/focus/UIA semantics without allowing model text to set trust badges.
**G:** partial blob wipe after error, misleading model-ready state, conversation-bound queue, stale image/attachment records, mistaken “replay” as effect re-execution.
**A:** React automated state and accessibility tests, Tauri event replay and crash/restart fixture, cross-chat keyboard/clipboard, duplicate edit/reaction, old-source citation, forced worker kill while switching, no lost partial tokens or unauthorized effect.

## WORK AREA 15 — Measurement, reliability and bounded resource benchmarks [P0/P1]
**I:**
- [ ] **R15-I1** Build prospective K/I/A/F matrix: plan-only, host-reviewed execution, and bounded utility, with stable model/quantization/task fixtures and independent outcomes.
- [ ] **R15-I2** Implement MEMORY-LOOP-03C read-only contexts vs no context on small genuine local GGUF; include baseline remeasurement, invalidated memory, plan JSON, source-origin, OS uncertainty and authority denial.
- [ ] **R15-I3** Measure latency, token/context overhead, memory/CPU, tool count, rediscovery, refusal correctness, false positive verified claims, and failure origins (model K vs interface I vs authority A vs feedback F vs environment).
**G:** fixtures/mocks cannot prove model choice; 0/18 earlier Qwen2.5-0.5B comprehension diagnostic is not a controlled comparison to Qwen3; same quantization and backends required for fairness.
**A:** signed model file/config/version hash, repeated fixed tasks, raw candidate replies + source digests, independent oracle/negative cases, sequential reruns and rejection of false improvement claims; no physical 7B+ execution prerequisite.

## WORK AREA 16 — Packaging, legal, versioning and CI-only release validation [P1; physical gate excluded]
**I:**
- [ ] **R16-I1** Produce reproducible Windows NSIS and Linux test candidate artifacts from exact locked source with pinned native engine/libs; retain accepted alpha separately and accurate versioning.
- [ ] **R16-I2** Complete dependency/SBOM/third-party license/source notice review, redistributability, signing identity decision record, installer cleanup/rollback and no silent auto-install of extras.
- [ ] **R16-I3** Run disposable hosted Windows/Linux packaging smoke and uninstall/upgrade/uninstall artifacts when CI infrastructure allows; preserve exact binaries and hashes.
**G:** Windows RC paths/brand, model runtime packaging, missing verified browser/LSP/helper payloads, SHA-pinned source/legal notices, final publisher signing approval.
**A:** PE/NSIS/LSP/native resource bundle compile and hash, checksum provenance, scripted install/uninstall in disposable CI runner, symlink/traversal safety, crash and stale-source tests. **Explicitly exclude** manual owner Windows 7B+ GGUF, Radeon/Vulkan benchmark and final interactive signoff.

## WORK AREA 17 — Lu IDE: independent source/PR topology, project evidence and working set [P1; separate branch stack]
**I:**
- [ ] **IDE17-I1** Refresh `ide/PROJECT_STATE.md` and IDE PR graph on **IDE branch**. Keep it independent of Lu Core until the owner authorizes live integration.
- [ ] **IDE17-I2** Close B1–B11 source integration: find/replace, revisioned unsaved buffers, private draft references/restart, two-pane editor, large-project working set, asynchronous tab admission, workspace navigation, safe clean-tab reclamation, disk-drift detection and reviewed external three-way conflict resolution.
- [ ] **IDE17-I3** Complete A1/A2/A3/C6/C7 project source-span identities, host capability catalog, current Git/worktree and artifact status, project operational ledger and task-to-source evidence without trusting repository instructions as permissions.
**G:** source revision differs from disk/unsaved buffer, performance under large projects, stale drafts, original changes between preview/save, mixed active projects, tracked local Git vs external GitHub evidence.
**A:** native Win/Linux and frontend tests, multi-project/tab stress, 1000s-file open/read/edit, CPU/memory measurements, exact-file SHA stale refusal, clean-tab reclamation, restart-durable draft IDs and linked source-scope denial; no owner physical 7B+ GGUF.

## WORK AREA 18 — Lu IDE: protected language services, builds and Jobs [P0/P1]
**I:**
- [ ] **IDE18-I1** Close DEV-02 protected Rust/clangd/Python diagnostics/completion/hover/navigation, pinned reviewed LSP binaries and actual protocol tests. Preserve distinction between Ruff linting and a real Python semantic provider; complete BasedPyright/ty comparison/choice.
- [ ] **IDE18-I2** Close DEV-03.1–03.5 runtime profiles, safe project build/test discovery, signed/reviewed offline language runtimes, real protected Cargo/CMake/Python/Clang tasks, bounded stdout/stderr streaming and durable worker/Jobs cancellation and recovery.
- [ ] **IDE18-I3** Finish separate native Linux cgroup-v2 and Windows SCM/AppContainer/Job custody paths, revocation and measured process cleanup; do not run a project executable merely by opening it.
**G:** stale compiler/toolchain visibility, Python LSP semantic support, subprocess descendants, terminal log quoting/truncation, out-of-date resources, dynamic project settings and partial JSON task logs.
**A:** actual LSP `initialize`/textDocument diagnostics/hover/definition with pinned server and OS process rights; reproducible Cargo/CMake/Python builds and test discovery; stdout streaming, cancel/restart, source revision/target identity; hosted Win/Linux exact-head native tests.

## WORK AREA 19 — Lu IDE: protected project debugger, source mapping and authorization [P0/P1]
**I:**
- [ ] **IDE19-I1** Close DEV-04.1–04.5D4C real Cargo binary receipt → executable debug information identity (ELF/DWARF, PE/PDB) → new human-approved LLDB-DAP launch → correlated `Stopped` frame → independently verified loaded process/thread PC and source line.
- [ ] **IDE19-I2** **Key unimplemented trust boundary:** Independently authenticate LLDB's actual Windows DEBUG_EVENT ownership/stop, not merely an agreeing Job member thread RIP; keep `lldb_debug_event_owner_attested=false` until proven.
- [ ] **IDE19-I3** Only *after* OS/backend provenance gates pass, design/implement tightly protected breakpoints, stepping, scopes, variables, evaluate (initially read-only); preserve fresh user debug authorization for every risk class and no arbitrary attach/runInTerminal.
- [ ] **IDE19-I4** Verify cancellation/restart, consumed grant denial and separate build/debug durable journal receipts including source maps and PDB identity.
**G:** even correlated LLDB DAP + Job RIP + symbolizer is not its own kernel debug-event attestation; wrong PDB GUID, PE relocation, multiple threads, stale symbols and deleted compiler source need negative handling. Source implementation on IDE draft PR #189 not equal passed Windows debugging.
**A:** native real Windows SCM/Job LLDB `--project-debug-line` plus cancellation mode, native Linux cgroup stopped ELF/DWARF, wrong PC/file/PDB cases, journal close + reopen and no replay, CI-only signed/verified symbolizer bundle. Keep breakpoints/stepping/variables disabled until stronger gate.

## WORK AREA 20 — Lu IDE: AI proposal path and separate app release hygiene [P1]
**I:**
- [ ] **IDE20-I1** Maintain human-first precise source proposals, multi-hunk preview, explicit stage-vs-save, immutable original revision and conflict refusal; repair remaining safe merge/proposal inbox gaps without letting model grant save permissions.
- [ ] **IDE20-I2** Finish reproducible standalone IDE Windows/Linux packaging and license/bundle verification, artifact hashing, service/cgroup cleanup and controlled CI install/migration checks.
- [ ] **IDE20-I3** Define but **do not activate** future Lu Core↔IDE first-party host capability/IPC bridge unless scope is explicitly authorized; same rule for NVP/Cognitum integrations. AF_UNIX-first portable service architecture is a future separate work order.
**G:** signed release chain, editor multi-project SDK provisioning, private profile migration, model-to-IDE authority boundary and code-proposal actor identity.
**A:** no-clobber proposal edits, edit-after-stale-source rejection, human stage/save denial, rollback and persistent provenance; Windows/Linux CI reproducible app package, license inventory and crash fixtures. No requirement for owner's installed local Lu/7B+ model.

## WORK AREA 21 — Future DLC interoperability register [P2; documentation/gate-only for now]
**I (only if current owner authorization allows):**
- [ ] **R21-I1** Reconcile `DLC-API-00..07` / `DLC-IPC` issues into a separate approved design for optional first-party Lu↔IDE/NVP/Cognitum host services: exact actor, model/provider ownership, bidirectional IPC, capabilities, resource handoff, process custody and revocation.
**G:** shared RAM/VRAM model unload/reload coordination, external service trust, rollback and differentiated resource limits. NVP, Cognitum and Single-Galaxy have **their own** backlogs; they are not to be implemented inside Lu Core as part of this Work order.
**A:** protocol/schema validation and host-only simulated IPC; no live DLC bridge or speculative feature expansion without authorization.

---

## Non-physical acceptance program — execute as a matrix
These gates are **in scope** and must be evidenced independently, even where there is already source code:

| Gate ID | Required evidence | Work areas |
|---|---|---|
| **A01 Exact source** | All relevant PR parents and head SHAs, source-tree lock, compiled tree and input binary hashes agree; current CI is successful, skipped/failures reported | 00, 16–20 |
| **A02 Native build** | Windows/Linux `cargo test`, `cargo check --locked`, Node/React, integration suites on exact head; no stale assertion substitution | 00–20 |
| **A03 Original ANSF** | Migration, source IDs, revision/permission isolation, backup/restart, invalidation and bounded recall | 02–04 |
| **A04 Independent source/outcome** | Actual Git/FFprobe/media/docs output verified independently from original worker report; contradictory/missing witness fails | 03, 07–12 |
| **A05 Fresh authority** | Exact current human turn/resource/effect, expiry/revoke, no old grant, no cross-chat/recalled instruction permission | 03–08, 13, 17–20 |
| **A06 Native OS custody** | Real Windows Job and Linux PGID/cgroup-v2 descendants, cancellation/host crash/forced kill, kernel proof separately | 06–07, 13, 18–19 |
| **A07 Durable recovery** | Task + effect + approval + conversation IDs reopen consistently across partial commits/restarts; UNCERTAIN never auto-replayed | 02–08, 13–14 |
| **A08 Tool behavioral quality** | Real filesystem/archive/Office/OCR/browser/research inputs, hostile fixtures, independent output checks | 07–12 |
| **A09 Small genuine GGUF** | Controlled real pinned **sub-7B model** does planner vs read-only tool/verified feedback, measured correctly; no canned response counted as model test | 01, 03–04, 07, 15 |
| **A10 UI logic** | React/Tauri memory/recovery/approval/chat/persistence state, accessibility and no false status; controlled virtual/mock UI alone not physical accepted | 03, 13–14, 17–20 |
| **A11 IDE native** | Verified LSP protocols, protected builds/tests, LLDB source-line and OS debug event boundary, journal close/restart | 17–20 |
| **A12 Delivery safety** | CI-built artifact/SBOM/licensing, source/hash identity, disposable runner package smoke; no manual local-PC 7B+ required | 00, 16, 20 |

### Explicitly deferred — record but do not ask the owner to perform
- **D01** Physical local-PC installed Lu interactive acceptance, signed end-user release approval and manual NSIS upgrade/uninstall.
- **D02** Physical local-PC **7B+ GGUF** (including user's preferred Qwen 3.8 27B A3B) inference, MMProj vision, AMD Radeon Vulkan/ROCm resource benchmarks and personal-device UIA testing.
- **D03** Broader model/provider and DLC integrations without direct owner scoping; unrelated NVP/Single-Galaxy/Cognitum implementation.

## Work execution strategy for ChatGPT Work
1. **Phase P0-A:** Work Area 00 — exact PR/branch/gate inventory and source/lock reconciliation. Inspect all *completed* existing jobs before changing source. Do not create dozens of redundant workflows.
2. **Phase P0-B:** Areas 03, 04, 05, 06, 07 — close memory/authority/oracle/lifecycle; repair real CI failures; establish bounded read-only actual reviewed tool loop and safe continuation across restart. Keep 02C/03A/03B parent gates visible.
3. **Phase P0-C:** Areas 02, 08, 13 — original ANSF/workspace/file/worker/recovery integration with real fixtures; create specific task-to-outcome receipts.
4. **Phase P1-D:** Areas 09–12, 14 — close only material functionality and end-to-end quality gaps of *existing* tools, media, browser, research, Observe and UX. No speculative additional mini-apps.
5. **Phase P1-E:** Areas 01, 15, 16 — small actual model K/I/A/F benchmarks, load/release robustness, legal/artifact packaging. The physical owner 7B+ test stays deferred.
6. **Parallel independent IDE lane:** Areas 17–20 — start with exact-head Windows/Linux native failures on PR #189 and existing IDE CI; then complete safe editor, LSP/protected build, debugger OS proof, signed artifact checks. Do not cross-pollinate IDE branch into memory PR or change Lu Core service contracts without review.
7. **Deferred documentation:** Area 21 — record owner-approved API future only.

### Every task completion must return
- Task ID(s), specific files and actual behavior changed; PR URL/base/head SHA.
- Commands, OS, real logs/artifact references, test cases passed/failed/skipped, defect fixes.
- Exact original source/receipt IDs and verifiers when effects or model decisions are claimed.
- Source vs native vs real-model vs CI package/host evidence separately, without false promotion.
- Residual gaps, independent blockers, and the next unblocked task. Continue implementing other unblocked tasks while unrelated CI is queued; do not claim asynchronous completion without an actual scheduled workflow.

**Definition of done for this Work assignment:** A consolidated, source-grounded all-work-area backlog, real code fixes and verifications as available, exact-head acceptance matrix with honest OPEN gates, and a refreshed canonical development ledger. The owner’s physical installed Windows/7B+ GGUF acceptance is intentionally not a blocker for these tasks.
