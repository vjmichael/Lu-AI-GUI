# Lu v1.0 tracked task list

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
