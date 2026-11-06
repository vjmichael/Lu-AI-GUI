# IDE-B3 — Find in Project (saved-text, read-only)

**Implementation branch:** `ide/project-search-20261105`  
**Target:** standalone, human-first Lu IDE. **Status:** source implemented, PR CI and installed Windows GUI acceptance open.

## Human workflow

Open a registered project, select **Find in project**, enter literal text, optionally select **Match case**, and choose **Search project**. The results show source path, 1-based line / UTF-16 column, bounded preview, saved SHA-256 revision, and exact UTF-8 byte offsets. Select a hit to open the source file and highlight the matched span.

The IDE compares the file's current saved SHA against the search result before opening it. If the file changed, it refuses to navigate on stale evidence. If the file already has an unsaved editor draft, it opens the tab but **does not apply saved-disk match coordinates to that draft** and explains the difference. The user remains in control of Save; no cross-file Replace All is available.

## Native host boundary

The new `source.findProject` descriptor is a typed **read** operation bound to Tauri `ide_find_in_project`. The native `IdeWorkspaceRegistry::find_in_project` obtains a scoped directory capability for the already-registered and active project. It does not accept an arbitrary absolute directory, follow symlinks, execute code, use Git, start a daemon, contact a network endpoint, or write files.

Limits (explicitly returned as `truncated` or `skippedFiles`):

| Guard | Limit |
| --- | --- |
| Search term | Nonempty literal UTF-8 text, up to 128 bytes; no control characters |
| Results | 240 matches |
| File count | 320 read attempts |
| Directory entries | 3,000 total, 400 per folder |
| Directory depth | 12 |
| File size | 256 KiB per file |
| Total scanned bytes | 16 MiB |
| Match preview | Bounded around the match, not the complete source file |

Generated/large trees such as `.git`, `node_modules`, `target`, `build`, `dist`, and virtual environments are excluded; unreadable, binary, non-UTF-8 or oversized files are skipped. **A zero-match result never proves a full project is clean**, especially if limits or skips are reported.

All results are versioned saved-file observations, not references into the volatile unsaved CodeMirror buffers. The search operation is separate from the in-file Find and draft identity work in PR #43.

## Verification gates

Native fixture tests cover exact UTF-8 byte / UTF-16 column offsets across Unicode/CRLF, saved source revision, case sensitivity, symlink escape attempts (Unix), excluded/binary/large files, malformed terms, closed projects and result cap reporting. Frontend contract assertions check command parity, no shell, safe result navigation and unchanged editor write boundaries.

**Pending:** exact-head Rust format/host tests on Linux, Tauri checks on Windows, frontend build/Node tests, and installed Windows UI click-through. No Lu bridge, LSP, worker, package installation, or real user-machine acceptance is implied.
