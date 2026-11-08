# Lu v1.0 tracked task list

**Owner test scheduling — November 7, 2026:** Defer routine **real-world physical Windows installed-app testing** (actual Lu.exe/WebView/AMD Vulkan, two GGUFs, NSIS install/uninstall, signed/reviewed release candidate) until close to **Lu v1.0 launch**. Continue source development and native GitHub Actions Windows/Linux compile and regression tests now. This postpones *when* manual release acceptance runs; it does not waive, pass or substitute for any missing physical evidence. Do not repeatedly block other source work on an unavailable personal Windows device.

**Workflow decision 2026-11-03:** Develop locally; use GitHub CI to check committed source. No routine physical Windows retests. Keep a deferred manual acceptance ledger for the near-v1.0 release candidate. An accepted functional alpha is not a production-stable v1.0.

**Accepted baseline:** `0.1.0-alpha.2`. **Local development:** `0.1.0-alpha.3-dev.1` (icon resource and byte-counted GGUF import changes). Never overwrite the accepted installer; every change gets a new version/commit and CI evidence. The repository application tree is still older than the local source snapshot: **REPO-01 is a blocker to claiming current-development CI coverage**.

## Tasks

| ID | Priority | Task | State | Proof needed |
|---|---|---|---|---|
| REPO-01 | P0 | Synchronize complete local development source with repository; verify manifest and artifact hashes | BLOCKED | Exact source identity, review and commit |
| CI-01 | P0 | GitHub Actions source identity, frontend tests, Rust core tests, Windows host compile | IN PROGRESS | Successful run on *converged* commit |
| WIN-ICON-01 | P1 | Windows executable native icon and taskbar grouping | LOCAL DONE | PE icon resources tested locally; Win10/11 taskbar check deferred |
| WIN-IMPORT-01 | P1 | Byte-measured GGUF import, validation, errors and cancellation | LOCAL DONE | 4/4 native tests locally; GUI check deferred |
| WIN-LOAD-01 | P0 | Investigate intermittent Windows Not Responding during llama GGUF load | IN PROGRESS | GUI heartbeat and host/engine event traces; physical check deferred |
| WIN-LOAD-02 | P1 | Actual model load progress | TODO | Only engine-measured data may drive percentages |
| MEDIA-01 | P1 | Reliable saved image preview rehydration | TODO | Restart, retry, missing media cases |
| MODEL-01 | P1 | GGUF/MMProj profile, engine logs, GPU offload evidence | IN PROGRESS | Parser/integration tests; physical Vulkan check deferred |
| DATA-01 | P0 | ANSF/SQLite conversations, immutable revisions, migration and backup | PARTIAL | Crash/restart/migration tests |
| AUTH-01 | P0 | Authenticated user-turn tool scopes and revocation | PARTIAL | Adversarial authorization and live desktop grant/dispatch tests |
| LIFECYCLE-01 | P0 | Native process lifecycle, cancellation, clean termination and resource safeguards (not mandatory sandboxing) | TODO | Actual Windows start/stop/reap and crash recovery |
| TOOLS-01 | P0 | First-party native read, search, atomic file writes, git, archives and JSON | TODO | End-to-end authorized Protobuf calls |
| E1-APP-EXEC | P1 | Reviewed application identity, scoped discovery, contained first-party worker execution, independent Office output and genuine benchmark | P01–P05B SOURCE CANDIDATES, stacked draft PRs #103 / #105–#110; hosted native Windows/Linux, real GGUF and installed OS acceptance OPEN | Exact-head REPO-01, hosted real-worker P03/P04B tests, genuine native/Qwen P05 trial evidence and P05B cross-language semantic witness; generic third-party adapters/containment still unimplemented; physical installed Windows deferred near v1.0 |
| TOOLS-02 | P0 | **Original MBRL**: model/API JSON strict validation, one normalization, two regenerations, typed rejection | SOURCE EXTENDED; CI PENDING | No user-facing JSON tool; model/proposal/SSE borders, typed Protobuf, scoped authority, no effect replay, structured feedback |
| TOOLS-03 | P0 | Git status/log/diff under Jan-first narrow schema and Code-task scope | SOURCE PROTOTYPE; CI PENDING | Original §3.9/§5.2 cross-check, typed native Protobuf, real repo tests, hostile Git config/process lifecycle, Linux/Windows, desktop authority |
| TOOLS-04 | P1 | Archive listing/inspection before archive extraction or creation | TODO | Zip-slip, symlink, decompression bomb limits, duplicate members, OS CI |
| JOURNAL-01 | P0 | Durable effect journal, idempotent replay and cancellation | TODO | Kill/restart/uncertain-effect reconciliation |
| MEMORY-01 | P0 | Working/object memory, provenance and belief revision | TODO | Editable and independently checkable retrieval |\n| CONTEXT-01 | P0 | Canonical recoverable goals, provenance, cross-chat context and task handoff (never authority transfer) | DOCS RECOVERED | Durable Lu Core continuity + host-authenticated recall integration and crash/restart tests |
| AGENT-01 | P1 | Durable agent orchestration, loop guard, progress cards | TODO | Multi-step failure recovery |
| UX-01 | P1 | Chat, diagnostics, accessible UI and streaming hardening | IN PROGRESS | UI automated tests |
| RELEASE-01 | P0 | Signed NSIS installer, upgrade and independent v1.0 E2E | DEFERRED | Near-v1.0 Win10/11 acceptance |

## Change tracking convention
Each change records task ID, commit SHA, tests (command/platform/results), CI run URL, failures, and deferred manual scenario. `DONE` means evidence exists for the stated scope only. Never treat mock/source-string tests as physical Windows evidence.

## Execution order
1. REPO-01 and CI-01 (converge exact source; CI must identify build).
2. WIN-LOAD-01, model/GUI observability, DATA-01.
3. AUTH-01 plus trusted effect-specific authorization and LIFECYCLE-01 before broad native TOOLS-01; mandatory sandboxing is not required.
4. JOURNAL-01, MEMORY-01, AGENT-01.
5. UX/installer hardening and RELEASE-01.

See [Deferred Windows acceptance](LU_V1_0_DEFERRED_WINDOWS_ACCEPTANCE.md) and [v1.0 design reconciliation](LU_V1_0_DESIGN_RECONCILIATION_20261103.md).

## Latest local engineering receipt — 2026-11-03
- **WIN-LOAD-01:** Local development source now records sanitized `DX-M-START` immediately before supervisor model load; existing `DX-M-READY` / `DX-M-FAIL` complete lifecycle. Timestamped receipts survive restart and do not expose raw prompts, paths, or keys.
- Independent Rust diagnostics regression **3/3 passed** (one test caught reverse-chronological API ordering; expectation fixed).
- Windows x64 cross-target Rust check **passed**; frontend **85/85** and source checks **29/29** passed; local release-identity check passed.
- Linux full desktop host test not run: GTK/gobject native SDK absent; the isolated native module suite is separately verified.
- **No physical Windows retest performed**, per owner instruction.
- Current local source snapshot SHA-256: `eb0153c15a287ae00aea24b573eb6003ca7f489bf315fccfa5fc7ab8f820bac8`. This ZIP is **not yet synced into GitHub**; CI on this branch must fail source-identity until convergence.

## 2026-11-03 continuation — source-convergence blocker and local foundation work

**REPO-01 stays BLOCKED / incomplete.** Local source archive contains 251 entries and is identified by SHA-256 `946bc0eded3f619de2b2c8ccd42f35ef23da976a89219a08fdd3cb06b9dbe76a`. A 249+ file source manifest was generated locally. Current tool environment cannot connect Git directly, and the GitHub connector offers text-file operations without a bulk archive upload. The source archive was **not** committed to the repository; do not interpret CI results as coverage of it.

**WIN-LOAD-01:** Local patch adds a bounded native watchdog to record `DX-M-WAIT` every 10s during a long load, with a stop signal before the ready/failure event. This is observability, **not** a fix for Windows Not Responding and **not** actual load progress.

**AUTH-01:** Local ChatStore now implements Lu Core's `TrustedUserTurnVerifier` using an exact host-generated generation ID from a real persisted User journal record. Invalid IDs, wrong conversation and duplicate IDs are rejected. Wiring into privileged tool admission is still TODO.

**DATA-01:** Added a host-only ANSF mirror adapter that append-only stores the exact verified User source and compares immutable existing source on replay. Live desktop integration is intentionally disabled until Rust compilation, migration and failure injection pass.

**Verification:** local Node test suite 87/87 passed. Rust compilation and native ANSF/tool tests remain **unverified** in this session (Rust toolchain unavailable); GitHub CI has not yet tested these patches. Physical Windows tests remain intentionally deferred to near v1.0.

**Next action:** synchronize and review every file of the exact source archive via an authenticated repository worktree, verify its manifest/hash, then run all GitHub Actions jobs against that new commit before accepting the new native foundation.

**CI portability correction:** Removed developer-local `src-tauri/.cargo/config.toml` pointing to `/mnt/data/.../vendor`, and excluded it from Git. The published snapshot now has 249 manifest-tracked source files plus the manifest itself. Manifest verification passed for every file. Native Rust compilation remains pending.

**REPO-01 manifest gate:** CI now explicitly invokes `lu-desktop/scripts/verify-source-manifest.mjs` before version checks. Complete local snapshot contains **250 manifest-tracked source files** plus its manifest; verification passed locally. The manifest and verifier must be pushed together for CI to move past fail-closed source identity. This is NOT a hosted CI pass.

## 2026-11-03 continuation — AUTH-01 / DATA-01 hardening and native CI tests

- **AUTH-01:** In the local development snapshot, production `ChatStore::trusted_user_turn_text` rejects an entire source journal if **any** event row is corrupt or unparseable; prior verifier could skip malformed rows while authorizing a valid earlier turn. Added native regression for corrupt JSONL and duplicate generation IDs. Tool permission admission is still NOT wired into production.
- **DATA-01:** Local host-only ANSF bridge now rechecks exact immutable source on a concurrent unique-message insertion race; mismatches fail. Added workspace-mismatch regression. Bridge remains off in live chat pending native Rust verification.
- **CI-01:** Added `tests/native-chat` portable harness on GitHub branch (real production `chat_store.rs`, trait-only stub), a gated native-chat CI job, mandatory source manifest verification, and Windows `cargo test --no-run` after `cargo check`. Green CI is NOT claimed.
- Local Node regression suite **89/89 PASS**; portable source identity check PASS; source snapshot SHA manifest **252/252 PASS**.
- Latest full local source archive SHA-256: `3d0eaa6f6ccf4f56c656c25ef368386c1a39168b5f7366fcbbd078933db30be7`. The source archive is **not pushed**.
- **Native Rust test execution remains unverified** because this working environment lacks `cargo` and `rustc`; full current source is still not on the GitHub branch, so REPO-01 remains BLOCKED. Physical tests deferred until near v1.0.

## 2026-11-03 — portable Rust native tests executed locally

Using recovered user-supplied Rust 1.98 + offline vendored crates, executed the **real** source modules:
- `cargo test --offline --manifest-path lu-desktop/tests/native-chat/Cargo.toml`: **11/11 passed**. Includes the production `ChatStore`, real SQLite `AnsfStore`, and the host-only ANSF mirror. Regression proves corrupted-journal fail-closed, duplicate ID rejection, restart and cross-chat isolation, ANSF idempotency/workspace isolation.
- `cargo test --offline --manifest-path lu-desktop/tests/native-dx/Cargo.toml`: **3/3 passed**.
- Frontend/source suite: **89/89 passed**. SHA manifest: **253/253 files passed**.
- Exact local source archive SHA256: `d459b577ddb27a841f0ee9cec1519a6628a1bd858837fcdc6547dda9b5d2f88b`.
- Native Tauri/Windows host complete build and GitHub CI have **not** run against this source snapshot. REPO-01 remains open; do not call the source synchronized. Physical Windows acceptance remains deferred.

## 2026-11-03 — REPO-01 published tree and WIN-LOAD-01 engine observations

