# B3 — Find in Project integrated into the current Lu IDE stack

**Status:** Source port implemented, exact-head Windows/Linux CI and installed desktop acceptance pending. The original independent B3 PR #46 is preserved for source provenance.

## Implementation

The active IDE now has a **Find in project** navigation entry for saved UTF-8 files in the currently opened OS-picked native project. Searches are literal, with an optional Match case checkbox. Results include registered relative paths, saved SHA-256 revisions, 1-based lines/UTF-16 columns, UTF-8 match byte ranges and explicit skipped/truncated counts.

The original PR #46 host module and regression tests (original commit a94c5d6af266f9a0940b4b9d03ce9d7671d98457) have been carried forward. Native source reads are exclusively through the registered active cap-std directory capability. The new Tauri command performs a bounded host operation without invoking arbitrary processes or network access. The existing 41 capabilities are retained and a 42nd read-only source.findProject capability is included with typed result definitions.

The current React/CodeMirror UI verifies the exact saved-file SHA before navigation and preserves unsaved editor drafts. A dirty existing tab is never overwritten or navigated using saved-source coordinates. Replies from a stale project or superseded search are discarded. No Replace All or implicit Save is introduced.

## Limits and exclusions

- Query: 1–128 UTF-8 bytes and no control characters.
- Matches: 240, explicit truncated flag.
- Files: up to 320 scanned; 3,000 entries total and 400 per directory.
- Directory depth: 12; file bytes: at most 256 KiB per file and 16 MiB total.
- Excludes symlinks, generated/build folders, binary/invalid text, unreadable and oversized files.
- A partial scan or zero hits is not proof that a project has no matching content.

## Acceptance

1. Native Windows and Linux host search tests: Unicode/CRLF offsets, revision, symlinks, generated/binary/large files, closed project, malformed queries, caps.
2. Exact-commit native frontend npm check, typed capability parity, Tauri command compilation with unchanged source dependency locks.
3. Genuine installed NSIS/GTK Debian UI acceptance: selected project, bounded search, exact hit click, dirty-draft protection, disk revision race, reload and uninstall.
4. Original B2 PR #43, its installed Windows PR #45 and original B3 PR #46 remain separately open. No merge or closure before independent source/CI review.

There is no change to Lu Core, no unrestricted worker launch, no new release approval, no automatic save, and no signed installer claim.
