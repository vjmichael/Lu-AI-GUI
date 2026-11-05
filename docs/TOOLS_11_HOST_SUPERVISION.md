# TOOLS-11 — Lu-owned background supervision and model notifications

**Decision (2026-11-04):** The AI model must not poll or monitor background
processes. Lu Core is the authoritative supervisor and event publisher. Model
inference is invoked only for a real question or a meaningful, host-admitted
notification that warrants reasoning. Idle progress must never burn model tokens.

Original Jan tool domain: `background` in §5.2 (start/status/stop). This
extension retains the per-task authority, native typed Protobuf, durable
journal, cancellation, and resource supervision already required by Lu.

## Execution model

1. User/model proposes a job. The **trusted host**, not the model, validates
   task authorization, backend capabilities, resource limits, and ownership.
2. The host mints the task ID; records an intent; then starts an *owned*
   worker through the existing OS process manager. No arbitrary shell.
3. The worker reports progress, checkpoint, liveness, and outputs through
   authenticated worker IPC. Lu persists the receipts in SQLite before
   presenting status as durable.
4. The UI receives host-created progress cards. No live AI generation is
   needed to display progress or detect an unhealthy worker.
5. The host durably queues **completion, failure, interruption, or explicit
   human-decision-needed** notices for the task's owning conversation.
6. When a model turn is eligible to consume an event, the host injects a
   bounded, trusted *notification envelope* as contextual input. It is **not**
   a model-authored tool result or permission grant. The user may instead
   receive UI-only delivery without waking or loading an LLM.
7. Delivery acknowledgements must be persisted. A crash cannot silently
   discard an unacknowledged event; replay may re-notify but cannot repeat
   a possibly applied tool effect. Every event has a monotonic sequence.

## Timeouts: media jobs are not model turns

A two-hour video or audio analysis job must not inherit a 60–120 second model
generation timeout. Three distinct limits apply:

- **Worker operation timeout:** bounded per decode/infer/encode operation,
  with cancellation and reaping of only the worker owned by Lu.
- **Liveness/heartbeats:** measured independently. A missed heartbeat may
  initiate probe/recovery; no fixed inactivity threshold should claim an
  operation failed without checking its phase and expected progress.
- **Overall task deadline:** configurable by job class or explicit user
  constraints, potentially hours; deadline expiry is a host policy decision.

Long media processing splits inputs into bounded segments (timestamps, frame
ranges or audio windows) with atomic, persisted output. The worker writes
intermediate artifacts to host-selected paths, not to LLM context. Lu uses
a resource guardian for GPU/VRAM, RAM, disk and CPU budgets. A model can be
unloaded or working in another chat during the job. If a worker crashes, Lu
reconciles the last checkpoint and effect state before deciding if a segment
may safely resume. **No automatic replay of UNCERTAIN effects.**

A live stream can use a bounded ingest queue and backpressure with explicit
drop/skip policy if real-time throughput is lower than input rate. Segment
results are indexed and optionally summarized for the model, never retained
indefinitely in a giant prompt.

## Notification policy

| Native event | UI progress | Model notification |
| --- | --- | --- |
| Started, running, heartbeat | Yes, coalesced | No |
| Ordinary checkpoint | Yes, throttled | No |
| Completed | Yes | Once, after durable acceptance |
| Failed / interrupted | Yes | Once with actionable reason, when eligible |
| Needs user decision | Yes | May request guidance; never grant itself authority |
| Resource warning | Yes | Only if a model action is actually useful |

**Safety boundary:** task ownership is checked at the task store and tool
registry. Model cannot create task IDs, alter event history, obtain raw
checkpoint bytes, expand workspace grants, or demand a worker kill.

## Implementation status

- Implemented (source): SQLite lifecycle events and sequence cursor in
  `tasks.rs`; read-only `native_background_status.rs`; host-filtered
  terminal event projection. Checkpoint notifications are deliberately
  suppressed for the model.
- Implemented (source, CI under verification): host-only supervised subprocess
  admission with executable digest pinning, Windows suspended launch + Job
  Object containment, POSIX owned process group, OS termination/reap, and
  durable CANCELLED receipts. Desktop operator cancellation requires a
  native confirmation and is shown only for currently owned tasks.
- Implemented (source): a read-only Background Tasks pane with real SQLite
  dashboard querying, active/recent filtering, progress, warnings, and
  refresh independent of model inference. The host watchdog polls child
  process completion and journals completion/failure.
- Not yet implemented: a packaged production media-worker catalog or user
  start command; authenticated worker progress IPC; verified resource
  budgets; Tauri push instead of dashboard refresh; durable model
  notification delivery/ack; long video/audio segment pipeline; installed
  Windows process-tree acceptance. Source-level features remain gated by
  native CI and desktop integration tests.
- CI: native Rust unit tests must pass on Windows and Linux, plus real
  installed-app testing later in release acceptance.

## First real host-worker integration: media integrity scan

A user can open the Lu Background Tasks pane and select **Scan media
file…**. The native OS file picker supplies a local video/audio file path
to the trusted host, which mints a unique task ID. The host launches a
separate copy of the **current installed Lu executable** using its
reserved `--lu-file-scan-worker` mode, after computing and independently
checking the executable's SHA-256. That child enters worker mode before
Tauri or any model is loaded. This is not an arbitrary-command interface.