- Published-source integrity: a reproducible `source-tree.lock.json` lists every Git-tracked file by exact blob SHA, and `scripts/verify-source-tree.mjs` rejects missing, added, changed, or index-divergent tracked sources. `lu-repo01-full-source.yml` runs the lock and mutation tests. This proves that **GitHub CI is testing the pinned GitHub source**, not that it matches the unpublished latest local snapshot.
- Cross-branch consolidation: added the unique CI/task ledger/native-chat scaffold from `lu/v1-01-windows-icon-import-20261103`. This is NOT a substitute for the locally verified 253-file source archive described above. Specifically `ansf_bridge.rs`, `native-dx`, and the locally patched `chat_store.rs` have not been recovered from the unpublished archive.
- WIN-LOAD-01: backend now reports actual `engine_spawned` with operating-system PID, `engine_health_200` after authenticated localhost HTTP readiness, `engine_exited_before_ready`, and `engine_health_timeout`; the host stores these in the 96-entry bounded diagnostic trace. No raw engine logs, command line, file path or key is exposed.
- Rust unit regression added for typed engine event callback and PID events; Windows CI outcome must be examined independently.
- **REPO-01 full local-to-GitHub convergence remains OPEN** until the exact last 253-file local development source is available and compared with the committed repository. The GitHub source lock subgate can pass without satisfying that independent criterion.
- No physical Windows or GPU acceptance performed, per deferred-test policy.

## LU-AUTONOMY-01 — capability freedom with enforced limits (2026-11-03)
Design audited against `SECURITY_AUTHORITY_AND_OS_CEILING.md`, `conversation_authority.rs`, `policy.rs`, `native_files.rs`, `journal.rs`, persistent engine and Win32 Job code.
- **PROVISION-01 / P0 NEW:** host-owned capability inventory plus verified user-scope runtime/SDK installation and rollback; never let agent invoke unreviewed installer scripts or demand UAC.
- **ORCH-01 / P0 NEW:** task capability preflight, dependency resolution and durable resume; distinguish unavailable tools, denied authority and missing environments.
- **WIN-JOB-01 / P0 NEW:** engine calls `Job::assign_and_resume_suspended` while inspected Job implementation only exposes `assign`; reconcile and compile Windows process contract.
- **CONTAIN-01 remains blocking:** Job Object process lifetime is not a restricted worker identity or egress/filesystem sandbox.
- **DEV-01 / after core:** sandboxed Lu Dev self-build/test and separately approved stable promotion.
- Audit and acceptance: [LU_AUTONOMY_01_PRODUCTIVE_CONTAINMENT_AUDIT_20261103.md](LU_AUTONOMY_01_PRODUCTIVE_CONTAINMENT_AUDIT_20261103.md). No new implementation or security acceptance claimed by the design audit.

## Owner-corrected autonomy architecture — 2026-11-03
**Binding design:** Lu has unrestricted *access* to the available host environment, not unrestricted *authority* to act. OS permissions do not imply user authorization. Previous mandatory sandbox/restricted worker interpretations are superseded. Use the full native Windows host with trusted task-bound authorization at each consequential action; preserve resource-guardian supervision, journaling, user approval for high-impact operations, rollback and cancellation. See `src-tauri/lu-core/SECURITY_AUTHORITY_AND_OS_CEILING.md`.

| ID | Priority | Task | State | Acceptance |
| --- | --- | --- | --- | --- |
| AUTH-ACCESS-01 | P0 | Distinguish host capability availability, Windows OS permission, authenticated user authority and exact execution admission | DESIGN CORRECTED | Real native tool can access an explicitly authorized host path; identical capability without user delegation is rejected |
| PROVISION-01 | P0 | Discover/provision native compilers, SDKs and dependencies without requiring an artificial container or workspace-only cache | DESIGN CORRECTED | Approved user-wide/system-wide install works through legitimate user interaction; unapproved install denied |
| LIFECYCLE-01 | P0 | Native process supervision, cleanup, cancellation and resource guardian (not universal sandboxing) | TODO | OS-level process lifecycle tests plus recovery receipts |
| ORCH-01 | P0 | Continue authorized multi-step work and resolve prerequisites with durable delegation | TODO | Restart/resume without permission inflation |
| WIN-JOB-01 | P0 | Reconcile pinned llama backend suspended process call with Win Job wrapper API | TODO | Windows compile and engine process cleanup tests |

Earlier CONTAIN-01 text requiring mandatory AppContainer/restricted worker must be treated as **superseded**, not as an unsatisfied requirement to implement universal sandboxing. Historical records remain for provenance.

## 2026-11-03 — WIN-JOB-01 and CI-01 compile unblock pass

- WIN-JOB-01: The Windows llama backend previously used `CREATE_SUSPENDED` but called nonexistent `Job::assign_and_resume_suspended`. It now uses `CREATE_NO_WINDOW` and the existing `Job::assign` directly after spawn, with failure killing/reaping the child. This is **process lifetime supervision only**; there is a narrow pre-assignment window and no claim of race-free containment or mandatory sandboxing.
- WIN-LOAD-01: Corrected a regression where a deliberately noncanonical `load-3` operation ID was expected to survive diagnostic redaction. It now expects `redacted-load-id`, consistent with data-minimization rules.
- CI-01: Replaced nonexistent source verification script with `scripts/verify-source-tree.mjs`; fixed `src-tauri/lu-core` location; removed the invalid assumption that published code equals the unpublished local alpha.3-dev source.
- CI Windows compile tasks now explicitly create **invalid, non-executable placeholder engine resources** solely to pass Tauri resource existence checks. They may not be packaged, distributed or used as inference acceptance evidence. A real NSIS build must stage the pinned four-component Vulkan engine with `stage-engine.mjs`.
- Pending: confirm CI tests for this exact commit, then address failures actually reported by Rust compiler/tests. Published source is still not the private latest local source (REPO-01 external convergence remains open). No physical retest.

## CI-01 / WIN-LOAD-01 follow-up — published-source portable diagnostics

- Fixed portable native-chat harness: real production `chat_store.rs` and `ansf.rs` now compile from the correct published paths. The unpublished `ansf_bridge.rs` is **not** represented as a passing test.
- Added actual `native-dx` harness compiling production `load_trace.rs` directly, with a restart-isolation regression; no Tauri, GPU or mocked trace logic.
- Added independent Linux portable diagnostics job to WIN-LOAD-01 workflow and retained Windows full desktop compile and engine-observer tests.
- This provides quicker test feedback while avoiding false acceptance of unpublished source files. GitHub CI status must still be checked on the exact locked commit.

## 2026-11-03 — CI drift repairs and AUTH-01 persisted identity

- **CI-01:** GitHub Rust compile evidence identified the resilience example missing `GenerationRequest.history` and `PersistentEngineConfig.load_observer`; both initializers updated. No unverified inference acceptance claimed.
- **UX-01:** Frontend CI exposed an obsolete regex asserting `reserve_generation`; adjusted to the actual history-aware reserve API, retaining the queue-before-reservation assertion.
- **AUTH-01:** Production `ChatStore` now stores `user_turn_id` alongside the append-only `User` event, derived from the existing host-generated generation ID. The host `lu_generate` command uses the new append path. Old/imported User events keep an optional absent ID and are **not** trusted.
- **AUTH-01:** `trusted_user_turn_text` fails closed for unknown conversation/turn, duplicate IDs, malformed rows and oversized log; `ChatStore` implements Lu Core's `TrustedUserTurnVerifier` contract. Regression covers legitimate persisted turn, restart, cross-chat denial, duplicate denial, malformed trailing row and legacy context.
- **AUTH-01 remains PARTIAL:** The native tool grant/admission dispatcher is not yet wired to this verifier and no user approval interface is established. This code cannot authorize a tool by itself.
- **REPO-01 remains OPEN:** GitHub's locked tree still differs from the unpublished exact local alpha.3-dev source snapshot. No physical Windows retest was performed.
- Windows job/desktop CI and the above native tests must be verified against the refreshed exact commit before closing their gates.

## CI capacity safeguard — 2026-11-03
REPO-01 and WIN-LOAD-01 GitHub Actions now cancel superseded runs sharing the same workflow/ref, preserving Windows CI capacity on rapid source commits. The latest commit still receives independent source-lock, native-core, portable diagnostics, frontend, native-chat, and Windows checks. A cancelled old run is never reported as green.

## AUTH-01 — authentic desktop journal to Lu Core read-only dispatch (2026-11-03)
- Added `src-tauri/lu-core/tests/desktop_persisted_authority.rs`: compiles the actual published desktop `chat_store.rs` as a Lu Core integration fixture, with an actual OS file outside the chat storage folder.
- Tests require *both* a host-verified persisted user turn (`ConversationAuthority::open_user_task`) and a separately issued exact-effect host grant (`PolicyEngine::grant`) before `dispatch_bound` can read the real file. Unrelated conversation, missing grant, wrong scope, revoked task, and legacy untrusted message are denied.
- Added the required `rand` test dependency to compile the real desktop source. The test must be observed passing in CI before acceptance.
- This is a production-code **integration test**, not a complete production UI integration: no tool approval dialog, native tool choice, dispatch from the GUI or credential/policy persistence are claimed. No mandatory worker sandbox.
- Preserve accepted `0.1.0-alpha.2`; REPO-01 local archive convergence and Win11 physical acceptance remain open.

## AUTH-01 portable harness correction — 2026-11-03

- The initial real-desktop verifier integration test under `src-tauri/lu-core/tests/` introduced a `rand` dev dependency not represented by the frozen core Cargo.lock, correctly rejected by `cargo test --locked`.
- Moved the full **actual `ChatStore` + `ConversationAuthority` + `PolicyEngine` + `NativeToolRegistry`** integration tests to `lu-desktop/tests/native-auth/` as an independently versioned portable harness.
- Restored Lu Core's original locked dependency list. Added a separate GitHub job `auth-portable`; successful identity checks do not count as authorization acceptance until this test passes.
- Proves the *host-authorized dispatch contract*, but no UI approval surface or production event routing is yet enabled. User confirmation remains required for operations not already delegated. Native host access is not a sandbox policy.

## CI regression correction — AUTH-01 corrupt journal fixture
Portable chat-journal CI located a genuine fixture path error in the newly added corrupt-record test: production journals reside at `<root>/conversations/<chat-id>.jsonl`, not `<root>/<chat-id>.jsonl`. Fixed the test path, retaining strict fail-closed corrupted-row verification. Do not count this as a successful test until the corrected commit is exercised in CI.

## CONTEXT-01 — original Lu conversations recovered (2026-11-03)
- Recovered relevant original/Lu conversation decisions and compared against the current repository's alpha acceptance, R22 roadmap, security contract and v1.0 scope. Published [canonical design/handoff](LU_CANONICAL_HANDOFF_CONVOS_01_21_20261103.md) and made it the README entrypoint. This is a selective recovered cross-conversation synthesis, not a complete 21-chat verbatim export.
- **No more user-as-message-bus design:** immutable ANSF source, revision/supersession links, bounded source-grounded retrieval, durable task context/checkpoints and cross-chat *context only* are a core v1.0 acceptance requirement.
- Superseded October 25 sandbox-first audit marked historical; current owner decision is broad native host access with user-delegated authority.
- Next engineered gate: finish real host-approved read-only tool turn, then persist context/authority separately. Context and imported conversation data must never mint tools or extend grant scopes.
- **Still open:** complete historical transcript ingestion, full unpublished local source convergence, source-locked Windows CI and production GUI runtime wiring.

## CONTEXT-01 code continuation (2026-11-03)
- Added exact `ContinuityStore::latest_for_conversation(project,task,chat)` lookup to avoid accidentally returning another chat's task checkpoint when project/task IDs overlap. Existing continuity SQLite remains the source of truth.
- Added independent Rust tests for restart, same-task/different-chat retrieval, immutability conflict and invalid conversation identities. CI pending; does **not** grant tool authority or make ANSF auto-retrieval available in the desktop.

