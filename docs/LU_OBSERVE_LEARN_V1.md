# Lu v1.0 — OBSERVE & LEARN
Date: 2026-11-06
Status: REQUIRED FOR V1.0; FIRST POLICY-KERNEL SOURCE COMMITTED; native desktop capture, memory write, and installed acceptance OPEN.

## Purpose
Enable Lu to witness the user's permitted desktop interactions and app-test workflows without repeated prompts or continuous LLM inference. The host observes structured UI events, selectively requests sensitive-data-redacted screenshots when separately authorized, correlates GUI state with application/test logs, and proposes explanations or teachable workflows.

A visible button is only a UI observation. It does not prove a native effect, a successful test, a file write, or user intent. PASS/FAIL requires independent expected-versus-actual verification from a trusted test harness; otherwise report inconclusive evidence.

## Layered event-driven runtime
- Native observer: user-selected applications only, OS-attested process/window binding, Windows UI Automation first; Linux AT-SPI where supported. Use semantic event subscriptions and local throttling rather than a full-screen video recording loop.
- Transient visual observation: Windows Graphics Capture and Linux portal-authorized capture only with extra consent. Redact sensitive UI before inference; destroy raw pixel/audio buffers immediately; do not persist them.
- Signal controller: UI state changes, focus changes, error indicators and verified test checkpoints are bounded and deduplicated. Ignore low-value events.
- Model routing: no model call when the already-loaded model is unavailable or occupied. At most six short inference recommendations and one deep recommendation per minute in the initial policy. A trusted host may schedule lower-priority turns only when interactive work is idle; never automatically load a model or grant new tool permissions.
- Diagnostics: combine application-owned semantic events, OS accessibility tree and optional redacted screenshot. For Lu IDE, use development-ledger and native task/effect receipts instead of assuming screen pixels prove a test result.
- Memory: independently reviewed, provenance-bound ANSF/SQLite experience notes. Observation is not permission to remember; reviewed workflow proposals are not automatic memory writes, execution authority or model training.
- Learning: user-approved Teach Mode produces reviewable workflow candidates with validated steps and tested outcomes. Training datasets and local fine-tuning require an entirely separate explicit authorization and reviewed export.
- Audio listening: a separately consented microphone interface may use lightweight voice activity detection and trigger transcription only for permitted speech. No microphone grant is implied by desktop observation.

## Privacy and authority requirements
- OFF by default. Separate human-given permissions to OBSERVE, capture redacted snapshots, REMEMBER, LEARN, TRAIN, and operate computer controls; all consent revocable.
- User selects each allowed application through trusted OS UI. Include visible observing indicator and Start/Pause/Stop controls in the desktop before release.
- Short session lease (initial policy maximum two hours); no automatic restart, no surveillance outside selected apps, sensitive-window exclusion and no covert keylogger, clipboard harvesting, hidden microphone recording or background uploading.
- Never offer normal users a switch that disables mandatory privacy filtering; no research bypass in v1 baseline.
- Screenshots can only be ephemeral. Default raw capture storage is zero and must remain zero on failed/cancelled tasks.
- Observation does not authorize mouse/keyboard control, software installs, filesystem access or network fetch; all such effects require separate Lu host policy approval and durable receipts.
- Learning proposals are inert until human review. No implicit model-weights updates.

## V1.0 acceptance gates
1. OBS-01 — real OS application picker and identity binding, Windows UIA event adapter, Linux support or clear availability limits.
2. OBS-02 — explicit redacted screenshot permission, secure-window exclusion, immediate raw-buffer erasure and OS privacy tests.
3. OBS-03 — supervised observer worker, CPU/RAM event budgets, process-tree cancellation, expiry/revoke and restart fail-closed.
4. OBS-04 — native event to model short/deep request bridge, model-idle arbitration, no silent model autoload, GGUF throughput/idle GPU metrics.
5. OBS-05 — verified GUI testing for Lu and Lu IDE, independent pass/fail/inconclusive plus exact source and native logs; no screenshot-only success claims.
6. OBS-06 — reviewed ANSF experience memory with provenance, corrections and deletion/revocation, no covert recording or silent training.
7. OBS-07 — installed Windows GUI with clear Start/Pause/Stop, app allowlist, status feedback, privacy disclosure and real desktop workflow demonstration.
8. OBS-08 — Linux/Windows native CI, hostile-event and cross-app denial tests, memory recovery, idle-resource and sustained-session benchmark.

## Present implementation truth
The first module native_observe_learn.rs is a host-only *policy kernel*. It implements explicit scope and expiring session IDs, default OFF, pause/resume/stop/revoke, application-scope checks, optional redacted screenshot permission flag, bounded event rates and brief/deep reasoning recommendations, replay rejection, and inert reviewable learning proposals. CI includes Windows and Linux Rust tests, but the exact-head checks are not assumed green until run.

This does NOT yet capture the desktop, launch an OS observer worker, identify windows, invoke a model, train/modify model weights, persist ANSF records, or test an installed GUI. Those are mandatory follow-on implementations before v1.0 acceptance. Keep Observe & Learn a distinct tool family, not a renamed Reader, Browser, Vision, or Image Workspace tool.


## OBSERVE-LEARN-02 — host, TaskStore and ANSF integration (source only)

The dedicated observer shares the existing Arc<TaskStore> and its durable statuses, with ephemeral consent and event state kept inside the already running Tauri desktop. Startup interruption and a host watchdog prevent stale observation authority after restart or expiry. No separate runtime.

The native_observe_ansf bridge accepts only bounded, provenance-checked host-test results with separate human approval and writes them into the existing ANSF store. It rejects cross-chat records and duplicates. This bridge is not exposed to the WebView until there is a trusted native GUI test oracle and review UI.

The current Lu application pilot will observe only explicitly approved semantic UI event indicators, not Windows desktop pixels or other applications. A renderer hint can never mark a GUI test PASS or FAIL. Brief/deep reasoning suggestions remain inert until integrated with the existing model queue. Actual Windows UI Automation and user-authorized Graphics Capture are still required before full desktop observation.


**Chat-switch privacy fence:** The existing Tauri status endpoint immediately revokes a live observation if the user switches to another conversation or no conversation. The native task becomes CANCELLED; it never silently observes under a hidden prior grant. A new host after restart has no consent even if the previous task was INTERRUPTED. This is verified by focused native host tests, not yet physical desktop acceptance.
