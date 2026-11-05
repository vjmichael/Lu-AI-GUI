# Lu IDE R3.4 — Multi-hunk reviews and untrusted proposal envelopes

**Branch:** `ide/workspace-development`  
**Date:** 2026-11-05  
**Status:** Native Windows/Linux/frontend CI and unsigned Windows release **passed**. Human Windows multi-hunk interaction acceptance remains open.

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

- [x] Linux native Rust suite: **42/42 passed**; `rustfmt --check` passed in [CI run 42053180326](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42053180326).
- [x] Frontend build and **13/13 tests passed** in the same run.
- [x] Windows host tests and standalone Tauri compilation passed in the same run.
- [x] Optimized unsigned Windows executable built in [run 42053262838](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42053262838), artifact `lu-ide-windows-alpha-r3-4-multihunk-review-v3`. Windows PE EXE verified (10,564,608 bytes), SHA-256 `efe38b25c1e60bd46dc7be764ad168e5cdb84f97882077a96caf014d1ae363da`. User-facing ZIP `Lu-IDE-Standalone-R3.4-MultiHunk.zip` integrity-checked, SHA-256 `aed3b1b9a994620623cd4d55c9a2519635f3385a30857a729aa394d0e3da60f7`; includes `R34MultiHunkProject` and `README-R3.4.txt`.
- [ ] Interactive Windows test: change two distant lines in one file; expect two hunks with correct line numbers, no extra removals.
- [ ] Interactive Windows test: edit file, review/cancel, verify bytes unchanged; reviewed save still requires explicit user action.
- [ ] Large/complex review still blocks reviewed-save rather than falsely presenting complete context.
- [ ] IDE works while Lu is disconnected; packages/build/test still unavailable.

## Verification repair history

- The first native build failed on a Rust ambiguous integer type for the diff byte counter. Fixed by explicit `usize` annotations.
- The next native run passed all tests but required `rustfmt` normalization. The exact formatter output was committed.
- A subsequent frontend source-inspection test assumed the Rust proposal method signature was one line; after formatting it wrapped onto multiple lines. The assertion was corrected without loosening the runtime schema.
- Only the **final green CI and release runs above** represent this deliverable. Earlier Windows release attempts were unsuccessful and are not distributed.

**Windows human acceptance is not implied by the GitHub build.** The user should test two distant edits in one file and verify that two independently numbered hunks appear before this milestone is closed.

## Architecture constraint

This work supports Task 3 (editor/review quality) and a foundation for Task 7 (Lu coding collaborator), without shortcutting Task 2's unresolved OS worker sandbox or Task 7's peer authentication and per-task authorization. The user continues to own each source save. Source content and model output are untrusted; a JSON envelope is **not a permission token**.

See [R3.3 Windows acceptance](IDE_07_PROPOSAL_REVIEW_FOUNDATION.md), [engineering handoff](../ENGINEERING_HANDOFF.md), and [canonical implementation plan](IDE_WORKSPACE_DESIGN.md).