The worker reads the file sequentially in **1 MiB bounded-memory chunks**,
publishes only numeric byte-progress frames and a final SHA-256 digest.
The host accepts those frames through a limited, line-oriented pipe,
checks monotonic byte counts and fixed input size, stores checkpoints in
SQLite, and marks completion only after process exit plus a valid
end-of-file digest receipt. Corrupt or truncated output causes a failed
task. Cancellation is still handled by the owner-specific OS process
supervisor; the model never polls the worker.

**This is a real media-file I/O/integrity scan, not video/audio content
understanding.** It does not transcribe speech, extract frames, or analyze
the content of the media. The scanner can run without any GGUF loaded and
does not inherit the model-generation timeout.

### Acceptance distinction

The Windows/Linux CI harness imports the *identical production worker
module* and executes a real native child against a 12 MiB file, checking
progress and SHA-256. The actual full Tauri desktop build remains
independently blocked by the unavailable verified llama-server executable
resource in GitHub source. Do not substitute a dummy engine or claim an
installed GUI acceptance pass from the harness alone.

## Scope decision: model-directed, host-supervised generic jobs (2026-11-04)

**Dedicated audio/video analysis engines are explicitly deferred.** Lu is a
general background-work manager, not an audio/video algorithm library. The
multimodal model or a future DLC chooses an approved media-specific algorithm,
but only Lu's trusted host starts and supervises the worker. The current
file-integrity scan remains the first real lifecycle acceptance fixture, not
the basis for implementing custom transcription, video decoding, or analysis.

The next essential gate is **an AI-accessible, strictly authorized typed task
control API**, discoverable in the model's tool catalog:
- `background.start`: propose an *approved job kind* and typed parameters.
  Host validates task-specific authority, input scope, resource budgets, and
  a registered worker capability. It returns a durable task ID promptly.
  Never accept arbitrary executable paths, raw shell commands, process IDs,
  or client-asserted permissions.
- `background.status`: exact-workspace scoped query (already implemented),
  without transferring checkpoints or host-private paths.
- `background.cancel`: request cancellation of an authorized, owned task;
  Lu carries out process-tree termination and durable receipt, optionally
  obtaining separate user approval. The model does not send OS signals.
- `background.result`: retrieve a bounded output manifest and trusted
  artifact references after completion, never an unlimited prompt payload.

Models must not poll heartbeat/progress or perpetually call `status`.
Lu watches processes independently and durably queues completion, failure,
interruption and decision-required events. A host-owned delivery cursor and
acknowledgement ensure notices are delivered to the right conversation without
silent loss or uncontrolled duplicate model turns. No event grants permissions.

Model-facing tool schemas, scoped grants and host-minted IDs let a multimodal
AI use long tasks **without writing a custom media analyzer into Lu Core**.
The worker may run for hours while the LLM is unloaded; results are stored
outside the context window and loaded in bounded portions when requested.

## Mandatory terminal push contract (2026-11-04)

A background worker must **never be responsible for waking the model**.
Lu's host supervises the worker and records one authoritative terminal
transition. The worker may report success/failure data, but actual OS exit,
verified results, explicit task deadlines, operator cancellation, and restart
reconciliation determine the host's final status.

Every terminal classification is notification-worthy:
`COMPLETED`, `FAILED`, `CANCELLED`, `INTERRUPTED`, `TIMED_OUT`.
A task that fails before launch is `FAILED`; an unexpected previous
`RUNNING` record after host restart is `INTERRUPTED`; an explicit
host deadline that *actually terminated and reaped* its worker is
`TIMED_OUT`. Missing heartbeat is not sufficient to claim timeout.

The terminal transition and a monotonic event sequence share a SQLite
transaction. A host-side durable inbox uses that sequence to support
at-least-once delivery and workspace-scoped idempotent acknowledgement;
neither model nor WebView can forge an end event. Lu emits
`lu-background-terminal` from a host-only watchdog and shows the event
on the desktop even when the Tasks pane is closed.

**Current model delivery:** a bounded, host-constructed event summary is
supplied to the **next eligible turn in the owning conversation**, without
copying worker-authored detail into model instructions. A notice is
acknowledged only after the AI response is durably completed. If the model
is unloaded, busy, fails, or the app restarts, the notice remains pending.
This deliberately does **not** auto-load a model, interrupt a turn, or
create an unsolicited synthetic user message. A separate, authorized
scheduler will later enable truly proactive model continuation if desired.

The present native file scan originates from an existing chat and uses its
verified conversation ID as the task's owner; jobs without a chat receive
a local UI-only owner and are not handed to an unrelated model conversation.

## Acceptance scenarios

1. 40-minute synthetic media task progresses with LLM unloaded; zero model
   inference calls for progress updates.
2. Restart after checkpoint: host reports INTERRUPTED and preserves outputs,
   then explicitly reconciles effect state; no blind replay.
3. Cancel task: OS-owned worker tree terminates, no orphans, no other task
   receives its cancellation.
4. Two workspaces: neither sees or receives the other's task or notices.
5. A burst of 10,000 checkpoints does not cause 10,000 model turns, UI
   freezes, prompt growth, or unbounded event delivery.
6. Worker blocked in a long but legitimate operation is not killed because
   it exceeds a short model token-generation timeout.
