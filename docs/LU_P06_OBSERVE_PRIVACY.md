# Lu Core P06 — OBSERVE-PRIVACY-01 (Observe & Learn)

## P06 remaining-gaps implementation batch (latest)

**Source changes:** OS foreground capture is now bound to HWND, PID and Windows process creation FILETIME, with independent recheck in the D3D11 callback; an OS picker result without source identity remains explicitly unverified. The trusted worker emits only a full-frame-blanked salted receipt—no original BGRA pixel crosses to host/LLM, and no claim of selective secret-region redaction. One 512 MiB CPU worker Guardian lease is admitted before spawn and held through process cancellation/reaping; GPU accounting is still zero/unverified. The installed-Windows evidence harness can verify reviewed EXE and WGC-worker hashes plus native process/window launch and (optionally) independent UIA initial Off state, **not an actual physical WGC picker/NSIS cycle**. Focused P06 CI now builds React before the Tauri macro and compiles the actual Windows WGC binary.

**Worker cleanup race repaired:** A failed OS process-supervisor poll is now UNCERTAIN and cannot be treated as a finished worker for release of a Guardian lease, successful Pause/Stop or a completed capture receipt. The native capture grant is revoked before cleanup, and the supervised process remains owned until a verified terminal transition.

**Windows packaging repair:** `scripts/package-observe-capture-windows.ps1` previously contained duplicated/corrupted PowerShell and invalid SHA checks. P06 rewrites one supported native worker/React/desktop build flow with opt-in NSIS and optional Research overlay, plus strict hash-only installed binary preflight. Dedicated P06 CI syntactically validates this script as well as the physical installed-app GUI smoke harness; it does not install the application, interact with a WGC picker, or certify an installer.

**Follow-up native/installed evidence repairs (same P06 work order):** The Windows frontend acceptance checker still asserted obsolete UI strings and the old `observer.launch_one_shot(&chat_id,plan)` signature; it is now aligned to the actual Guardian-admitted path, visible review-only handoff and current redaction notice, not weakened into a dummy PASS. A real `ObserveHost` test verifies that rejecting a malformed capture plan returns its Guardian CPU/RAM lease without starting a worker. The installed-Lu harness now provides an optional explicit `-ExerciseObservationLifecycle` check, using native Windows UI Automation for consent request, pause, resume and stop. The native consent modal remains a separate human decision; the script cannot approve it, capture a desktop frame, declare backend proof, or promote GUI appearance to a verified native effect.

**Outstanding:** exact-head Windows/Linux compilation and repairs; real interactive physical WGC identity/picker/cancellation trials and installed NSIS acceptance; OS identity for generic picker/monitors; trusted sensitive-region selective redaction before *any* image reaches Lu's model; genuine pixel-to-model scope and verified GUI test receipt. User-deferred actual AMD Vulkan/MMProj installed acceptance remains unclaimed. No P07 implementation is authorized.


**Owner order:** explicitly start P06 after P03 on 2026-11-06. `lu/p06-observe-privacy-20261106` is stacked on P03 draft PR #71 exact source `f54fd82cccd5803b2912efe7d8be513b19b9b559`. This is a separate development branch: the ongoing Observe & Learn draft PR #61 is left unchanged; the current P06 branch retains Observe source already integrated into the newer P03 tree. No P04/P05/P07 work was authorized by this order.

## P06 scope: privacy before active vision

P06's priority is *OFF by default; explicit human consent; event-driven, bounded native observation; visible pause/stop and revocation; provenance-marked verified evidence; human-reviewed handoff*. Not a continuous screen recording, always-running transformer, autonomous training, or an AI with unsupervised OS access.

### Already present and reused

