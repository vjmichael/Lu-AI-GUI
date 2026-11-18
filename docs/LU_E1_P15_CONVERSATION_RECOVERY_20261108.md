# E1-P15 — Durable conversation task continuity (draft candidate)

**Date:** November 8, 2026. **Issue:** #149. **Parent:** E1-P14 PR #148. All E1 native and physical installed acceptance gates remain open.

## Purpose

Preserve a verifiable relationship between the original human chat request, the model's selected operation, its distinct host-controlled OS worker receipt, durable checkpoints and a separate independently observed source outcome. The result must be understandable after interruption, without treating an APPLIED effect as factually verified or automatically repeating it. Lu Core stays unsandboxed; no new shell, executable permission, Docker or mandatory MCP is introduced.

## A — Durable task-to-conversation identity

The existing TaskStore now has a singleton lu_reviewed_turn_origin SQLite table with the exact conversation ID and original human generation ID. The desktop writes the mapping only after the authentic user turn has been independently checked; attempts to rebind a journal to a second conversation or turn fail closed. Old journals without a recorded origin do not acquire retroactive provenance.

## B — Host-only read-only recovery projection

The new reviewed_task_continuity.rs reads the original per-turn SQLite task and effect journals in READ_ONLY mode. It checks host-owned file names, ordinary files and sidecars, bounded journal sizes, actual model-to-worker operation call mapping, original scoped completed worker task, last checkpoint percentage/count, retained receipt and independently recorded matching receipt hash. Git HEAD, status, branch, history and media facts have fixed operation names. It never displays raw worker stdout, checkpoint bodies, local paths, source hashes or credentials. Missing, damaged or inconsistent records are reported as unresolved; no old result is silently promoted.

## C — Safe continuation decision (no old-work replay)

The host returns RECONCILIATION_REQUIRED when any evidence is absent, uncertain, contradicted or not independently verified. FRESH_REQUEST_ONLY is possible only for previously verified historical operations and still requires a new explicit human request, new authority, a new effect ID and newly reviewed source. It does NOT prove present source state, active worker termination, or permission to run again. The UI has no Continue/retry action, and TaskStore.resume is never invoked from this recovery projection.

## D — Conversation-visible history

The existing lu_tool_turn_history command already authenticates the chat and saved human turn. It now attaches a read-only recovery timeline from matching host journals. Native tool activity shows per-operation outcome, checkpoint count/progress, original model call and worker journal linkage, and human-readable next steps. The data is reloaded from SQLite after restart rather than inferred from in-memory progress indicators. No model/WebView-provided paths or permissions are accepted.

## E — Native and installed-app acceptance

The dedicated workflow lu-e1-p15-conversation-recovery.yml asserts an exact frozen Git source head and requires five real Node regression tests, actual TaskStore origin tests on Linux/Windows, the real desktop recovery Rust module compiled and tested on both hosts through the portable production-source harness, plus a Windows React/Tauri compile. CI-only inference placeholders are explicitly NOT a real model or installed application.

**Installed Windows acceptance OPEN, deferred to the pre-v1.0 release-candidate phase.** Required real-world tests:

1. Install the real NSIS Lu build, launch GGUF on approved physical Windows/Vulkan hardware and open multiple persistent conversations.
2. Have a loaded real model independently select the authorized Git or FFprobe tool and inspect actual approved local resources; capture executable identities and verified worker/oracle receipts.
3. Kill or interrupt the installed app during actual execution; reopen the app and inspect the original conversation, last durable checkpoint, interrupted effect uncertainty and separate OS descendant process state.
4. Try partial SQLite journals, wrong original human turn, changed Git/workspace/media bytes, mismatched outer/inner effect IDs, malformed or symlinked journals and repeated unacknowledged notices.
5. Verify safe follow-up requires a NEW human authorization and NEW effect call IDs. Confirm no auto-replay, no false green outcomes, no cross-chat leakage and normal uninstall/reinstall data preservation. Retain actual installed binary hashes, runtime logs and visible evidence.

## Acceptance dependencies

- E1-P03 repaired Windows Job Object actual launch/cancel/reap/recovery on exact head.
- E1-P06B/C/D/E actual Git and FFprobe subprocesses with bounded typed output.
- E1-P07 real loaded GGUF self-selecting reviewed tools, not a scripted response.
- E1-P09/P10/P11/P12 real independent Git/WAV/MP4 outcomes and clear human evidence.
- E1-P13 host-approved source revision custody and actual mutation tests.
- E1-P08C delegated cgroup-v2 kernel proof remains a separate Linux descendant custody gate for escape-capable workers.
- Reconcile diverged ancestor PR heads and obtain exact-head native evidence before merging.

**Limit:** task and effect SQLite are read in separate read-only snapshots. This is not a cross-database atomic commit, a transactional resource snapshot, proof of current worker custody or an executable continuation protocol. The active, authorized resume/retry path must remain gated on native OS acceptance.

See issue #149 for further destructive tests and acceptance requirements.