## DESIGN-CORE and CONTEXT-01 consolidation (2026-11-03)
- **DESIGN-CORE:** `docs/LU_CENTRAL_DESIGN.md` consolidates recovered Lu Convos 01–21 decisions, current owner-confirmed access/authority architecture, runtime, native tools, ANSF, continuity, agent workers, GUI, installer, DLC boundaries, acceptance gates and explicit supersessions. README points to this one authoritative source. Old drafts remain supporting provenance, not competing requirements.
- **CONTINUITY AUTH:** Added `ContinuityStore::latest_for_verified_user_turn`; exact current user turn must pass the trusted verifier before read of the exact project/task/chat checkpoint. Recalled context never yields tool authority. Added core regression and a portable integration test with the **real desktop ChatStore**, including wrong-conversation and quoted-turn rejection.
- **CI-01:** The preceding native-core CI failure identified an obsolete `GenerationRequest` construction in `p2_inference_lifecycle.rs` missing `history`. Updated to current typed history field. Source-lock, native-auth, frontend, diagnostics and portable-chat jobs had passed at the earlier snapshot, but complete Windows/native-core and the new changes require verification on this final commit.
- **Remaining:** full GUI approval/dispatch, ANSF automatic retrieval, actual resumed agent orchestration, unpublished local alpha.3-dev source equality, and installed Windows acceptance. Not closing CONTEXT-01 or AUTH-01 yet.

## DESIGN-CORE — v1.0 roadmap consolidation (2026-11-03)
- Confirmed original roadmap in both Library and GitHub: `LU_R22_STRUCTURED_CHAT_AND_ALPHA_V1_ROADMAP.md`. Reconciled it with `LU_V1_0_DESIGN_RECONCILIATION_20261103.md`, the owner-accepted alpha.2 record and this task board.
- `docs/LU_CENTRAL_DESIGN.md` now owns **nine implementation phases (0–8), dependencies, objective acceptance evidence, the original R22 milestone inventory, current task sequence and deferred scope**. This is a fully integrated roadmap section, not just a pointer to the old document.
- Historical R22 alpha conditions remain provenance but no longer reopen the functional alpha milestone accepted by the owner. Original work for native tools, advanced worker recovery and distribution is retained as v1.0.
- User's current broad-host-access / delegated-authority contract supersedes historical mandatory isolation assumptions. End-to-end permission tests and controlled effects remain requirements.
- No code/test acceptance implied by roadmap documentation; statuses live in this ledger and exact GitHub Actions runs.

## TOOLS-01 / AUTH-01 — first live native GUI read (2026-11-03)
- **Implemented, pending CI:** `lu-desktop/src-tauri/src/native_host_read.rs` uses the actual host-selected native OS file, `ChatStore::append_user_with_turn_id`, `ConversationAuthority::open_user_task`, separate `PolicyEngine::grant`, `NativeToolRegistry` + native Protobuf `lu.files.read_text`, immediate grant revocation, and durable chat answer checkpoint/completion.
- Tauri host command `lu_inspect_text_file` accepts a chat ID but **never a model/WebView-supplied path or grant**. A native OS picker selects the file. The GUI adds an **Inspect file** action, displays output via the usual persisted chat history, and does not require a loaded LLM.
- Scope is deliberately one selected file, validated parent directory and filename, with 64 KiB maximum text. This is a per-invocation argument contract, **not** a Lu-wide OS filesystem sandbox. No document content can grant further tools.
- `lu-desktop/tests/native-auth/src/lib.rs` now directly tests the real native-host module, including durable history/restart, duplicate turn refusal, wrong-chat isolation, selected directory refusal, and bounded truncation.
- **Known gaps:** This proves only direct human-picker read, NOT model-proposed tool-call approval UI, full cross-conversation owner identity, per-argument policy beyond fixed picked path, persistent capability ledger, production effect receipts or arbitrary native write/tool execution. The action must be compiled/tested on the committed revision and retested as an installed Windows app nearer v1.0.

## AUTH-02 — model-proposed, human-approved native read (2026-11-03)
- **Implemented in development source; exact-commit CI pending:** `read_proposal.rs` strictly parses a single completed model-response JSON schema: `{kind:"propose_read_file",file_name,purpose,max_bytes}`. It rejects paths/traversal, control characters, excess bytes, unexpected fields, wrong action kind, conflicting/reused proposal IDs and cross-conversation/turn replay. It never issues a tool grant.
- Production Rust `lu_generate` now attempts to stage only successfully completed model responses. On a valid proposal it emits `lu-read-proposal` to the React desktop; ordinary text responses do not stage anything.
- GUI displays inert proposal cards with **Choose and approve file** / **Deny**. Approval is a separate host command that opens a native file picker, verifies the exact requested filename and host-bound origin, consumes the proposal once, and executes through the existing authenticated-turn native read with the **proposed byte limit** (at most 64 KiB). Closing the picker dismisses the request. The WebView cannot supply a file path or policy grant.
- Portable integration tests cover a strict untrusted parser, write substitution refusal, injected unknown fields, duplicate ID denial, wrong chat/turn, selected-file mismatch, single-use semantics and real bounded native text reads. No automated write execution.
- **Outstanding:** CI verification and real Windows UI acceptance, stronger resource identity pinning across native picker → worker open, completed-output identity/ref verification beyond the in-memory generation handler, persisted proposal receipts/recovery, tool schema advertisement to the LLM, richer target-level authorization and long-running worker cancellation.
- `AUTH-02` is **not complete** merely because code was committed; pending models are never auto-authorized, and this only covers a human-reviewed read-only tool.

## AUTH-02 verification snapshot and CI-01 stale fixture repair (2026-11-03)
- GitHub Desktop CI run `41740888531` on prior locked commit `d1de182`: source identity **PASS**, frontend **PASS**, portable chat journal **PASS**, portable authenticated desktop/native dispatch including AUTH-02 **PASS**, diagnostics-portable **PASS**. Windows-host was still in progress at this observation.
- Native-core was **FAIL** due to obsolete `PersistentEngineConfig` initializer in `tests/real_gguf_tool_json.rs` missing `load_observer`; updated it to `None`. The correction must pass on a *new* source-locked commit before native-core is accepted.
- No installed Windows user approval test, file-identity race test, or durable approval/recovery acceptance yet. Do not declare V1-04 complete.

## JOURNAL-01A — native read durable effect + distinct approval decision ledgers (2026-11-03)
- Added `src-tauri/lu-core/src/read_approval_ledger.rs`: native-only SQLite WAL/FULL database of proposal ID, original conversation+generation identity, exact expected filename, read budget and decision state. `PENDING` can transition once to `APPROVED` or `DENIED`; conflicting identities, replay or broader limits fail closed. Existing decisions remain queryable across restarts.
- Production desktop stages the proposal in this ledger before publishing the UI review card and stores the user approval/denial before action. The model and WebView cannot supply the target path, policy grant, or actual approval decision. A consumed in-memory proposal cannot be approved twice. No automatic capability is inherited from the persisted decision.
- Native OS-picked file inspection now optionally dispatches through `ConversationAuthority::dispatch_bound_durable` using host-owned `EffectJournal` at `native-effects.sqlite`: it persists a prepared/executing state and validated Protobuf APPLIED receipt or blocks ambiguous effects as UNCERTAIN. The chat text is persisted separately.
- Added core Rust tests for approval replay, identity mismatch, crash-interrupted pending records, failed/malformed requests and durable read integration tests in `native-auth` for receipt, recovery and bounded reads.
- **Important unclosed gate:** Automatic startup `reconcile_pending()` and `reconcile_interrupted()` are intentionally NOT called until there is verified exclusive Lu process/DB ownership. A second active instance must not mistakenly reconcile a live operation. Pending in-memory proposals are lost on restart (fail-closed), and prior records remain for later explicit reconciliation.
- **Important scope:** This is durable *read* receipts and *decision* audit, not full atomic write/delete/execute idempotence, not durable worker cancellation/kill, and not Windows installed-app acceptance. Real path identity between dialog selection and worker open is not fully pinned. Test verdicts must be obtained from exact-source CI before acceptance.

## JOURNAL-01A approval ↔ effect identity link (2026-11-03)
- Added a nullable unique `effect_call_id` to approval records (including a migration for pre-release journal rows). The trusted host binds the fresh generated tool call ID to the APPROVED model proposal **before** tool dispatch.
- One pending proposal cannot bind to another chat/generation, one approval cannot bind twice, and two approvals cannot share an effect ID. Persisted approval can now be joined to the matching `EffectJournal` result after restart; this is only an audit link, not authorization.
- Added regression cases for restart link survival, ID replay, different-chat collision and end-to-end persisted text/receipt correlation. CI is required before declaring the gate accepted.

## CI-01 follow-up — updated smoke examples (2026-11-03)
- Prior source-locked CI run `41742393532` had source-identity, frontend, diagnostics, portable native-chat, and portable native-auth **PASS**, but native-core **FAIL** on stale example `examples/inference_supervisor_smoke.rs` missing `PersistentEngineConfig::load_observer`; Windows-host was cancelled by subsequent revisions.
- Repaired that initializer, and proactively checked **all Rust examples** under `lu-core/examples/` for both `load_observer` and `GenerationRequest::history`. Corrected `inference_supervisor_smoke.rs`, `persistent_llama_smoke.rs` and `llama_cli_smoke.rs` (alongside the prior `linux_persistent_llama_resilience_smoke.rs` fix).
- Added pre-release approval schema migration test to prove older decision rows survive addition of `effect_call_id`. These changes remain pending native-core and installed Windows verification on their exact source-locked revision.

## JOURNAL-01B — exclusive desktop owner and safe startup reconciliation (2026-11-03)
- Added `src-tauri/lu-core/src/desktop_owner.rs` (native SQLite writer transaction held for process lifetime in a separate lock database). Acquisition is required **before** opening or reconciling ChatStore, ANSF, effect or read-approval SQLite journals. A second instance using the same app data directory is refused while the first retains its SQLite OS lock; a crashed process releases the lock.
- Production `DesktopState::new` now takes ownership before other persistent state is initialized and calls `EffectJournal::reconcile_interrupted` and `ReadApprovalLedger::reconcile_pending` before serving new UI commands. `EXECUTING` becomes `UNCERTAIN`; `PENDING` becomes `INTERRUPTED`; neither can be silently replayed/approved. Other completed/denied records are retained. Lock holder is wrapped in `Mutex<Connection>` for Tauri's `Send + Sync` state.
- Core integration tests exercise two competing owners, lock release/reacquisition, simulated crash with interrupted effect and pending approval, denial of approval restoration, exact-one reconciliation and no blind journal replay.
- **Known limits:** SQLite writer lease ensures *one cooperating Lu instance for this exact data directory*, not an adversarial OS-wide exclusive file lock against other unrelated programs. A desktop process still alive with a stolen DB handle is outside current tests. OS-native handle-pinned file identity between user picker and read remains open; read/write execution safety depends on this future hardening.
- CI acceptance requires green native-core, desktop host, portable auth, source-identity and Windows trace on the exact final source-lock commit. No physical Windows retest until near v1.0.

## TOOLS-01 / JOURNAL-01B — approved native file handle pinning (2026-11-03)
- Added `register_bundled_file_tools_pinned` to Lu Core native filesystem tools. The trusted host opens the OS-selected file once and passes that handle into the native Protobuf file tool. Dispatch rejects mismatched model-controlled filenames and never reopens the target pathname. The read limit and existing per-turn authorization remain enforced.
- The desktop now checks file type against the opened handle (and, on Unix, compares device/inode metadata to detect a swap before handle acquisition). Native file content after approval is obtained only from that pinned handle.
- New `model_native_files` test replaces the approved filename after handle capture and verifies the original bytes are returned, plus mismatched path refusal. Windows uses stable handle testing without assuming filename replacement is permitted by file sharing semantics.
- **Remaining:** Windows native picker → open handle race cannot be completely excluded without an OS-specific no-follow/handle identity open; a Windows adversarial symlink/reparse hardening test is required. Pinned read is not a general write/execute safety guarantee.
- Exact-commit Rust/Windows CI and installed-app acceptance are pending.

