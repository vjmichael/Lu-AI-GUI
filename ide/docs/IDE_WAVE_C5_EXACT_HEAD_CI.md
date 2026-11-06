# Lu IDE Wave C5 — Exact-head public CI receipt observation

**Date:** 2026-11-05 · **Development branch:** `ide/wave-c5-exact-ci-evidence-20261105`. This is a source implementation stacked on C4 PR #48, itself dependent on C3 PR #47. Latest-head CI and installed Windows acceptance are separate, open gates.

## Objective
Associate genuine public GitHub Actions metadata with a specific locally observed Git HEAD without claiming that the IDE itself ran code or that an installer works. C5 is read-only and human-triggered, not a build worker.

## Native operation
`ide_read_exact_head_ci` is the typed `git.ciExactHead` host operation. It:
1. Requires an actively registered local Git worktree whose canonical root exactly matches the selected project; validates the 40-character HEAD SHA. It refuses to follow parent repos.
2. Resolves the locally configured `origin` through the already-constrained `github_live` parser: only canonical public github.com HTTPS/SSH Git URLs, safe owner and repo components. It makes an explicit bounded no-redirect HTTPS request to **fixed** api.github.com for workflow runs with `head_sha=<local sha>`.
3. Considers only the expected `ide-standalone.yml` and `ide-continuity.yml` workflow paths and latest matching run IDs. It separately queries bounded required job metadata. The mandatory standalone job names are `host-linux`, `frontend`, and `desktop-windows`; continuity requires `continuity`. All mandatory jobs must be present exactly once, completed successfully, and not identify a different head when SHA is present.
4. Reports a commit CI pass only if all expected workflow runs and jobs are successful, the listing is complete, no API response is unavailable, and local HEAD did not change during inspection. Pending, failed, missing, truncated, ambiguous, rate-limited/private, and not-observed states remain distinct.
5. Returns a time-stamped, bounded, typed read-only receipt to the human Source control view. No job is started, re-run, or cancelled.

## Evidence scope
A successful workflow proves **only** that those listed GitHub Actions jobs reported success for that exact commit. It does not prove the current uncommitted worktree, unsaved CodeMirror drafts, a user-installed Windows executable, installer signature, completed task/PR merge, benchmark comparability, or usable tools. The view always states installed application and task acceptance are `not_tested`/`not_verified`. No proof is generated for private GitHub repos without public API access.

The C4 Development Evidence reconciliation accepts an observed receipt only for the same selected project and local SHA, with a completed check timestamp and no local-head change flag. The C3 Copy development checkpoint includes only a short sanitized copy of this **cached explicit observation**, never performs GitHub access itself and never promotes task/installation state.

## Security and limits
No OAuth/token/password, arbitrary URLs, redirect following, file uploads, Git CLI, Git fetch, remote modification, Rust worker execution, dynamic model dispatch, always-on monitor, or Lu IPC bridge. Requests are capped (runs up to 30, jobs 80, response sizes 512 KiB each and short timeouts). If the listing exceeds the cap, no success verdict is allowed. GitHub metadata is time-sensitive and can later be invalidated by a new commit/CI attempt.

## Validation
Pure Rust fixture tests cover exact workflow path and commit identity; all mandatory jobs required; duplicate/missing/wrong-head jobs never accepted. Node tests cover stale receipt/project isolation, no automatic network query on checkpoint Copy, typed schema and UI wiring. Native cargo test, cargo fmt, Windows Tauri, and frontend checks must pass on exact branch head, followed separately by real installed Windows UI acceptance. None is claimed solely from the commit.
