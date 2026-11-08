# IDE-BUF-01 — Revisioned unsaved-buffer source identity

**Date:** 2026-11-05  
**Status:** P05 independently re-ported from source PR #43, pending native installed verification; original PR remains open.  
**Target:** Human-first, AI-ready **standalone** Lu IDE. No active Lu integration.

## Why the IDE needs two revisions

The saved source SHA-256 (`diskRevision`) identifies bytes observed on the filesystem and remains under the native Rust host. An **unsaved CodeMirror draft** is different state. It can change without any disk mutation and cannot be cited accurately using the saved SHA alone.

A draft reference therefore contains:

| Field | Purpose |
| --- | --- |
| `projectId`, `relativePath` | Source identity within registered project |
| `bufferId` | Cryptographically random UUID for a single open/reloaded/staged editor instance |
| `bufferRevision` | Monotonically increases on *every* CodeMirror document transaction, including Undo |
| `baseDiskRevision` | Last successfully observed/saved disk SHA-256, not a claim of present disk contents |
| `fromUtf16`, `toUtf16` | Half-open CodeMirror offsets, exact for editor selections |
| `startLine`, `startColumnUtf16`, `endLine`, `endColumnUtf16` | One-based human-readable locations |
| `fromByte`, `toByte` | UTF-8 byte offsets in the *captured draft*, not the disk file |
| `draftSha256` (clipboard representation) | Hash of the entire draft computed in the WebView from exact copied revision |
| `lifetime` | Explicitly `session-only`; no promise that it resolves after app restart |

**Draft identity is NOT a filesystem authority or durable file identifier.** The parent React editor owns a volatile identity map indexed by tab key. The identity is created when a source tab opens, reloads, or a reviewed proposal is staged; deleted when a tab/project closes; and rebased on successful Save. A Save does not silently erase more recent unsaved changes. Copies revalidate ID and revision *after* asynchronous digest calculation.

## Human workflow

1. Open a source file in Explorer. Existing saved-source reference remains available and still requires a clean saved file.
2. Edit the file. The status bar shows **Draft revision N**. Undo/Redo generate new revisions even if text becomes identical to an earlier draft.
3. Select a specific code span, or place the caret on a line.
4. Choose **Copy draft reference**. The IDE copies a structured, session-scoped reference with a full draft SHA-256. It does **not** copy or publish the source text, call an AI model, run a process, or save a file.
5. The reference resolver can use a current identity and CodeMirror state to reject stale ranges. References saved outside the session are informational and cannot be assumed actionable on reopening.

The selection and line map use CodeMirror's UTF-16 indexing (with each line break counting as one editor position); byte offsets are independently calculated using UTF-8 over CodeMirror `state.sliceDoc()`, which preserves configured CRLF serialization. Uniform CRLF/CR files retain their line separators using `EditorState.lineSeparator`. Mixed-line-ending documents remain a separate editor preservation limitation and must not be confused with saved disk bytes. The reader refuses ranges spanning more than 120 lines or more than 32 KiB of selected UTF-8 text.

## Boundary guarantees

- Pure JavaScript `app/web/draft_ref_port.js` only manipulates already-owned editor text and immutable identity metadata.
- No new Tauri command, model endpoint, port, native tool authorization, disk mutation or project-file upload is added.
- No persistent storage of unsaved drafts. The operational continuity ledger does not receive draft content or identities.
- User must still explicitly Save using existing native revision-conflict checks to publish file changes.
- This initial slice is an **in-process buffer reference**. A future AI-ready semantic buffer capability must separately establish authenticated actor, host/IDE lifecycle, current revision precondition, human authorization for effects, and bounded disclosure rules. A clipboard reference alone grants none of these.

## P05 source reconciliation
The B2 draft reference is independently ported with a new module path to the evolved E2 editor. The nine unported *original* blob paths remain absent under the P05 provenance guard, and the historical PR and CI cannot close on source equivalence alone. Source test and actual installed Windows/Linux acceptance required before acceptance.

## Regression and acceptance

`app/tests/draft_ref_port.test.mjs` covers CRLF, Unicode/emoji offsets, byte/UTF-16 differences, undo staleness, identity reset, Save rebase, malformed/out-of-range spans and non-disclosing clipboard formatting. `standalone.test.mjs` checks UI integration and the absence of new host IPC permissions.

**Open gates:** PR #43 frontend tests/build, Linux host tests, Windows Tauri check; installed Windows GUI check for Copy draft reference and clipboard; tab-switch/undo/redo/save/reload behavior on a physical machine. No installer produced or acceptance claimed for this feature.