## WIN-FILE-01 / WRITE-01 — Windows pinned-file verification and create-new atomic primitive (2026-11-03)
- Fixed committed CI blockers: Windows desktop and WIN-LOAD-01 workflows now build the actual React `dist` before invoking Tauri's `generate_context!`; Lu Core's missing historical `tests/fixtures/upgrade/0.8.3/.../messages.jsonl` test dependency is replaced by an explicitly labeled self-contained Jan-shaped JSONL parser fixture. **This is not a substitute for independent real historical Jan migration acceptance.**
- **WIN-FILE-01:** Trusted desktop opens the selected file using Windows `FILE_FLAG_OPEN_REPARSE_POINT` and `FILE_SHARE_READ`, rejects reparse-point handles, and uses stable final-component no-follow/reparse validation and pins the opened file handle; Win32 file-index comparison remains a separate unclosed hardening step. Unix uses device/inode comparison. The native read remains pinned to the handle across Protobuf dispatch. Added portable tests and Windows-specific handle/rename/reparse checks; Windows-host CI now executes them.
- **WRITE-01 stage 1:** Introduced `native_atomic_write::create_new_atomic`, a bounded (1 MiB), host-only **create-new** primitive using `create_new` staging, `sync_all`, atomic no-overwrite hard-link publication and removal of the staging alias. It refuses path traversal, existing target, stage replay/collision and oversized bytes; returns a SHA-256 receipt. Added Linux/Windows tests for no overwrite, orphaned staging, invalid targets and target content.
- **Critical limits:** WRITE-01 has **no GUI/model tool route, no authenticated authorizer and no effect-journal wrapper yet**; calling this primitive is therefore reserved for trusted host code and cannot be called by the model. A hard-link-capable filesystem is required; other filesystems fail closed. This is process-level safe publication, not a power-loss-durability guarantee, and the parent-directory/temporary-name adversarial replacement race still needs a handle-relative implementation. `PublishedNeedsReconciliation` means the target might exist; never retry without inspecting the journal and target.
- **Still open:** exact committed Rust + Windows CI verification, adversarial file-handle tests on real Windows, authenticated journaled write dispatch, explicit recovery/rollback protocol for a published target and installed-app acceptance near v1.0. Accepted alpha.2 unchanged.

### WIN-FILE-01 stable API correction
- During review, `std::os::windows::fs::MetadataExt::file_index` and `volume_serial_number` were identified as unstable on the pinned toolchain. Removed those calls rather than enabling unstable features or claiming Windows file-ID comparison was complete. The file is still opened with stable `OpenOptionsExt` no-follow/sharing flags, checked for reparse attributes and held through the native read. Full Win32 by-handle ID equivalence testing remains open.

### CI-01 Windows esbuild portability repair (2026-11-03)
- Windows CI runs `41747199414` and `41747199442` reached the frontend build but failed at `spawnSync esbuild ENOENT`: the old bundler spawned a bare `esbuild` executable, whereas Windows npm installs platform-specific command wrappers.
- Replaced `lu-desktop/scripts/build.mjs` CLI process spawning (and its Linux-only path fallback) with esbuild's programmatic Node API, preserving the browser/ES2020/ESM/automatic-JSX/minified bundle and the copied HTML/CSS assets. This makes Windows and Linux use the same implementation and avoids synthetic frontend placeholders.
- The exact locked revision requires fresh Windows CI verification. Neither passing Linux UI tests nor staging a CI-only engine binary substitutes for installed-app acceptance.

## WRITE-01B — typed, approved, durable create-new dispatch (2026-11-03)
- Added `lu-core/src/native_create_new.rs`: the first-party `lu.create_new_file.commit` native tool reuses strict `lu.v1.CeWriteRequest` and `CeWriteResult` Protobuf messages, calls the existing 1 MiB-bounded no-overwrite primitive, and derives its staging identity from the trusted host-issued effect call ID. Registration is inert without a separately authenticated grant.
- Added `ExactCreateNewPolicy`: it verifies the *exact host-approved filename* and SHA-256 of the complete byte payload, as well as the nested host policy grant. It refuses altered contents, off-target paths, oversized requests and unknown contracts. `ConversationAuthority::dispatch_bound_durable` additionally verifies the persisted USER turn, conversation and task scope before `EffectJournal` admits the effect.
- New `src-tauri/lu-core/tests/native_create_new_authority.rs` tests first write + Protobuf receipt, exact byte/target fidelity, cached-success replay without a second write, revocation blocking cached receipts, changed-byte call-ID conflicts, existing-target denial, and simulated crash after EXECUTING transitioning to UNCERTAIN without replay.
- Added a **real desktop ChatStore** integration test to the portable native-auth suite, rejecting another conversation's origin and verifying a host-issued/write-scoped user turn, persistent effect receipt and subsequent task revocation.
- Fixed existing `read_approval_ledger` legacy-schema migration failure (GitHub Core run `41747601196`): pre-release approval rows without `created_at`/`updated_at` are upgraded in place and timestamp-backfilled, preserving the original decision and effect link.
- **Status:** implementation committed, pending full exact-source-lock Rust/Windows CI. **NOT model-callable in the Lu desktop yet.** Only trusted test/host code can register this write tool; it has no GUI review card, no durable write-specific approval decision row, no user-picked writable-directory route, and no handle-relative directory publication. Those remain release gates before exposing arbitrary filesystem writes.

### WRITE-01B CI coverage update (2026-11-03)
- Added an explicit Windows CI command for `native_create_new_authority` in addition to the earlier Windows `native_atomic_write` test. Tests verify real filesystem creation, exact human-bound target and bytes, policy revocation, durable receipt replay, already-existing target behavior, and interrupted-effect refusal. Installed app write flow remains disabled.

## WRITE-01C — desktop review, native confirmation and durable recovery (2026-11-03)
- Added strict, bounded `write_proposal.rs`: a completed model response may propose `{"kind":"propose_create_file","file_name":"...","purpose":"...","content":"..."}`. Unknown fields, traversal, directory selection, duplicate/stolen proposal IDs and bodies over 16 KiB fail closed. The model never supplies a writable directory, effect call ID or permission grant. The pending in-memory bytes are never restored as executable work after restart.
- Added `write_approval_ledger.rs`: SQLite WAL/FULL audit rows bind original model generation, conversation, exact filename, byte count, SHA-256 and host-selected full target to PENDING → SELECTED → APPROVED, or DENIED/INTERRUPTED. APPROVED additionally binds a unique host-issued effect ID. On exclusive-owner startup, incomplete PENDING/SELECTED become INTERRUPTED; APPROVED is audit context only, not permission to replay.
- Desktop `lu_generate` stages only a completed exact-schema response and emits an inert review card. React displays **the complete proposed UTF-8 text**, filename, purpose, content digest and byte count. First click resolves the exact host-owned **Lu Drafts** path and displays it; second click requires a separate **native OS confirmation dialog**, showing exact path/size/digest, not merely a WebView Boolean. Denial is durably recorded. The WebView cannot supply a path, bytes or grant to `lu_confirm_write`.
- On native approval, the trusted desktop creates a fresh real persisted USER turn and dispatches the 1 MiB-capable create-new tool through `ConversationAuthority`, `ExactCreateNewPolicy` and `EffectJournal` with the **16 KiB UI proposal limit**. Existing files are never overwritten, effect/approval IDs are joined, and the result is persisted in chat. An uncertain result warns the user and never automatically retries.
- Added `lu_write_recovery`: per-chat read-only audit shows selected target, expected content digest, approval and effect states, and an independent bounded file observation (MISSING/DIFFERENT_BYTES/MATCHES_APPROVED_BYTES). A matching target is **not proof of which process published it**, and is never used as authority or automatic reconciliation.
- Added core `write_approval_ledger` regressions and real desktop ChatStore integration tests for two-stage review, complete native write, immutable existing file, cross-chat denial, malformed model JSON, replay/restart and bounded recovery observations. Added a Windows CI gate for the ledger. Native confirmation must also compile and be exercised on Windows before acceptance.
- **Not complete:** No arbitrary host-directory write UI, no overwrite/patch/delete, no handle-relative directory publication, no irreversible-effect undo, no physical installed-app acceptance. The initial draft destination is deliberately app-owned until stronger OS-specific directory identity handling is verified. All exact-revision CI jobs must pass before WRITE-01C is accepted.

### WRITE-01C Windows CI follow-up (2026-11-03)
- Windows host trace CI identified a deterministic `cargo --locked` failure after the desktop gained direct `prost = "0.14"`: `lu-desktop/src-tauri/Cargo.lock` still lacked `prost` in the `lu-desktop` package's direct dependency list. Added the existing locked `prost 0.14.4` dependency to that package; no new crate version was introduced.
- Native Windows confirmation uses the Tauri dialog plugin's `blocking_show` on a worker thread, requires positive OS-modal consent, and records a denial if cancelled. Frontend handles that cancellation without announcing a file creation. Windows compile/installed-app consent acceptance remains open.
- Evidence of exact Windows and desktop CI results must be recorded only after those jobs finish on the source-locked revision.

## WRITE-01D — format-aware, user-selected directory writes and conservative reconciliation (2026-11-03)
- **Corrected WRITE-01C CI blockers:** `write_approval_ledger` test had accidentally used the *same* SHA-256 for an approved and purportedly altered payload; corrected adversarial fixture. Windows `lu_write_recovery` E0382 moved fields before using them; moved independent evidence calculation before record ownership transfer.
- **AI-selected formats, host-validated:** new `lu-core/src/write_format.rs` identifies UTF-8 text from the model-proposed filename and content. JSON and JSONL/NDJSON receive actual syntax parsing; Markdown, TXT, Rust, Python, TOML, YAML, HTML, CSV, config, source and other UTF-8 text formats can retain original bytes. Binary/opaque file extensions, NUL bytes, malformed JSON/JSONL and traversal names fail closed. This is **NOT** a full compiler/parser proof of every language or format; application-specific validation is a later extension. `write_proposal.rs` and `native_create_new.rs` both enforce this admission.
- **Actual model awareness:** `prompt_context::model_file_creation_hint` supplies the `propose_create_file` JSON proposal contract and examples only for explicit user create/save-file intents. It remains unprivileged model guidance and does not edit user-authored chat history, issue approval or enable overwrite. Typed tool advertisement in the inference transport is still future work.
- **Beyond Lu Drafts for NEW files:** `lu_review_write_destination` now supports `destination_mode=choose_folder`, which invokes the trusted OS native folder picker. The WebView/model can choose this workflow but NEVER pass the directory path. Lu presents the exact resolved destination and full proposed text to the user and requires independent OS-modal confirmation. `lu_drafts` remains the default alternative.
- **Directory-handle safety:** `WriteProposalQueue` now opens and retains an `Arc<cap_std::fs::Dir>` at folder selection. On confirmation, `execute_approved_create_pinned` passes this handle to the first-party native Protobuf tool. `native_atomic_write::create_new_atomic_in_dir` stages, syncs, hard-links without overwrite and removes staging names **relative to that same opened directory**. This resists post-selection parent-path replacement. It is a per-operation handle capability, not a global Lu sandbox. Existing path-based API remains a wrapper that opens the handle itself.
- **Recovery assessment:** new pure `write_recovery::assess` combines durable approval state, effect state and independently observed target bytes into conservative evidence labels. `UNCERTAIN` + matching bytes remains **not proven**, and no assessment reauthorizes or automatically retries. The GUI displays the classification. External user-selected folders are **not rescanned automatically** by recovery; the audit labels them as external and retains the exact approved target and digest for explicit later review.
- **Regression tests:** `write_format` accepts expected Markdown/JSON/JSONL/source text and rejects malformed structured/binary targets. Unix `native_atomic_write` replaces a directory pathname after handle acquisition and proves publication still occurs to the original directory. Portable desktop-auth tests prove no folder authority arises from model JSON, reject invalid formats and retain the selected directory identity. `write_recovery` tests prove no uncertain case becomes automatic retry or verified success.
- **Remaining critical v1.0 work:** Writing NEW UTF-8 text into the user-selected folder is implemented but requires complete exact-commit Linux/Windows CI and installed-app testing. **Editing/overwriting existing text files is not implemented**: requires a separate authorized replace/patch tool, the existing file's immutable snapshot and expected SHA, verified diffs, backup/undo custody, robust atomic publish/rollback and external-editor concurrency handling. Arbitrary binary formats, multi-file transactions, nested tree creation and unreviewed autonomous writes are not implemented. Filesystem hard-link capability and Windows real-folder-pick acceptance still need verification.

### WRITE-01D CI repair — 2026-11-03
- First exact-source CI `41753425755` passed native-core (including `write_format`, `write_recovery`, and pinned atomic-write tests), source identity, frontend, diagnostics, and portable chat-journal. Portable native-auth and Windows-host both failed compiling the **same** test fixture: an `r#\"...\"#` Rust raw string containing Markdown `# Notes` prematurely closed the delimiter. Changed this fixture to `r##\"...\"##`. This was a fixture syntax problem, not a demonstrated live write defect.
- Fresh complete CI and installed Windows acceptance remain required on the new locked revision before marking WRITE-01D accepted.

