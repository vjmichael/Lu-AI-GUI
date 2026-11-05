# Lu IDE R3.4 — Multi-hunk reviews and untrusted proposal envelopes

**Branch:** `ide/workspace-development`  
**Date:** 2026-11-05  
**Status:** Implemented on branch; require exact-HEAD CI and Windows interaction acceptance before closing R3.4.

## Delivered code

1. **Correctly grouped multi-hunk diffs:** `ide/host/src/review.rs` uses Similar's line diff algorithm and `grouped_ops(3)` to distinguish edits separated by unchanged regions. A review has `hunks[]` containing Git-style `@@ -old,count +new,count @@` labels and source/destination line numbers on each line. Added/removed counts are computed for all changes (not merely displayed lines) when the full input is within the computation budget.
2. **Windows editor UI:** `ide/app/web/main.jsx` and `styles.css` render separate diff hunk sections with original/proposed line-number gutters. The content remains escaped React text, not executable markup.
3. **Untrusted proposal envelope:** `ide/host/src/proposal.rs` defines a strict `schemaVersion=1` JSON data format. Required fields: `taskId`, `projectId`, `claimedProducer` (untrusted text, **not authenticated identity**), `rationale`, and up to four `edits` containing relative target path, exact SHA-256 base revision, and bounded proposed UTF-8 text.
4. Unknown or effect-bearing fields (`command`, `autoApply`, `approved`) are **rejected**, not ignored. Duplicate paths, traversal, invalid project IDs/revisions, oversized payloads and NULs are rejected. Previewing multiple proposed edits is **read-only** and independently revalidates each file's current revision.
5. **No Lu connection, no agent executable worker, no proposal application endpoint.** The existing human editor save is still a separate action requiring current revision. Self-declared `claimedProducer: "Lu"` does **not** grant authority or prove who sent data.

## Bounds and completeness

- Text source files are still limited to 512 KiB each in the standalone host.
- Multi-hunk diff computation is capped at 8,000 combined lines. Above that, the review is marked truncated, with `countsExact=false`, and the UI does not pretend to know the true change counts.
- The response emits at most 240 preview lines, 32 KiB of source text and 64 hunks. On any truncation, **Save reviewed changes** remains disabled.
- Small files with multiple distant edits get separate accurately located hunks, not one large misleading replacement block.
- Future work: explicit diff compute deadlines/cancellation for worst-case input, streaming or virtualized huge reviews, rich word-level diff, independent approval grants, durable proposal queue, authenticated Lu bridge, multi-file transactional apply.

## Concrete tests

- Host tests verify distant modifications form separate hunks, accurate old/new line numbers, newline and CRLF changes, pure insertion, size truncation and no false complete diff result.
- Strict envelope tests reject self-authorization and arbitrary command keys, duplicates, traversal, oversized edits and unsupported versions.
- Native integration tests preview two-file proposals **without changing disk bytes**, and fail the entire review on a stale source revision.
- Frontend tests verify rendered hunks/line numbers, no HTML injection, and absence of native agent-apply/execution endpoints.

## Required acceptance

- [ ] Exact HEAD Linux native Rust tests and `rustfmt --check`.
- [ ] Exact HEAD frontend build and tests.
- [ ] Exact HEAD Windows native host tests and Tauri compile.
- [ ] R3.4 Windows executable build, ZIP and checksum verification.
- [ ] Interactive Windows test: change two distant lines in one file; expect two hunks with correct line numbers, no extra removals.
- [ ] Interactive Windows test: edit file, review/cancel, verify bytes unchanged; reviewed save still requires explicit user action.
- [ ] Large/complex review still blocks reviewed-save rather than falsely presenting complete context.
- [ ] IDE works while Lu is disconnected; packages/build/test still unavailable.

## Architecture constraint

This work supports Task 3 (editor/review quality) and a foundation for Task 7 (Lu coding collaborator), without shortcutting Task 2's unresolved OS worker sandbox or Task 7's peer authentication and per-task authorization. The user continues to own each source save. Source content and model output are untrusted; a JSON envelope is **not a permission token**.

See [R3.3 Windows acceptance](IDE_07_PROPOSAL_REVIEW_FOUNDATION.md), [engineering handoff](../ENGINEERING_HANDOFF.md), and [canonical implementation plan](IDE_WORKSPACE_DESIGN.md).
