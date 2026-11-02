# Lu R18 — Cross-conversation generation scheduling

**Status:** Source and local tests passed; independent physical Windows acceptance pending.
**Repository:** `vjmichael/Lu-AI-GUI`; develop locally, GitHub as source record.

## User-observed fault

R17 allowed moving to another chat while text generated, but it still had one
frontend `working` flag, one active generation ID and one stream buffer.
A second conversation could not submit a queued request without interrupting
work in another conversation. Lu Core intentionally reserves **one active turn
per loaded model session**, so trying to run both would produce Busy errors.

## R18 contract

- Continue using one safely supervised llama-server process; do **not** claim
  physical concurrent token generation from one session.
- Enqueue up to eight outstanding turns (one per chat) using a bounded
  host-owned FIFO with RAII ticket release, mutex/condition-variable waiting,
  and model-session affinity. A full queue fails explicitly.
- Journal the user submission before acknowledging the queued request; on
  activation, assemble that conversation's most recent context, excluding
  the current user turn from the prior-history portion.
- The active model cannot unload/reload while any chat has a queued or active
  ticket; session identities are rechecked before engine reservation.
- Queued cancellations are host-local and never cancel the other conversation
  using llama-server. Active cancellations carry the generation identity into
  the existing Lu Core supervisor. The pre-reservation race is handled by
  the latched cancellation flag.
- Tokens and start/queued events carry both chat and generation IDs. The
  React frontend owns per-chat buffers and displays `Queued` / `Generating`
  for the correct chat; switching the visible chat never cancels inference.
- The Send action is blocked only for the chat currently submitting or
  generating (plus explicit model-management operations). Folder and favorite
  actions remain available while another chat generates.
- Canceled partial answers remain in their conversation's persistent journal;
  never import unfinished assistant text as authoritative context.
- Queued turns do not execute simultaneously; true multi-context inference
  and continuous batching are a separate, optional backend feature.

## Acceptance checks

1. Queue tests: FIFO handover, queued cancel isolation, cross-session denial,
   bounded admission, pre-start cancellation.
2. UI reducer tests: two independent chat buffers, mismatched token ignored,
   no global busy guard preventing another chat's Send.
3. Frontend bundle and legacy tests; Windows MSVC-target Rust compilation.
4. Physical Windows: start A, switch B and submit without canceling A; B waits
   and then runs; canceled queued B must not terminate A; switching back to A
   must show its complete answer after restart.
5. Verify engine GPU-offload separately; this change does not assert AMD
   Vulkan inference or multi-context performance acceptance.

## Limitations

- At most eight outstanding generations across the desktop, one outstanding
  generation per chat and one physically executing generation per model.
- A submitted queued turn remains in the durable journal even when canceled
  before it begins; no fabricated assistant output is inserted.
- Development-only NSIS package, full Windows GUI acceptance pending.
