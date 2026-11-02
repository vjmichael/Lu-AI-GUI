# Lu Core milestones — memory, conversation UX, and optional A/V

**Status:** Owner design clarification / staged implementation (2026-10-25).
**Priority:** Reliable Lu 1.0 foundation first. Optional external workspace DLC ecosystem **after** Lu core release.

## Product boundaries

- Lu Core: chat, conversation identity, ANSF history, cross-chat recall,
  agent/task execution, tool policy, progress, and the basic extensible UI.
- Core UX configuration: keyboard submission behavior, chat bubble colors,
  shapes, reactions, per-chat backgrounds, workspace backgrounds, themes.
  Theme packages should be declarative, schema-checked assets, not executable
  plugins or hidden sources of authority.
- Optional first-party feature extensions: voice, camera, A/V ingest/analysis,
  accelerated model backends; these may be installed/disabled individually.
  Lu must work without them. Camera/microphone permissions and data retention
  must be explicit, visible and revocable.
- Workspace DLCs: *post-core-release* extensions, not implementation blockers.

## ANSF (AI-Native Session Format)

The context window is a **cache**, not the full archive. The original session
transcript stays on disk and never silently disappears when the model prompt
is compacted or the UI virtualizes older messages.

### Implemented storage slices

`ansf.rs` introduces a SQLite session book with:
- Durable session identities, topic chapters and summaries.
- Exact, append-only entries with stable numeric entry IDs and stable
  host-supplied message IDs. Original content is preserved independently of
  its navigational summary.
- SQLite FTS5 index on original text and summaries; session-filtered search.
- Cursor-paginated reads, suitable for a virtualized chat and secondary
  memory-worker pane.
- Explicit topic changes: the host closes one chapter with a summary and
  starts another. No implicit model-generated reclassification.
- WAL and FULL sync with transactional writes.

The next slice, `ansf_library.rs`, implements the separate SQLite `library.db`
concept: a global reference-only index for sessions (books) and chapters
(tables of contents). It provides global FTS5 book/chapter search, exact
source-location metadata, idempotent reindexing, and removal of stale index
entries without deleting the source ANSF book. It never copies messages into
the catalog. The desktop will need to authenticate access, supply confined
paths, and rebuild/search the catalog in the background.

**Not implemented yet:** Jan thread migration/import, automatic topic shift
detection, semantic/object memory,
context relevance assembly, transclusion links, revision-aware beliefs,
virtualized renderer, in-GUI memory search, model/tool APIs, multi-modal blob
storage, retention/purge/delete procedures, and host-authenticated role/owner
enforcement. The host must supply trusted session/message IDs; this crate
does not authenticate its own callers.

The design uses one SQLite DB per conversation and a separate compact
cross-conversation library index. The host chooses and confines book paths.
Index writes across databases are not atomic; rebuild from the original
ANSF books after an interruption, and never treat search metadata as a grant.

### Immutable edits and revision notations (new storage slice)

Jan can rewrite a message with the same original message ID. ANSF must not.
`ansf_revisions.rs` now appends a new immutable entry for each genuine change
and records an immutable SQLite edge: original entry → prior revision → new
revision. The original Jan message ID continues to identify the initial
version; `current_message` resolves the newest version. Historical messages
stay readable and indexed, including their exact old wording.

For a **trusted, directly observed human edit** the Memory Worker pane should
show `Edited · MMddyyyy HH:mm:ss` beneath the currently displayed bubble,
localized to the user's timezone; hovering/clicking shows `View versions`,
old text, previous version references and the full edit chain. The timestamp
on the underlying record is UTC (ISO 8601). An **imported changed snapshot**
must instead show `Source update detected · MMddyyyy HH:mm:ss`, because the
true user edit time is unknown. The display is not yet implemented.

