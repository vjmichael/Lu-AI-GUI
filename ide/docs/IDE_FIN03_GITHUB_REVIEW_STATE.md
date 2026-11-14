# FIN-03A — Same-origin public GitHub reviews

Implemented a native read-only endpoint requiring the registered project's
exact public GitHub origin and an open PR on the **same local HEAD**. Only
the PR number can be submitted from WebView. A bounded unauthenticated
request with no redirects inspects latest reviewer decisions, distinguishes
stale commit reviews and dismissals, and refuses positive approval claims
when pagination is incomplete. A second native PR observation checks that
origin, branch and SHA have not drifted. The GUI displays the counts
without interpreting them as branch-policy or merge approval.

FIN-03 remains **open**: 6E-03 remote branch/branch-protection evidence,
6E-04 full check/artifact receipt binding, 6E-05 carefully authorized
remote publication, and typed local revert/stash/unstage/worktree
management have not been implemented. Protected remote operations require
a separate native credential and OS consent design; no unsafe plain Git
command or token handling has been introduced.

## FIN-03G — Read-only local revert/stash impact review (2026-11-12)

A new native `git_fin03_operation_preview` module and the Tauri command
`ide_git_preview_local_mutation_impact` provide **preview-only** reviews
for three proposed operations: revert a specified lowercase 40-character
local commit OID, apply an existing stash reflog slot, or drop one existing
stash reflog slot (index 0–31). The IDE source page has an accessible
selector and a read-only result panel. It contains **no Apply, Drop or
Revert action**, authority token, OS grant or mutation executor.

The native review requires an exact registered ordinary worktree with
a direct local branch HEAD; fingerprints the original Git index file;
checks repo state, foreign index lock, dirty entries and conflicts;
binds exact HEAD/selected commit/tree/parent and current stash ref;
constrains diffs to 256 paths and display to 48; and rejects unsafe
path names. Unsupported file modes, potentially overlapping paths,
untracked stash parent, dirty worktree, previous FIN-03 interruptions
and unverified merge/conflict results are explicitly represented as
blockers. Read-only source and selected stash identities are rechecked
before the review is returned. No Git process, ref update, stash API,
checkout, index write, source-file write or network operation occurs
in the implementation.

**Important:** path-level overlap is only a conservative observation.
An absence of overlap is NOT evidence that a revert, stash apply or
stash drop will be safe, replayable or conflict-free. Stash drop is
called out as loss of a saved reference. For revert, overlap measures
edits after the selected original commit, not the commit's own diff.

Native Rust fixtures establish intended assertions that original HEAD,
stash ref, index bytes and source files remain unchanged; Node source
contracts assert absence of execution APIs. **These tests are committed
but not yet executed.** The FIN-01D native kernel recovery acceptance
gate still blocks all new destructive FIN-03 operations. No release
authority is implied by the review SHA-256.
