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
| DATA-01 | P0 | ANSF/SQLite conversations, immutable revisions, migration and backup | TODO | Crash/restart/migration tests |
| AUTH-01 | P0 | Authenticated user-turn tool scopes and revocation | TODO | Adversarial authorization tests |
| CONTAIN-01 | P0 | OS-enforced native worker isolation and cleanup | TODO | Windows Job Object/token limits |
| TOOLS-01 | P0 | First-party native read, search, atomic file writes, git, archives and JSON | TODO | End-to-end authorized Protobuf calls |
| JOURNAL-01 | P0 | Durable effect journal, idempotent replay and cancellation | TODO | Kill/restart/uncertain-effect reconciliation |
| MEMORY-01 | P0 | Working/object memory, provenance and belief revision | TODO | Editable and independently checkable retrieval |
| AGENT-01 | P1 | Durable agent orchestration, loop guard, progress cards | TODO | Multi-step failure recovery |
| UX-01 | P1 | Chat, diagnostics, accessible UI and streaming hardening | IN PROGRESS | UI automated tests |
| RELEASE-01 | P0 | Signed NSIS installer, upgrade and independent v1.0 E2E | DEFERRED | Near-v1.0 Win10/11 acceptance |

## Change tracking convention
Each change records task ID, commit SHA, tests (command/platform/results), CI run URL, failures, and deferred manual scenario. `DONE` means evidence exists for the stated scope only. Never treat mock/source-string tests as physical Windows evidence.

## Execution order
1. REPO-01 and CI-01 (converge exact source; CI must identify build).
2. WIN-LOAD-01, model/GUI observability, DATA-01.
3. AUTH-01 and CONTAIN-01 before privileged TOOLS-01.
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

**REPO-01 stays BLOCKED / incomplete.** Local source archive contains 251 entries and is identified by SHA-256 `51cce0ad677ce8df7845d778e039a60b1d9f9847003b44078e8b3b81f6ee642a`. A 249+ file source manifest was generated locally. Current tool environment cannot connect Git directly, and the GitHub connector offers text-file operations without a bulk archive upload. The source archive was **not** committed to the repository; do not interpret CI results as coverage of it.

**WIN-LOAD-01:** Local patch adds a bounded native watchdog to record `DX-M-WAIT` every 10s during a long load, with a stop signal before the ready/failure event. This is observability, **not** a fix for Windows Not Responding and **not** actual load progress.

**AUTH-01:** Local ChatStore now implements Lu Core's `TrustedUserTurnVerifier` using an exact host-generated generation ID from a real persisted User journal record. Invalid IDs, wrong conversation and duplicate IDs are rejected. Wiring into privileged tool admission is still TODO.

**DATA-01:** Added a host-only ANSF mirror adapter that append-only stores the exact verified User source and compares immutable existing source on replay. Live desktop integration is intentionally disabled until Rust compilation, migration and failure injection pass.

**Verification:** local Node test suite 87/87 passed. Rust compilation and native ANSF/tool tests remain **unverified** in this session (Rust toolchain unavailable); GitHub CI has not yet tested these patches. Physical Windows tests remain intentionally deferred to near v1.0.

**Next action:** synchronize and review every file of the exact source archive via an authenticated repository worktree, verify its manifest/hash, then run all GitHub Actions jobs against that new commit before accepting the new native foundation.
