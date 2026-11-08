# Lu IDE DEV-01.1 — bounded native project Quick Open

**Status:** SOURCE-IMPLEMENTED; exact-head Windows2022/Ubuntu24 Node CI and actual installed Windows/Linux UI evidence OPEN.
**Branch:** `ide/dev01-bounded-quick-open-20261108`, based on standalone P03/P04 closeout `dde157e82fa09376a0262e75bd70387c25f0cad2`. This is deliberately independent from unmerged P05 B2/B3 and is **not** a product/release-candidate merge.

## Delivered

- `Ctrl+P` and a Files-pane Quick Open control open a keyboard-accessible file picker scoped to the **currently selected registered native project**. File search is case-insensitive relative-path matching; exact paths, basename matches and prefixes rank first. Up/Down, Enter and Escape work; no source contents are indexed or disclosed.
- The picker fetches entries **only** through existing `ide_list_source_directory` native commands, with a maximum of 24 visited directories, 500 indexed files, and 30 displayed matches. Native truncated directories or exhausted budgets display an explicit **partial results** notice. It skips symlinks and non-file native entries; source-file preview still uses `ide_preview_source_file`.
- Native result validation refuses cross-project responses, malformed listings, parent-directory mismatches, absolute/traversal/control-character paths; changed project or closed dialog increments the request token and invalidates late results. No direct browser filesystem access, subprocess, permission-bypass, implicit Save, script execution, or opportunistic LSP changes.
- Unit regressions cover nested paths, stable ranking, traversal/foreign project rejection, truncation and directory/file caps, in-flight project cancellation. UI-contract tests prevent silently replacing the true native path with a synthetic tab.
- Dedicated `.github/workflows/ide-dev01-quick-open-native-ci.yml` checks the reviewed npm lock, exact source checkout, full `npm run check` on real Windows2022 and Ubuntu24, and explicitly **does not** label this installed/native-GUI acceptance.

## Explicitly outstanding

1. Inspect final-head CI source results for both OSes; repair any failing real test while preserving security gates.
2. Add **genuine installed Windows and Linux GUI** Ctrl+P keyboard, open-file, incomplete-index, restart, and unsaved-tab retention acceptance to the later integrated candidate's real OS suite. Browser-only React tests do not close this.
3. Complete the rest of DEV-01: safe new file/folder, move/rename/trash, split views, draft recovery and large-repository performance. This scoped quick-open does not claim those finished.
4. Resolve BLK-05 GTK chooser, BLK-06 protected worker/debugger, P06 technical provenance and B2/B3 exact-head merge separately; only integrate accepted sources. Leave signing, legal review and P07/WO10 deferred.

## Acceptance rule

Source implementation is not full IDE feature acceptance. Do not merge this draft to `main` or the standalone release head until the owning task requires and passes its **combined-head** OS and installed-app gates. No historical CI/PR removal and no independent redistribution/legal authority is implied.