## DOCS-01 / EDIT-01 — user Documents defaults, file-location links and corruption-resistant edit policy (2026-11-03)
- **Default Documents, not an arbitrary app folder:** the desktop now resolves `AppHandle::path().document_dir()` from the operating system (Windows user's Documents known-folder location, including redirection; Linux XDG Documents where configured). This is the initial target for **new** text files. No silent fallback to Lu app-data, Downloads or an invented folder. If the OS directory does not exist or has become unsafe, Lu reports an unavailable default and asks for a folder instead of creating an unexpected directory.
- **Advanced Settings override:** `lu-core/src/document_preferences.rs` persists a single optional custom directory in SQLite WAL/FULL. `lu_document_settings` reports current/system paths and availability; `lu_choose_default_document_folder` accepts a directory ONLY via the native OS folder picker; `lu_reset_default_document_folder` restores the OS Documents default. The path is never writable from model output or a WebView-supplied string. Settings displays the chosen folder under the collapsible Advanced pane and saves it across restarts.
- **Every successfully created file has an Open containing folder action:** the UI shows an immediate created-file card and a persistent recovery-record link. `lu_open_created_folder` receives only conversation/proposal IDs, joins the approved write row to the **APPLIED** native create-new effect and nonempty durable receipt, checks the stored target's filename and ordinary parent directory, then launches the OS file manager directly (Windows Explorer, Linux `xdg-open`, macOS `open`) without a shell. Failed, denied, interrupted or uncertain effects cannot produce a verified folder link. No WebView-provided filesystem path is accepted.
- **New edit preflight (NOT an overwrite executor):** `lu-core/src/edit_safety.rs` deterministically compares the snapshotted original to the exact expected SHA-256, validates the proposed UTF-8 replacement and JSON/JSONL where applicable, rejects unchanged/binary/oversized/mismatched edits, produces a bounded change summary and large-change risk classification, and explicitly requires review of the **complete diff**. Its allowed automatic repair attempts are **zero**. Tests cover hash mismatch, malformed replacement JSON, invalid encoding, extensive edits, Unicode-safe diff boundaries and no-op refusal. Existing files still CANNOT be modified by the current native write tool.
- **Mandatory next edit pipeline:** (1) capture file by handle and persist immutable original with a checked digest; (2) produce typed edit proposal/diff, validate encoding/schema and language-specific lint/parser results; (3) require approval of target + original hash + full diff and a reversible backup; (4) verify the original has not changed immediately before publish; (5) atomically commit with durable journal and post-commit verification; (6) expose explicit recovery/undo if incomplete. No automatic AI fix→rewrite→fix loops, and never overwrite a changed file without fresh approval. A failure should return structured evidence for a human-authorized new attempt.
- **Acceptance:** implementation and tests committed on development branch; exact locked native-core/frontend/portable-auth/Windows CI must pass, with installed-app verification near v1.0. Legacy Lu Drafts files are left where they were and are NOT silently moved or deleted; their prior audit records remain available. User override does not alter prior selected targets. The accepted `0.1.0-alpha.2` build is unchanged.

### DOCS-01 Linux directory resolver hardening
- Resolve the Linux user's XDG Documents path through the OS/Tauri resolver; if the resolver cannot supply one, use the conventional `$HOME/Documents` path rather than failing desktop startup or silently redirecting into Lu's app-data folder. This fallback is never auto-created or written without directory availability checks and the existing exact-file approval. Windows continues using the known-folder resolver, preserving Documents redirection.

## 2026-11-03 — WRITE-01E approval-bound exact edit continuation
- **Source branch:** `lu/write-01e-approved-edit-gate-20261103`, based on the complete published WRITE-01D / WIN-LOAD branch, *not* default `main`.
- **WRITE-01D recovered:** `edit_safety.rs`, `write_format.rs`, `write_proposal.rs`, `native_host_write.rs`, document preferences and create-new ledger already exist. Do not create competing authoritative editing modules. A separate prototype `reversible_edit.rs` on `main` is not the integration target.
- **WRITE-01E:** Existing `edit_safety.rs` extended with `bind_reviewed_edit` and `verify_reviewed_edit`. Cryptographic review digest domain-separates and length-prefixes trusted destination, full original bytes and full replacement bytes. Recomputed planning checks plan-field tampering and syntactic validity. Fresh-source mismatch and replay on other destinations are rejected. Regression tests added in the module.
- **Evidence:** source commit `86708cfac18131e81c87bc70d75cf6f07eac4280`. Source-lock update `c999e8c607b2a54fa1ecebef49b3430537d00f90`. No local cargo/rustc available. No green CI result verified; tests **pending**.
- **Boundary:** This is *pure preflight*, not desktop-enabled editing. Trusted host user approval, pinned target handle, crash-safe backup, journaled atomic publish, reconcile, explicit rollback and independent Windows filesystem adversarial tests are **not implemented/accepted**. A model may not fabricate `trusted_approved_sha256`; host must establish that value from a real reviewed user decision.
- **Next:** Implement host-bound review handoff and durable backup/publish/rollback inside existing trusted native tool path. No blind retry on uncertain outcome. Do not silently change the user Documents-folder default or Advanced Settings override.
- **REPO-01:** This source-lock verifies tracked source within the branch; it does not resolve the last unpublished local source archive convergence.

## 2026-11-03 — WRITE-01E.2 native effect implementation
- **Implemented in branch, not GUI-enabled:** `src-tauri/lu-core/src/native_edit_existing.rs` registers `lu.edit_existing.commit` and `lu.edit_existing.rollback` as host-scoped first-party Protobuf WriteWorkspace tools. `ExactEditPolicy` checks reviewed original/replacement/target and defers to the authenticated `ConversationAuthority` + `PolicyEngine` grant; `dispatch_bound_durable` uses the existing SQLite `EffectJournal`.
- **Durable artifacts:** same-directory create-new intent, original-byte backup, synced replacement stage, last-source check, capability-relative rename and readback receipt. Read-only edit/rollback recovery observations; no blind re-execution. Explicitly approved rollback has its own intent/stage and journal effect ID.
- **Test source:** new `tests/native_edit_existing_authority.rs` integration through production Rust authority/registry/Protobuf/journal. Additional in-module crash-barrier, duplicate ID, symlink/hardlink, conflict, tampered backup and rollback tests. Windows CI explicitly includes the integration test.
- **Not yet accepted:** No Rust/Windows CI pass established in this chat; the desktop does not yet offer a trusted full-diff approval workflow. Non-cooperative cross-process edit race, Windows directory durability, file ACL/ADS/xattr identity and reparse-point behavior need hardening. Full local-source REPO-01 convergence remains OPEN. Do not call WRITE-01E DONE or enable model edits until integration acceptance.
- **Implementation explanation:** `docs/WRITE_01E_DURABLE_EDIT_TRANSACTION_20261103.md`. Auth and journal semantics remain separate; failure is UNCERTAIN and needs reconciliation, never model repair-loop retries.

## WRITE-01E.3 — Desktop edit approval/recovery continuation (development branch)
- Model may output a **strict inert** `propose_edit_file` JSON with basename, purpose and replacement UTF-8 text. New `edit_proposal.rs` maintains a bounded queue; the real existing file is chosen **only** through a native OS file picker. Selection binds a verified original snapshot, pinned parent-directory handle, full replacement, SHA-256 baseline and `edit_safety` review fingerprint.
- Desktop UI displays **complete** original bytes (64 KiB review limit), complete replacement (16 KiB model proposal limit), all changed lines and exact destination. User-facing acknowledgement is separate from a native confirmation; neither model text nor a WebView boolean is accepted as effect permission.
- A new `edit_approval_ledger.rs` SQLite record preserves stage/selection/approval/denial and unique effect call ID, with pending review invalidated after restart. `native_host_edit.rs` creates a trusted persisted user turn and dispatches through the existing `ConversationAuthority` + `PolicyEngine` + `ExactEditPolicy` + Protobuf registry + durable `EffectJournal`.
- Read-only recovery panel shows approval/effect separately and requires reselecting the original **containing folder** through a native picker before checking on-disk transaction artifacts, including a missing target. Explicitly does **not** automatically retry or approve a rolled-forward uncertain effect. Applied edit receipts can open the containing folder through a verified ledger link.
- Prior Windows CI error E0282 in create-file recovery `observed_target` type is explicitly fixed on this branch.
- **Acceptance pending CI:** compile and run full React/UI, Rust core, Windows native tests and desktop Tauri compile on the exact new source lock. Windows NTFS directory sync, metadata retention, cross-process races, installed-app OS dialog UX, and independent physical Windows acceptance remain open. Never merge the draft PR or enable an accepted release on source changes alone.

## 2026-11-03 — WRITE-01E.3 accepted CI baseline; WRITE-01E.4A undo inspection
- **Recovered after connection interruption:** revision `13cf8de2bace3ea7444545666dfa4e8b91bb9e84`, GitHub Lu Desktop CI run `41766078286`, all 7 jobs successful including Windows Tauri compile; REPO-01 run `41766078348` and WIN-LOAD-01 `41766078264` succeeded. This certifies development CI, NOT installed Windows acceptance or cross-process atomic compare-and-swap.
- **WRITE-01E.4A:** added `inspect_rollback_candidate` as a read-only check of the exact original backup, matching published source bytes, target identity, supported text-format validation, and bounded complete before/after (no truncation). The desktop recovery folder picker can display the complete potential restore diff, but has **no rollback action or authority**.
- **Receipt hardening:** `verified_edit_receipt` independently decodes durable Protobuf ToolReply → EditExistingResult and binds applied state, edit tool ID, call ID, committed bit, byte count and both SHA-256 digests. Recovery, undo preview and edited-file folder links fail closed on an invalid or mismatched receipt; journal state alone is insufficient.
- **New tests:** original/current mismatch and backup tampering refuse undo preflight; no side effects on inspection; corrupted/missing/wrong-tool/wrong-type/wrong-call receipts refuse positive classification; UI checks the receipt-validation gate.
- **Acceptance still pending:** run full Linux/Windows CI on the *new* locked revision. No physical Windows retest yet. Native rollback API exists but is deliberately not exposed as a desktop action. NTFS reparse/hardlink, ACL/ADS/metadata preservation, noncooperating external editor races and power-failure semantics remain open v1.0 gates.


## 2026-11-03 — WRITE-01E.4B separately authorized restore (source-committed; CI pending)
- **Distinct consent:** Recovery inspection through the native containing-folder picker must first establish a verified original edit receipt, original-byte backup, unmodified edited target, and complete before/after review. This stages a *new* `RollbackReviewRecord` in `rollback-approvals.sqlite` and a one-shot pinned `RollbackReviewQueue`. Reinspection denies old pending reviews; restart interrupts all unconfirmed reviews.
- **Native host effect:** `lu_confirm_rollback(chatId, reviewId)` accepts no bytes or filesystem paths from the WebView. It rechecks the durable review against the original edit approval, obtains a **new native OS confirmation**, then grants a *different* authenticated persisted user turn/task and `lu.edit_existing.rollback` WriteWorkspace scope. `native_host_edit::execute_approved_rollback_pinned` rechecks the exact original edit receipt, complete source/backup snapshot, and dispatches through the native Protobuf registry and `EffectJournal`; the original edit consent is never reused.
- **Custody and recovery:** The rollback transaction records its own durable intent and preserves the *displaced edited bytes* in `.lu-edit-<rollback-id>.prior` before staging the original version. Both before- and after-restore versions remain independently recoverable. `observe_rollback` refuses a restored target with missing/corrupted displaced-version custody; `verified_rollback_receipt` checks the independent restore effect's typed Protobuf receipt and digest.
- **User experience:** Full original/current/change lines, explicit checkbox, separate native confirmation, Keep Current denial, durable rollback history and uncertainty indicators. No automatic re-execution after a failed/interrupted restore. The accepted `0.1.0-alpha.2` baseline and draft PR #5 remain untouched.
- **Tests added:** Rollback approval exactness, single use, restart invalidation; actual desktop native-auth restore, preserved edited bytes and original backup, receipt verification, wrong-chat/review refusal, external modification refusal; crash-point prior custody and corrupt recovery; frontend authority contract. Windows CI explicitly runs these new tests.
- **Acceptance remaining:** Full Linux/Windows CI on the exact new lock, desktop installed-app OS confirmation UX, Windows reparse/hardlink/ACL/ADS behavior, cross-process race closure, and power-loss semantics. Do not claim universal editor safety or v1.0 completion before these gates.

## WRITE-01E.4C.1 — Windows file identity + metadata refusal (source committed; CI pending)
- Original 4B fully passed exact GitHub CI at `f40e3206`: Lu Desktop CI `41771221728`, source freeze `41771221850`, WIN-LOAD `41771221721`.
- Added `lu-core/windows_file_guard.rs` with isolated Win32 GetFileInformationByHandleEx; inspects the **opened file handle**, validates 128-bit FileId/volume, refuses multi-hardlink targets, unsupported reparse/read-only/hidden/system/sparse/compressed/encrypted/offline flags and named/unknown alternate data streams (fail closed on unsupported FileStreamInfo).
- Both existing-text edit and separately authorized rollback now recheck the native file identity alongside SHA-256 at the final prepublication barrier; both journal manifest readers use the strict file admission. The same-byte different-FileId deterministic replacement test checks that content digests alone cannot substitute an unapproved object.
- Added Windows CI steps for the Win32 guard tests and native edit/restore adversarial tests. Accepted `0.1.0-alpha.2` baseline unchanged; draft PR #5 remains unmerged.
- **Not complete:** root-open identity during the user's original native picker/approval is not yet bound to the transaction; cross-process CAS between final check and rename is not atomic; DACL/ACL/ADS *preservation* (as opposed to ADS refusal), parent directory flush, power loss, Windows non-cooperating writers, junctions, and installed-app behavior require additional independent engineering and acceptance. Do NOT claim universal safety. Full CI status on current source lock pending.

- **WRITE-01E.4C FFI build gate:** The first Windows compilation refused unsafe blocks directly inside `lu-core` (correct; `#![forbid(unsafe_code)]`). Moved all Win32 FFI to existing audited Windows-only `lu-win-worker-job/src/file_guard.rs`, retaining an unsafe-free Lu Core re-export. Windows CI explicitly compiles/tests that FFI crate. This corrected source revision is pending exact-source-lock CI; do not bypass the Lu Core unsafe policy.

## WRITE-01E.4C.1 — Verified development CI checkpoint
- **Verified implementation commit:** `8eb7fa51d36eaef0cfb27222b40aa9c06edea3ed`.
- **GitHub workflow outcomes:** Lu Desktop CI `41773702637` SUCCESS (all jobs, including native Windows adversarial FileId/hardlink/ADS/refusal tests, previous file edit/rollback tests, and full Windows Tauri compile); Lu REPO-01 `41773702703` SUCCESS; Lu WIN-LOAD-01 `41773702696` SUCCESS.
- All native Win32 FFI is in the isolated `lu-win-worker-job` crate. Lu Core's `#![forbid(unsafe_code)]` was preserved. This is a **development CI gate acceptance**, not a claim that Windows filesystem races, ACL preservation, physical installed-app behavior or power failures are solved.
- **Next hardening work:** bind the native-picked file's exact identity across human approval into the transaction (not merely prepublication); then address noncooperating external-writer CAS/leases, NTFS DACL/ACL/timestamps and directory flush with deliberate independent Windows acceptance fixtures. Continue on draft PR #5, preserving the accepted alpha baseline.


## WRITE-01E.4C.2 — Native picker FileId bound to original edit authorization (pending CI)
- **Source continuity:** Windows `edit_proposal::choose_file` captures the native volume/file ID from the OS-picked *opened* file, verifies that the same object is opened via the host-pinned directory capability, and incorporates its 48-hex-character native ID token into a v2 domain-separated full-before/after `ReviewedEdit` SHA-256. Existing SQLite `edit_approval_ledger` persists this exact review hash, preserving the native identity commitment without model-originated permission data. Linux continues the existing unchanged v1 review fingerprint.
- **Trusted execution:** `native_host_edit` checks the selected file ID again before constructing a human user turn. Windows host calls `register_edit_tools_pinned_selected` with the exact selected basename and Win32 FileId, never a WebView path/ID. The first-party native tool refuses Windows registrations without a bound selected ID and refuses a different basename. After the durable journal's EXECUTING fence, `commit_in_dir_selected` verifies that same FileId before writing the intent and again just before publication. Name and byte hash alone no longer suffice even if an external actor swaps in another file with identical bytes.
- **Separately approved undo:** On native folder re-selection, rollback review checks the newly selected current file ID before and after the full restore preview. A one-use `SelectedRollback` retains that Win32 ID; host rechecks it before authorization and native `rollback_with_stop_selected` rechecks it before the rollback intent and before publish. Rollback still requires a separate native confirmation, trusted user turn, policy scope and journal effect.
- **Explicit tests:** Windows native-auth cases swap selected files for different file IDs containing identical bytes after review, and verify neither edit nor rollback creates a new journal effect/manifest. Windows core tests bypass desktop precheck and verify its native transaction-entry identity fence rejects same-byte substitutions before backup/intent. Windows registry integration fixtures now use native-picked identities, with a fresh selection before a later rollback. Cross-platform unit tests verify that two distinct native identity tokens produce different approval digests. CI runs the Windows tests explicitly.
- **Still open:** The selected FileId is committed inside the review SHA and retained in volatile host review state, but a separately queryable native ID column in persistent approval/rollback records is not yet implemented. A noncooperating editor can still write/rename during the final check-to-publish window; native Win32 compare-and-swap/exclusive ownership, ACL/ADS/metadata-safe replacement, directory flush and power-failure verification are not closed. Installed-app acceptance remains deferred. Draft PR #5 remains unmerged; accepted alpha.2 baseline unchanged. This revision is **not accepted until its exact CI passes**.


## WRITE-01E.4C.2 — Exact-revision development CI ACCEPTED
- **Tested code revision:** `2b664fd2b95b6daa7a2d53788006b3e722148afe`.
- **Successful workflows:** Lu Desktop CI run `41777297445`, REPO-01 exact source run `41777297387`, and WIN-LOAD-01 run `41777297414`, ALL SUCCESS.
- Desktop CI verified full Linux Lu Core and portable user-turn-to-native authorization, original file-edit and independent rollback, the Windows native picker-to-effect *same-byte/different-file-ID* adversarial cases, direct native transaction-entry checks, and the full Windows Tauri compile. Two CI mistakes were corrected before acceptance: a Linux `Option<String>` inference error and an older Windows integration fixture that lacked the newly mandatory native identity proof.
- **Boundary:** This closes selected-object ID continuity, not universal file-edit safety. The review SHA commits to the selected Windows FileId, but a separate explicit FileId field in the persistent approval record is future audit work. Noncooperating external-editor compare-and-replace atomicity, NTFS DACL/ACL/timestamp/ADS preservation, native parent-directory flush, real power-loss resilience, and installed-app Windows acceptance remain OPEN. `0.1.0-alpha.2` stays frozen and PR #5 remains a draft.
- **Next:** WRITE-01E.4C.3 should tackle metadata-conserving Windows publication and external-writer race limits with independently verified DACL/custom ACL/ADS/time fixtures, without attempting silent metadata loss or weakening the native file selection trust boundary.


## WRITE-01E.4C.3A — Conservative Windows security/time custody and write-exclusion (source committed; CI pending)
- Win32 FFI remains in isolated `src-tauri/lu-win-worker-job/src/metadata_guard.rs` with a safe Lu Core re-export; `#![forbid(unsafe_code)]` in Lu Core is preserved.
- **Before publish:** `GetSecurityInfo` snapshots owner/group/DACL from the capability-opened target. A newly synced stage must possess a byte-for-byte equivalent security descriptor, or the transaction refuses publication. `GetFileTime`/`SetFileTime` preserve creation and last-access timestamps on the stage, while last-write intentionally changes. All APIs are checked, and unsupported/permission-denied metadata fails closed. Lu does not bypass ACLs, copy ACLs by privileged operation or arbitrarily reset security. Custom explicit ACLs not inherited identically are refused instead of silently lost. The original ACL/creation data are rechecked before and after the capability-relative rename.
- **External writers:** Native `ReOpenFile` on the already capability-opened target takes a path-free handle with `FILE_SHARE_READ|FILE_SHARE_DELETE`, excluding `FILE_SHARE_WRITE`, across the final hash+FileId+metadata checks, rename, and postpublish verification. Existing writers cause refusal; new writable opens conflict with the lease. `FILE_SHARE_DELETE` is necessary to allow Lu's own rename, and hostile competing rename/delete operations remain possible. This is not a Windows atomic compare-and-swap.
- **Recovery:** The original-byte backup and staging artifacts remain when metadata or writer checks fail after the journal EXECUTING fence; the outcome is UNCERTAIN and must not be retried automatically. Source and replacement checks remain SHA-256 and native FileId-bound to the user's exact review, with separate rollback approval.
- **Tests:** Dedicated Windows native FFI tests for security/time custody and writer handle sharing, Windows core edit+undo creation-time retention, refusal of explicit custom ACL inherited-security mismatch, and refusal when another writer holds a writable file handle. Windows CI runs the FFI module and existing `windows_` adversarial Rust tests.
- **Open v1.0 gates:** SACL/audit inheritance, arbitrary EAs/object IDs, filesystems with different timestamp semantics, true name-bound cross-process atomic CAS, native Windows parent-directory flush, real power-loss durability and installed-app acceptance. The final CI result on the frozen revision is still pending. Stable `0.1.0-alpha.2` is unchanged; draft PR #5 remains unmerged.


## WRITE-01E.4C.3B — Windows metadata/identity evidence across interrupted publication (source; CI pending)
- Prior milestone **WRITE-01E.4C.3A** fully passed exact revision `8ed8029747c7840bda3ac1f88094b0a67ec1d6e1`: Lu Desktop CI `41780064278`, REPO-01 `41780064175`, WIN-LOAD `41780064519` all SUCCESS, including the full Windows Tauri compile.
- **Durable security witness:** new Windows edit and rollback manifests contain SHA-256 of the opened target's owner/group/DACL self-relative descriptor and creation time; the ACL bytes themselves are *not* stored. Witnesses exclude last-access (read-mutated) and last-write (deliberately changes). Read-only `observe_edit`, `observe_rollback` and `inspect_rollback_candidate` reject matching-content files if security evidence differs. Legacy Windows manifests without a native witness are classified unverified; they require manual custody review, not silent restoration.
- **Durable stage identity witness:** before any Windows target-name rename, Lu syncs a separate `.lu-edit-<effect-id>.seal` record containing exact target, staged content SHA, stage native FileId/volume token, and staged security witness. After publication, positive receipts require that same stage FileId under the target name and a matching seal. Recovery rechecks the seal to refuse same-byte file substitutions even if their creation time, owner/group, DACL, and text are identical. The seal is an evidence artifact; it is **not a cryptographic MAC or protection against a privileged adversary who can rewrite all files and journal evidence**.
- **Windows tests:** deterministic competing rename *after* Lu publishes the approved data with a different FileId but the *same bytes and security/times*; explicit restore counterpart; altered seal; changed ACL with unchanged contents; legacy manifest without metadata witness. Existing journal crash-point fixtures continue to cover original/backup/stage and postpublication observations. Native edit and independently approved rollback retain their per-turn grant, file ID, exact hashes, custody, and no-automatic-retry rules.
- **Open gates:** even a final FileId check after rename is detection, not a compare-and-swap with the prior target name. Between final prepublish check and replacement, an adversarial noncooperating rename can still cause loss of someone else's replaced path; only a fully proven name-bound primitive or a limitation/refusal policy can close that. Standard-user Windows parent-directory sync and power-cut durability remain undocumented/unproven; object IDs, EAs and SACLs are not yet preserved/refused with complete coverage; actual installed app not tested. Do not equate a signed SQLite receipt or matching data with physical persistence.
- **Release scope:** This is a branch implementation, not `0.1.0-alpha.2` and not v1.0 acceptance. Draft PR #5 remains unmerged. Current exact-source CI is pending.

## 2026-11-03 priority pivot: native tools, not more file-edit edge cases

- WRITE-01E source checkpoint `2a9a1355` on `lu/write-01e-approved-edit-gate-20261103` passed Desktop, REPO-01 and WIN-LOAD GitHub CI for the tested source scope. **Freeze further WRITE-01E race/power-loss optimization** for now. Its known NTFS atomic namespace, metadata and release-acceptance limitations remain open and are not waived; keep the draft separate.
- Owner clarified that **TOOLS-02 is internal JSON validation at Lu\'s model/tool boundary**, not a user-facing JSON file inspection tool. Code checks untrusted JSON envelopes and direct codec arguments on entry, including duplicate keys, syntax and limits. Host results must pass an exact success/error feedback schema and strict serialization before model continuation. There is no UI, OS picker, file inspection, editing, or separate tool grant. See `TOOLS_02_INTERNAL_JSON_BOUNDARY_20261103.md`; exact-source CI remains required.
- Next non-write tools: Git read-only inspect/status, archive listing and bounded compression utilities, structured source/schema validation and program discovery. Continue through existing registry + per-turn `ConversationAuthority`; no model-created grants.
- **DLC redesign held for v2.0**. Native Windows and Linux both use AF_UNIX, never Windows named pipes; NVP requests cooperative Lu-owned model unload/reload for RAM/VRAM allocation, Cognitum remains optional reviewed reasoning/calculation provider. See `LU_V2_0_DLC_AF_UNIX_RESOURCE_REASONING_DEFERRED_20261103.md` and Lu issues #8/#9. Design only; no DLC implementation in the current v1.0 branch.

## TOOLS-02 owner correction — internal JSON boundary (2026-11-03)

- All temporary **user-facing JSON inspector, native picker, desktop composer button, file Protobuf messages and associated tests were removed**. They were based on a mistaken interpretation and do not belong in Lu v1.0.
- The existing `model_json_boundary` now serves as the internal bidirectional gate for **structured protocol JSON**. Direct model tool arguments cannot bypass nested duplicate-key refusal, and outbound host-to-model tool feedback is validated against an exact typed envelope before model continuation.
- Native registry, capability grants, model authorization and Protobuf tool codecs remain authoritative. JSON repair, where already permitted, only strips known fence/trailing punctuation and cannot create arguments, tool names or permissions.
- Existing alpha and WRITE-01E baseline preserved. Review `TOOLS_02_INTERNAL_JSON_BOUNDARY_20261103.md`; exact-commit Linux/Windows CI is required before accepting the source change.

## TOOLS-02 original MBRL completion scope (2026-11-04)

- Owner directed full original `Jan-Mod-Design-Document.md` §§4.6–4.8 behavior, superseding prior broad "advanced JSON repair deferred" wording. No semantics-changing repairs, hallucinated fields or user-facing JSON inspector.
- Core generation loop now has two bounded pre-dispatch recovery attempts, typed `lu_boundary` refusal feedback for parsed tool-shaped mistakes, and BLOCKED terminal classification; external llama SSE and desktop user-approved file proposal parsers reject malformed/ambiguous JSON. Verify actual native Linux/Windows CI and desktop compiler/tests before completing this row.
- Separate prerequisite for complete desktop native tool operation: schema-advertised `NativeToolTurn` must be wired to the trusted GUI with current persisted user-turn authority; MBRL code alone does not grant tool powers or establish installed v1.0 acceptance. Track in issue #13.

## 2026-11-04 — Jan-first original tool design merged (owner directive)

- Original Library `Jan-Mod-Design-Document.md` preserved **unaltered** in `docs/reference/Jan-Mod-Design-Document.md` on the tools branch. Its original toolkit/workspace requirements are now the **primary tool architecture**, subject only to explicitly changed owner decisions.
- `docs/LU_NATIVE_TOOLS_MERGED_DESIGN.md` is the binding consolidated **tool subsystem** entrypoint: original §3.9 Code workspace placement; §4.6–4.9 JSON boundary/Protobuf/recovery/effect journal; §5.2 unified internal domains vs narrow model schemas; §7 loop guard; plus confirmed Lu owner-specific authorization, independent runtime and v2 DLC decisions. Central design links both records.
- **TOOLS-03** is a unified native Git inspection *domain* in Lu Core, not an automatically advertised Git/IDE agent. Model-facing tools are narrow, host-approved repository/code-task only. Source proof fixture and OS CI job added on `lu/tools-03-git-inspection-20261104`; source lock/CI/hostile repository failure injection and installed GUI acceptance must be independently checked. No Git write operation or general shell grant was added.
- **Every next native tool requires a documented original section mapping before implementation.** A later assistant-generated tool idea may not supersede original intent. Source-only tests cannot be marked v1.0 release-complete.

## E1 reviewed application admission — November 7, 2026

- **E1-P01:** Added host-reviewed executable preflight and genuine ELF/PE SHA-256 admission, source branch `lu/e1-reviewed-app-admission-20261107`, PR #103. Initial native Linux/Windows CI **FAILED** due to `wrong.app_version="../replace"` incorrectly assigning `&str` to `String` in a test; also found Unix symlink test accidentally nested inside another test. Both are repaired in the next E1-P02 submission. Never claim earlier failed CI succeeded.
- **E1-P02:** Host-only bounded reviewed application catalog; per-current-user-turn permission and host policy, exact read-only tool and workspace, on-demand actual executable SHA-256 + opened OS identity, fail-closed removal/duplicate/version mismatch, no registered default apps, no PATH scanning or app process launch. Linux/Windows native source tests and isolated E1 CI submitted but **new exact-head results pending**.
- **Version claim limit:** reviewed release label bound to a checked SHA-256, NOT a live application version response or publisher signature. A contained and approved runtime version probe, when needed, belongs to E1-P03.
- **Owner release schedule:** defer real hardware/interactive Windows, WebView, two-GGUF AMD Vulkan and NSIS install/uninstall acceptance to near Lu v1.0. Continue hosted Windows/Linux native tests now.


## E1-P03 — reviewed application native execution (November 7, 2026)

- **New isolated workstream** \`lu/e1-p03-contained-app-worker-20261107\`, stacked on E1-P02 draft PR #103. Real physical Windows/installer/Vulkan testing remains deferred near Lu v1.0 by owner instruction; native GitHub Windows/Linux actual process tests are **not** deferred.
- Existing E1-P02 read-only app discovery is a separate authority. The new E1-P03 broker requires a fresh/current \`ExecuteCode\` grant tied to the exact persisted user turn, task, tool and workspace; model text never supplies binary/args/process ID.
- The only admitted initial operation is Lu’s audited first-party \`--lu-e1-version-one-shot\` worker with host-reviewed binary SHA and file identity, 15-second fixed deadline, empty child environment, null stdin, bounded progress protocol, existing Windows Job Object and Linux process group + parent-death protection. 256 MiB OS address-space/job cap for this fixed mode. This is **not** a generic shell or third-party application-launch authority.
- The execution journal persists \`PREPARED\`/\`EXECUTING\` **before spawn**; owner cancellation and task revocation kill/reap the actual process tree and record \`UNCERTAIN\`. Duplicate and prior uncertain IDs do not restart. Only matched process-exit/host-known marker receipts can mark the process action APPLIED, with application output correctness explicitly NOT verified.
- Real native evidence workflow builds two genuine fast/slow Rust OS worker executables, performs runtime launch, completes validated protocol, revokes live workers, checks original conversation cancellation scope, bad binary/hash and host veto, and simulates exclusive restart UNCERTAIN reconciliation. These process-dependent Rust tests are explicitly run via \`cargo test -- --ignored\` in dedicated Windows/Linux CI; ordinary tests do not silently count them.
- **Status:** source implementation submitted, full P03 CI/native execution and system isolation acceptance pending; general reviewed third-party adapters and semantic outcome verification remain later gates.


## E1-P03 exact-head regression closeout candidate — November 7, 2026

- Dedicated E1-P03 Windows/Linux CI on \`149d888e\` was observed queued, with no actual OS worker result; do not confuse a queued job with acceptance.
- Source inspection found broker revocation discarded process custody before verifying kill/reap, and a late worker result could conflict with cancellation. **Fix:** \`revoke_pending\` state, durable UNCERTAIN before cancellation, retained custody on failed kill, and no APPLIED publication after revoked/cancelled outcomes.
- Added real fast-worker race test (OS process already reaped when revocation arrives), and 22-second bounded timeout fixture to prove the supervisor's 15-second hard deadline and non-replayable UNCERTAIN journal, on **both hosted Windows/Linux**.
- E1-P03 full native acceptance **OPEN** until the new exact-head compiler, source identity, process-kill, timeout and restart journal results are complete; physical installed Windows stays deferred to near v1.0.


## E1-P04 output oracles — November 7, 2026

- **Source candidate** \`lu/e1-p04-independent-outcome-oracles-20261107\` stacked on E1-P03 draft PR #105. Routine real-world installed Windows/NSIS/Vulkan acceptance remains deferred until near the Lu v1.0 release candidate; native CI tests are required now.
- Independent host-only JSON structured-outcome oracle reads **actual host-granted opened files**, checks OS handle identity and full SHA-256, strict typed schema, approved ordered rows and revision; rejects empty/large/changed/spoofed/unknown/duplicate-field artifacts. A model's "succeeded" or file existence is not evidence.
- Current original-user-turn \`ReadWorkspace\` authority, independent host policy, and matching real P03 \`EffectJournal\` APPLIED + TaskStore COMPLETED + exact scanner protocol receipt are required **before** validation; \`with_authorized_worker_effect\` fences its bounded read-and-persist operation against concurrent revocation.
- Atomic separate-SQLite verified-output receipts preserve immutable same-call artifact digest/OS file identity across restart; changed-bytes replacement is denied. The oracle reports \`exact_semantics_verified=true\` for the specific file **only**, while \`artifact_producer_independently_attributed=false\` and \`external_application_output_verified=false\` remain explicit.
- Genuine Linux/Windows test workflow submitted for output bytes, row/cell-like semantics, revocation, malformed process receipt, duplicates, SQLite restart and artifact tampering. Tests use a **fixture P03 journal receipt**, not a real app-producing-artifact test. Full P03 real-worker acceptance remains independently queued, and E1-P04 producer attribution and broader document/spreadsheet/film output oracles remain **OPEN**.
- **Next:** E1-P04 real worker-to-artifact custody, additional application-format semantic validators; then E1-P05 measured model/utilities/approved-app comparison. Do not mark E1-P04 production-accepted from source alone.


## E1-P04B real worker artifact custody (November 7, 2026)

- Dedicated stacked P04B source implements *actual* bounded stdout binary custody for reviewed DOCX/XLSX workers, with 256 MiB Windows Job/Linux address-space limit, native OS child owner and verified P/D/A stream. No worker-chosen filesystem destination.
- Separate current-user-turn ExecuteCode + WriteWorkspace host grants, independent Office document text / worksheet cell semantic oracle, cap-std trusted directory create-new-only atomic publication, file re-read byte witness and separate SQLite process and publication receipts. Failures keep the journal UNCERTAIN; no replay.
- Exact-head hosted Windows/Linux CI must build the real app producer, a slow cancellation fixture, and a forged-but-hash-valid OOXML fixture; tests run explicitly under --ignored so they cannot be silently skipped. Producer attribution refers **only to the reviewed first-party worker's owned pipe**, not a general external app.
- E1-P04 generic third-party operation and physical installed Windows remain OPEN; proceed to E1-P05 with genuine task baselines and conservative missing-result reporting.


## E1-P05 measured three-arm benchmark candidate — November 7, 2026

- Source branch \`lu/e1-p05-three-arm-benchmark-20261107\` stacked on E1-P04B. Two fixed actual OOXML tasks: reviewed DOCX with the exact approved paragraph and XLSX with four expected cells. Test \`direct_model\` (real SHA-pinned local GGUF text), \`native_utility\` (existing Rust spreadsheet creator; DOCX creation unsupported), \`reviewed_app\` (actual supervised first-party worker and verified OS-pipe stdout publication).
- Independent measurements: real native artifacts, hashes and ZIP structure checked from CPython; exact semantic content checked in Lu Core, effects journaled, original user grants and host veto checks, p50/p95 latency, raw model answers and model GGUF/engine/runtime/source identities.
- The model is **not** attributed a document artifact from prose. Without a genuine model engine, direct-model status is \`NOT_EXECUTED\` rather than invented; native-only and model-inclusive runs remain distinct. Record explicit unsupported utility DOCX and reviewed worker's first-party nature to avoid spurious rankings.
- GitHub CI native Linux/Windows plus local pinned Qwen three-arm job required at **final exact head**; physical installed-Windows/NSIS/Vulkan testing remains deferred near Lu v1.0; third-party app sandbox and dynamic tool-routing acceptance remain open.

## E1 implementation handoff — November 7, 2026, independent evidence pass

- E1-P03: real fixed first-party process and revocation/reap source candidate, PR #105; its dedicated real Linux/Windows child-process tests `--ignored` in normal core suites and must PASS in dedicated matrix to close this gate.
- E1-P04: actual open-handle identity, bounded typed output validation, authorized original-turn journal receipt, PR #106; separately distinguish verified observed structure from attributable application production.
- E1-P04B: fixed reviewed worker stdout artifact custody, actual DOCX/XLSX native parser and create-new publication with distinct receipts, PR #107; real OS worker and cancellation matrix must pass to close.
- E1-P05: native spreadsheet utility vs reviewed application vs actual pinned local Qwen benchmark, PR #109. The no-tool model cannot produce a file; utility DOCX is unsupported. No invented results or unfair generalizations. Requires physical CI report bytes and exact runner/model SHA evidence.
- E1-P05B: cross-language Python DOCX/XLSX content witness, hostile OOXML ZIP/relationship/content tests, explicit host-policy veto and journal-coverage assertions, stacked draft PR #110. Four locally executed Python structural fixture tests passed in the development environment; this is NOT a run of actual OS workers or Qwen. Hosted exact-head and source-lock CI still required.
- **Independent deferred gate:** owner-directed physical Windows installed Lu.exe/WebView, NSIS and GPU/Vulkan testing near Lu v1.0 release candidate. Do not equate hosted CI with installed-app acceptance. Further third-party application adapters must be separately scoped; fixed first-party fixtures never authorize generic shell/network/process execution.

## E1-P05C — native Word/XLSX benchmark parity repair (November 7, 2026)

Static source review exposed an error: `native_docx_create::render_docx` and `lu.docx.create` were already present in Lu Core. Original P05 v1 incorrectly marked native DOCX as unsupported, biasing app-vs-utility results. E1-P05C source now renders real DOCX and XLSX for the exact same native tasks, under WriteWorkspace and journal control, with actual independent Office readback and one host-policy denial attempt per task. Evidence schema is v2, not comparable to v1. Real Windows/Linux worker and pinned Qwen receipts remain independently required; physical installed Windows remains deferred near v1.0.

## E1-P06A — typed reviewed third-party adapters (November 7, 2026)

A host-only fixed version-operation adapter registry now checks explicit original-turn ExecuteCode authority, independent host veto, exact native PE/ELF SHA and file identity, sealed typed payload, bounded output/runtime declarations and read-grant separation. The adapter cannot spawn a process or claim independent application output/network isolation. Three genuine local-binary fixture tests submitted in Linux/Windows native workflow; **hosted CI still pending**. Real external process execution, version parsing, durable journal, OS sandbox and cancellation belong to P06B/P06C. Preserve E1-P03/P04B/P05C unresolved acceptance and deferred physical installed Windows gate.

## E1-P06B — genuine reviewed external process (November 7, 2026)

Implemented a host-only reviewed Git --version launch through the existing Windows Job/Linux process-group supervisor, with current original-user-turn ExecuteCode + policy, exact reviewed file hash and OS identity before spawn, 1–15s deadline, 1–4096 byte host-only output, strict single-line Git/Python parser, SQLite journal receipts after exit/reap, and fail-closed UNCERTAIN on revoked, cancelled, oversized, malformed, failed or timeout results. Six dedicated ignored-by-default native tests exercise real installed Git and test-only real Python cancellation/revocation/timeout/oversize, plus parser and stdout fixtures. Linux/Windows native results **pending**; source-only changes must not close runtime gates. Broader third-party sandbox, network isolation and typed media operations E1-P06C remain separate, installed physical Windows near v1.0 remains deferred.

## E1-P06C — typed useful external Git operations; preserve native Lu access (Nov 7, 2026)

The reviewed external Git adapter now offers fixed HEAD commit and tracked-status reads against host-approved local repositories; output is bound to 16 KiB and 30 seconds, strictly parsed after supervised OS exit/reap, and journaled as normalized results only. Separate authenticated current-turn ReadWorkspace and ExecuteCode grants, host policy, reviewed executable digest, host-owned path and immutable operation bytes are required. Native tests validate real Git clean/dirty status and unchanged index, correct HEAD, access denial, wrong task origin/scope, injection, invalid/symlink repo and restart persistence. Lu itself stays an unrestricted native host: Access ≠ Permission ≠ Authority. External worker process supervision is not mandatory sandboxing of Lu. P06C native CI remains open, as do P06B actual OS cancellation and E1-P03/P04B/P05C evidence gates.

**P06C hardening addendum:** fixed Git argv now includes an explicit core.fsmonitor=false override to suppress configured external fsmonitor helpers, and rejects . and .. path components. Windows host-approved paths retain native command-compatible spelling after filesystem and symlink validation (not an arbitrary model path); no mandatory Lu sandbox introduced. Linux/Windows native acceptance remains open until complete.

## E1-P06D — additional external Git context (Nov 7, 2026)

Added fixed read-only current-branch and repository history-count operations to the E1-P06C reviewed external Git adapter, with new sealed typed request bytes, strict bounded parsers, deliberate null for detached HEAD and normalized durable receipts. The supervisor's exact argv allowlist now admits those two specific commands, not arbitrary CLI arguments. Native OS ignored tests cover genuine Git branch/history and detached-head behavior on Windows/Linux, alongside existing P06C read grants, host veto, status and HEAD tests. Independent Linux Git CLI behavior was exercised, but does **not** prove Lu/Rust worker acceptance. E1-P06D hosted CI, older E1-P06C/P06B tests and installed Windows remain open. Production Lu native host is not sandboxed; per-turn authority and child process ownership remain separate. See docs/LU_E1_P06D_GIT_CONTEXT_20261107.md.

## E1-NATIVE-EVIDENCE-01 — exact-source acceptance and measurement repairs (Nov 7, 2026)

Audited P03/P04B/P05C/P06B/P06C/P06D dedicated GitHub Actions. They were all awaiting the initial source job; there were no real OS-native worker results or benchmark artifact to close. Independent audit found the default PR checkout/GITHUB_SHA references a synthetic merge commit rather than the frozen PR head, and P05's previous p95 indexing understated two-trial tail latency. Submitted exact-head checkout assertions, branch-level concurrency, strict ignored-test minimum counts (P03 6, P04B 5, P06B 6, P06C 3, P06D 5), P05 empirical nearest-rank p95, and an independent raw results source/row/witness/summary validator with synthetic contract fixtures. All native Windows/Linux and genuine Qwen evidence gates remain OPEN until actual hosted test logs and evidence artifacts exist; this tooling repair does not close them. Details: docs/LU_E1_NATIVE_EVIDENCE_20261107.md.

## E1-P06E — reviewed native FFprobe WAV/MP4 facts (Nov 7, 2026)

Implemented a new host-only, typed FFprobe broker with reviewed executable SHA, host-selected approved WAV/MP4 input path and digest, fixed local-file-only demuxer command, independent original-user-turn ReadWorkspace+ExecuteCode grants, strict bounded JSON parser, post-reap normalized SQLite receipts and fail-closed cancellation/revocation/recovery. Five genuine native Windows/Linux tests are configured (real WAV/MP4, authority denial/injection, invalid media and changed file) plus parser and file hash fixtures; native acceptance OPEN pending real OS CI. Lu Core remains a native unsandboxed host; this scoped worker is monitored for reliability, not a mandatory sandbox on Lu. Desktop/model tool exposure is not yet connected. See docs/LU_E1_P06E_MEDIA_ADAPTER_20261107.md.

## E1-P07 — model/desktop reviewed Git and FFprobe tools (Nov 7, 2026)

Source implementation adds five argument-free reviewed app model tools (Git HEAD/status/branch/history and WAV/MP4 facts) to the real Lu ModelToolCatalog, NativeToolRegistry, NativeToolTurn and supervised inference loop. A new one-turn desktop Tauri/native picker and two React Send opt-ins bind original saved human messages to ReadWorkspace and ExecuteCode grants, without permitting a model-chosen program, path or argument. Independent inner OS-worker and outer conversation effect IDs prevent durable journal collision; a cancellation monitor revokes the task and kills owned processes. Expanded native tests now check five independently parsed real OS Git/FFprobe results and ten persisted outer/inner effects, plus a second scripted-model production conversation-session test. A new SHA-pinned Qwen3-0.6B GGUF + llama.cpp Ubuntu acceptance workflow must prove the model *itself* selected Git and FFprobe without host routing and that the live tool feedback/final answers and receipts survive SQLite reopen. Source complete for the candidate; all exact-head CI checks and installed-Windows UI acceptance OPEN. No blanket sandbox on Lu Core and no required MCP or Docker. See docs/LU_E1_P07_REVIEWED_APPLICATION_CHAT_20261107.md.

## E1-P08 — reviewed native-app crash/restart journal reconciliation (Nov 7, 2026)

After E1-P07 but without closing its pending CI: implemented new
`reviewed_app_recovery.rs` and integrated it at desktop startup under the
pre-existing exclusive `DesktopOwner` writer lease. It scans strictly
host-minted and bounded per-turn Git/FFprobe SQLite journals (no user/model
paths), rejects malformed/linked/reparse files, converts stale RUNNING
tasks to INTERRUPTED and EXECUTING effects to UNCERTAIN, preserves APPLIED
receipts, and does not replay an operation. One-sided journals from a
crash between file creations are handled without making up the missing
database. Counts-only operator diagnostics are emitted when recovery
is needed. Added a Tauri-free production-source harness testing genuine
SQLite restart, preservation, idempotency, one-sided creation failure,
malformed filenames and Unix symlinks. Dedicated exact-head Ubuntu 24.04,
Windows 2025 and Windows desktop Tauri workflow added. **Source candidate,
native/installed acceptance OPEN**. This is effect/ledger recovery, NOT
proof that a Linux child survived or was reaped after parent death.
The original authenticated ANSF user turn now receives a persisted
interrupted assistant history marker, matched and deduplicated by turn ID
using indexed SQLite event state. Completed responses are never rewritten;
queued Send markers cover a crash before worker journals exist. Normal
reviewed errors also become visible unsuccessful history checkpoints.
Linux-only source adds a first-party lifetime guardian that anchors the
reviewed worker's private PGID and SIGKILLs its own group on Lu pipe EOF,
including parent SIGKILL. Dedicated real-host-crash, real-grandchild
and unrelated-process survival tests are submitted; cgroup/setsid
escape containment and adopted-child reaping are NOT proven.
The debug-only crash fixture is gated by an opt-in Cargo feature and
the future Linux single installer still needs the guardian sibling binary.
**Exact-head Windows/Linux CI and kernel crash proof are OPEN.**
The physical installed-Windows acceptance remains deferred toward v1.0.
See docs/LU_E1_P08_REVIEWED_RESTART_RECOVERY_20261107.md and
docs/LU_E1_P08_LINUX_GROUP_GUARD_20261108.md.

## E1-P08B/C — exact native repair and escape-capable worker decision (Nov 8, 2026)

Earlier E1-P08 hosted native receipts at run 42454451010: Windows
portable 3/3 PASSED; Ubuntu 24.04 portable 4/4 PASSED; Windows Tauri
compile FAILED with Rust E0369 in the previously committed reviewed
desktop mode test. Replaced the invalid `matches!("git"|"media",...)`
expression with independently valid literal match tests. Exact-current-head
Windows Tauri proof remains OPEN, as does the new guardian test matrix.

Linux kernel negative control reproduced `setsid()` child survival
after original-group SIGKILL, despite remaining in the same cgroup;
the child was cleaned up via a kernel-pinned pidfd. Submitted the
negative control to dedicated native CI. **Decision: cgroup-v2 delegated
subtree + cgroup.kill is necessary before Lu can promise complete
termination of descendants that escape a process group.** Current
group guardian remains lower-assurance and explicitly scoped to
ordinary cooperative Git/FFprobe descendants. True delegated cgroup
custody and abnormal-host-crash cleanup are independent E1-P08C OPEN
gates, not a mandatory sandbox on Lu Core. See
docs/LU_E1_P08_CGROUP_V2_ESCAPE_DECISION_20261108.md.