The Jan importer defaults to refusing unexpected edits. Only a trusted host
which verifies that a Jan snapshot is newer may select the explicit policy
that appends `ImportObserved` revisions. A stale backup must not silently
become the latest source. Edits never grant tool execution authority.

### Future source references and UX

`library -> session -> chapter -> entry`; exact entry IDs support direct
search, jumping to a passage, neighbor-window loading, and cross-chat
references without duplicating originals. Linked chat content remains
**untrusted reference material** in the destination context; it never
carries authority from the source conversation.

Message reception indicators should distinguish:
- `Received`: successfully stored/queued, no inference needed.
- `Processed`: coordinator has classified/understood the update.
- `Applied`: a correction actually changed the task or its plans.
Never simulate understanding with a green check merely because the UI saw
the text.

A lightweight conversation coordinator may handle acknowledgments and
non-actionable comments without invoking the full LLM. Tool effects and task
revisions remain controlled by the verified host policy. User-selectable
Enter-to-newline and Ctrl+Enter-to-submit modes are presentation behavior,
not changes to stored turn meaning.

## Long audio/video processing — future first-party extension

A/V import must never synchronously serialize a whole 4K video to a data URL
and place it in a chat store. Ingest **references** to content-addressed,
locally managed media files and process them with bounded streaming readers.

### Proposed pipeline

1. Intake: validated file handle, metadata, size, duration when available,
   MIME/container detection, content hash, and a task bound to the originating
   conversation. Merely attaching a file may authorize *read/inspect* under an
   opted-in setting, not cloud upload or executing file instructions.
2. Demux/decode: incrementally read packets; use bounded audio and video
   buffers, seek/index keyframes when needed, and avoid retaining a full file
   or all decoded frames in memory.
3. Analysis: run low-cost CPU tasks (duration, streams, waveforms, sampled
   thumbnails) first; schedule optional GPU decode/inference for workloads
   that benefit. User-visible policy limits GPU time, VRAM, batch size, and
   concurrent jobs; media can be analyzed without loading a second LLM.
4. Results: persist timestamps and provenance for frames, transcripts,
   captions and recognized segments. ANSF references those artifacts,
   rather than duplicating the binary media in its conversation messages.
5. Durability: use Lu's existing `TaskStore` checkpoint, heartbeat,
   interruption and progress records; preserve completed segments and resume
   or retry only idempotent stages after application restart.

### Progress and timeouts

A task reports **phase**, bytes processed/total bytes, media time position
versus duration when known, progress estimate when measurable, monotonic
elapsed time, last heartbeat, output location, and cancellation/retry state.
For non-measurable work, show an indeterminate progress indicator, **not a
fabricated percent**. Elapsed time is calculated in the active process;
across restarts accumulate persisted elapsed time.

No arbitrary hard *job-duration* timeout on large files. Nonetheless there
must be bounded stall detectors and subprocess timeouts, memory/queue quotas,
cancel and cleanup controls, and safe recovery; "never time out" must not
mean an unresponsive decoder blocks indefinitely. A stalled worker is
diagnosed, restarted, or paused with a checkpoint and an accurate error.

Incremental decode does **not** imply zero RAM: compressed packets, codec
reference frames, audio buffers, thumbnails and ML batches need finite
buffers. GPU processing is optional and requires an explicit scheduling
decision; feature detection should choose capable backends, not silently
downgrade to ancient algorithms.

### Acceptance examples

- A large MP4/WAV is ingested by path/handle; GUI RAM does not scale
  linearly with duration or hold an extra base64 copy.
- Closing or switching the chat pane does not lose task progress.
- Elapsed time, current phase, processed bytes/media timestamp, heartbeat
  and user cancellation remain inspectable.
- A decoder/analysis worker crash does not destroy existing artifacts; jobs
  restart from safe segment checkpoints or report the precise interruption.
- GPU admission policies prevent optional A/V analysis from starving the
  primary chat model without user consent.
- Local-first works offline; no cloud upload occurs unless separately
  authorized by the human.