- `native_observe_learn` controller denies events without explicit consent, enforces a 30-minute current Lu-only scope, allows pause/resume/stop, expires grants, bounds event rate and never automatically writes memory.
- `native_observe_scope` restricts window/app/monitor/virtual-desktop targets; renderer hints are never trusted OS identity or source proofs.
- `native_awareness_lane` runs cheap native event classifications with explicit renderer/native/runtime/test provenance and deduplication; small model and main model handoff remain recommendations, not automatic inference.
- `ObserveHost` reuses native TaskStore, BackgroundProcessSupervisor and the current desktop; read-only notes and handoff exist. Single-frame Windows WGC capture remains separately prompted and process-supervised. Receipt hash alone is neither a verified source identity nor a model visual interpretation; no raw pixels are retained in the normal Lu pilot.
- `native_gui_verification` compares intended source/artifact digests against *independently attested* GUI/test evidence, returning PASS/FAIL/INCONCLUSIVE. Real independently operating Windows WGC/GUI test proof is still an acceptance gate, not established by the comparator itself.
- ANSF publication remains separate, off by default and human-reviewed. No renderer event or model suggestion can independently publish memory, run code, or start training.

### New implementation on this P06 branch

1. **SQLite source provenance repair:** previous `lu_awareness_notes` CHECK constraint recognized renderer/accessibility/native-test sources but *rejected native-runtime source values* that the trusted ObserveHost already emitted. New installs include the genuine source. Existing installs perform an atomic copy/rename migration preserving old rows and recreating the scoped index before they can append native-runtime notes. Native failure is an **unverified runtime indicator**, not a GUI test verdict.
2. **Bounded metadata retention:** enum-only awareness rows automatically expire after **seven days**, pruned at TaskStore open and each consented notes/handoff read. Retention is logical SQLite deletion; never claim forensic erasure of old WAL, backups, previously copied prompts, or chat history. No screenshots, paths, app contents, prompts, or key presses enter this table.
3. **Per-chat forget:** native `lu_observe_forget_notes` command checks a real existing conversation, prompts with a **native confirm/deny modal**, and invokes `TaskStore::delete_awareness_notes_for_workspace`. SQL filters by exact chat and `obs-%` task identity. An explicit cancel is distinguished from confirming zero notes. This does NOT delete independent task lifecycle/audit records, ANSF memories, copied text, ordinary chat messages, or live capture processes.
4. **Prevent stale handoff after forgetting:** ObserveHost clears current-session counts and invokes `AwarenessLane::discard_observations` when deleting current chat notes. This clears volatile note and suggestion history but **does not rewind monotonic event sequence**, preventing replay. Observation scope and consent do not widen or auto-start; a new event requires the same live host grant.
5. **Visible review:** the React observer pane exposes Forget for active/paused and off sessions, reports seven-day retention, clears stale locally rendered notes/handoff on confirmed deletion, and leaves native Capture/Model/ANSF authority unchanged.
6. **Tests:** P06 native tests cover existing SQLite schema migration, trusted native-runtime insertion, chat-isolated deletion, TTL pruning, no stale review or replay, and native host provenance. New Node contracts check the actual Tauri/React wiring. Focused `.github/workflows/lu-p06-observe-privacy.yml` compiles/tests Lu Core on Windows and Linux, builds React and checks/tests the Windows desktop host, without faking an inference engine.

## Acceptance distinction

**Source integrated** is not **native CI verified** until the final exact-head Windows/Linux jobs pass. It is also not **physical Windows WGC/installed NSIS/model acceptance**: these remain deferred by owner order. The capture worker's OS picker, source identity, sensitive-region redaction, live GUI behavior, and capture-to-model bridge require independent physical acceptance and more implementation. The optional resident small-model runtime, independent model scheduling, and automatic IDE diagnosis/ANSF publication remain deferred; do not silently turn on observation or train models. P04 Reader/OCR and P05 Browser/Transfer are separate postponed work areas.

P06 can be considered *privacy foundation source implemented* after these changes. Full Observe & Learn release acceptance remains OPEN. Stop before P07 until instructed.
