# UX-CHAT02 — Immutable original, human display corrections, pinned replies, provenance

**Date:** November 8, 2026. **Parent:** UX-CHAT01 draft PR #157. **Status:** Source/native-test candidate, not production-installed acceptance.

## Why this exists

Real messaging history needs edits, replies, provenance and continuity. Lu's canonical ANSF records must remain truthful, though: an edit to something a model originally saw must **not** rewrite the historical user/model turn, silently change past tool authorization, or claim that a prior worker did different work.

## Implemented source

### Immutable version chains

`lu-desktop/src-tauri/src/chat_message_revisions.rs` adds two tables to the existing host-selected `lu-message-interactions.sqlite` UI-only database:

- `lu_message_revisions` is an append-only chain indexed by the actual original `(chat_id, ANSF visible seq, version)`. Each version stores the unchanged original SHA-256, previous version SHA-256, SHA-256 of corrected text, full human correction, editor attribution and timestamp. A maximum of 16 revisions per original and 16 KiB of corrected UTF-8 text per revision is allowed. Separate SQLite triggers forbid mutation and deletion of these rows.
- All revisions are **human display corrections**. Even corrections to a response originally authored by Lu remain labeled `HUMAN_DISPLAY_CORRECTION`. Inference context, tool results, preserved worker journals and existing ANSF original messages remain unchanged. This is not a retroactive tool/task repair or a hidden model rewrite.
- `edit(... expected_version,expected_sha,...)` enforces optimistic concurrency against the latest current revision under an immediate SQLite writer reservation. Stale edits and any source-hash mismatch fail closed. `read` verifies every link to the original and SHA chain before showing any correction. The original is always accessible as version 0 in the version-history dialog.

### Durable reply-to-message relationships

- A user can open **Reply** on a saved message, see the exact displayed version, and explicitly post a **new human-authenticated message** pinned to that message's original sequence, current revision number and full SHA-256. The source is rechecked under an immediate write transaction before admitting the reply intent; a stale version cannot be pinned as if current.
- `lu_message_reply_intents` stores host-minted `g-...` reply-turn IDs, exact parent coordinates, pinned revision SHA, prepared reply-text SHA, and a lifecycle of `PREPARED → LINKED`. Parent identity/digest cannot be changed once prepared; a unique reply-turn identity and immutable-update triggers prevent retargeting.
- The host first persists PREPARED metadata, then calls **`ChatStore::append_user_with_turn_id`**. A new Lu Core `authenticated_chat_turn_visible_seq` indexed lookup and desktop facade prove that precise new ANSF human turn's original visible sequence. It never chooses a reply by text match, the latest message, or an arbitrary WebView-supplied message ID. The host then links the original turn if and only if its text matches the prepared SHA. Only original human-authorized chat turns, never old model replies, can become newly posted replies.
- If Lu stops after the ANSF append but before the link becomes LINKED, the UI displays outstanding metadata. **Reconcile saved reply links** queries existing authenticated user turns and attaches their original sequence; it NEVER posts, replays, retries, or generates another message. An intent with no proven matching original remains pending, visibly unresolved. A successful new reply never automatically calls GGUF or Git/FFprobe; later model conversations may naturally include saved human messages as history according to the normal context rules and freshly granted authority.
- A later display edit to the parent **does not change the pinned version/excerpt** of earlier replies. Cross-chat links, wrong sequence, bad revision SHA, unauthenticated or mismatched child turns are rejected. Maximum 2,048 linked reply intents per chat with bounded reconciliation.

### UX

- Existing original message actions gain **Edit display**, **Versions / source** and **Reply** controls; interactive corrections and provenance are loaded in independent batches of up to 64 original ANSF sequence IDs, preserving hot-scroll bounds.
- A human correction is labeled in the bubble. The separate Versions/source view shows original text, all corrections, SHA-256 and reply's pinned parent reference. Regular **Replay** remains an exact-original display, not the edited text and not an OS effect.
- Reply writes have a dedicated composer and explicit **Post linked human reply**. They do not silently use the normal AI send path, so the model isn't triggered and native application permissions cannot be inherited. A pending-link notice offers an explicit reconciliation action.
- On chat navigation all modal state closes, so a dialog cannot accidentally post into another chat. Original ANSF page navigation stays unchanged. No arbitrary UI CSS, SQL, model-derived permission or filesystem path is accepted.

### Native acceptance

The dedicated `.github/workflows/lu-ux-chat02-revisions-replies.yml` workflow requests exact-head REPO-01 source verification, five production Rust integration tests on Ubuntu 24.04 and Windows Server 2025 using the real Lu Core ANSF + desktop ChatStore, Node/React source checks, and a full Windows desktop cargo check. The native tests cover immutable version chains across restart, two versions and stale edit rejection, source parent pinning after subsequent edits, prepared reply and crash-before-link recovery with no repost, cross-chat/wrong-target rejection, invalid input, and actual append-only SQLite triggers.

**What these tests do not prove:** an installed Windows NSIS app displaying/copying/entering edits, actual WebView focus/hot-scroll, native clipboard behavior, a genuinely loaded GGUF with tool execution, or OS descendant reaping. Those remain independent acceptance requirements near v1.0.

## Follow-up work

- Explicit UI restoration of a previous display version should create a **new revision**, never update/delete history.
- Richer threaded conversation navigation, reply grouping and visual context (links pinned to original source and version, not mutable text).
- If user wants a corrected message to influence future local-model inference, implement a **new human-authenticated correction turn** with visible provenance and explicit model authorization, not rewire the old executed turn.
- Source-level preservation/migration of the derived UX metadata when chats are deleted/archived needs separate product decisions; never silently rewrite original ANSF sources.
