# AGENT-01H — Read-only next-step preparation and recovery review

**Owner direction:** Continue Lu execution-layer development while the current-source AGENT-01G Windows CPU NSIS test installer builds. **State:** implementation candidate only; exact-head native Rust/React/Windows acceptance and genuine GGUF/worker evidence are still pending. **Branch:** `lu/kiaf-execution-agent01f-20261108` stacked on UX-CHAT02, PR #159.

## What is implemented

`lu-desktop/src-tauri/src/agent_plan_progress.rs` now derives an `AgentContinuationPreview` from the actual authenticated original-human conversation's read-only plan history. The existing `read_chat_progress` uses only the host-selected bounded SQLite plan file, authenticates the saved originating USER turn and verifies the expected schema. The new projection uses strictly bounded existing step indices and persisted scheduler status, not model-supplied approval. It reports:

- **Manual one-step review only:** first `INDEPENDENT_CHECK` / `INDEPENDENT_WITNESS` without a prerequisite, only while original plan review is approved and no schedule owner/uncertain history exists. It does not issue a native tool invocation or claim a session; the existing AGENT-01G native picker, current ANSF goal verifier, genuine worker and independent oracle are still mandatory.
- **Released or still-owned supervisor:** `STOP_RELEASED_OWNER`, `STOP_OWNER_CUSTODY_UNCERTAIN`, or `STOP_RECOVERY_REQUIRED`. A previously verified step followed by `RELEASED` cannot automatically become a new owner just because more steps remain.
- **Missing or uncertain historical evidence:** `STOP_UNCERTAIN_STEP`, `STOP_OUTSTANDING_STEP`, `STOP_MISSING_WITNESS`, `STOP_DEPENDENCY_UNVERIFIED`. A linked historical witness is **not** live current source proof.
- **Dependency review candidate:** `PREVIEW_DEPENDENCY_RECHECK` identifies the next step's index only for **human review**, requiring fresh independent verification of the actual earlier witness and a new original human turn. No actionable execution token/owner is returned.
- **Unsupported or changed plan:** `STOP_UNSUPPORTED_STEP_TYPE`, `STOP_STALE_REVISION`, `STOP_SOURCE_RECHECK`, `STOP_PLAN_REVIEW`, `STOP_INVALID_DEPENDENCY`, `STOP_NO_STEPS`. Never replace an unsupported plan task with a generic shell/tool or an inappropriate mini-app.

Every preview sets `liveSourceReverified=false` and `executionAuthority=NOT_AUTHORIZED_FOR_EXECUTION`. The new React UI renders the mandatory reason without a grant or action button. The existing **Review and run one step** button is narrowed to the single `MANUAL_ONE_STEP_REVIEW_ONLY` candidate, not any outstanding dependency chain.

## Tests and evidence limits

- Four added production `agent_plan_progress::tests` using real `ChatStore` and the bounded SQLite history: initially eligible approved manual step, released owner + restart with historical first witness, linked receipt that still cannot overcome `RECOVERY_REQUIRED`, and cached dependency/unsupported step refusal.
- Six deterministic Node source-contract cases verify production backend/frontend wiring, no automatic worker dispatch, first-step-only UI eligibility, dependency blockers and genuine native Rust test presence.
- `.github/workflows/lu-agent01e-broker-progress.yml` now runs those Node cases and requires the full native progress-test suite (8 Linux, 7 Windows, where the existing symlink regression is Unix-only).
- This is a **read-only preview only**; neither source contracts nor native projection tests prove genuine loaded-GGUF tool choice, reviewed Git/FFprobe child process, historical independent source freshness, true OS descendant custody, installed Windows GUI behavior or a safe multi-step agent. The separately queued NSIS build and Qwen3 E1-P07 acceptance are the relevant independent gates.

## Next eligible work after acceptance

Once the genuine E1-P07 original ANSF → actual Qwen3 tool choice → independently verified reviewed Git/FFprobe worker → crash/restart trace and per-OS process custody pass, design a **new human-requested continuation transaction**. It must verify the actual historical source/revision from the worker journals, bind the next exact step/resource/budget to a NEW saved USER turn, obtain a fresh OS-native confirmation, and establish a new host owner with no inherited permissions. It must not unquarantine uncertain effects or reuse a prior released owner. A different plan revision or user goal requires a new human review. **Do not enable this runtime path from the preview.**

**Companions:** [K/I/A/F active tasks](LU_KIAF_ACTIVE_EXECUTION_LEDGER_20261108.md); [AGENT-01G execution acceptance](LU_AGENT01G_REVIEWED_STEP_EXECUTION_20261108.md); [physical Windows NSIS build](LU_AGENT01G_PHYSICAL_WINDOWS_NSIS_TEST_BUILD_20261108.md).
