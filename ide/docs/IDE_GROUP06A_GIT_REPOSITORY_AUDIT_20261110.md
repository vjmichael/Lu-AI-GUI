# IDE Group 6A — Git repository management source audit (2026-11-10)

**Branch:** `ide/group06a-git-repository-audit-20261110`.
**Source audited:** `2f1509f641d0f526c8185601057940b449a66605` (Group 5 source head).
**Scope:** audit and prioritized gap classification ONLY. No Git API, worker permission, UI action, repo mutation or native acceptance was added. Group 4/5 acceptance branches are preserved.

## Confirmed in-source capabilities — retain; do not rebuild

| Capability | Source | Precise behavior / limitation |
| --- | --- | --- |
| Registered-project repository status | `ide/host/src/git_status.rs` | libgit2 reads exact registered root, branch name, HEAD SHA, staged/worktree/untracked counts, up to 120 path summaries; explicit local stored-tracking ahead/behind if available. No subprocess, Git hook or live fetch. |
| Local diff | `ide/host/src/git_evidence.rs` | Staged HEAD/index or unstaged index/worktree patch; 64 KiB/80-file bound; excludes untracked file *contents*. |
| Local history | `ide/host/src/git_evidence.rs` | Up to 30 commits reachable from current HEAD, timestamp and summary. Not a global branch/reflog explorer. |
| Public GitHub comparison | `ide/host/src/github_live.rs` | Explicit fixed-host, no-credential, read-only branch SHA check against local and stored tracking state; cannot fetch objects, prove ancestry of different SHAs, access private remotes or mutate remote. |
| Exact-head Actions evidence | `ide/host/src/github_ci.rs` | Bounded explicit public GitHub workflow/job checks, exact SHA/path/job verification. Not general GitHub PR/review UI. |
| Reviewed one-file staging and commit | `ide/host/src/reviewed_git.rs`; `ide/app/src-tauri/src/lib.rs` | Local already-tracked file only, pinned source/index/HEAD, human OS dialog, libgit2 blob/commit, no hooks/fetch/push; rejects unrelated pre-staged changes, conflicts and detached HEAD. This is narrower than Group 6B advanced staging. |
| Multi-project Git snapshot | `ide/host/src/advanced_workflows.rs` | Read-only registered project HEAD/artifact fingerprints and reviewed exports, not branch/worktree management. |
| Human UI | `ide/app/web/main.jsx` | Source control panel invokes status/diff/history and one-file review/stage/commit; separate GitHub comparison. Does not imply installed-app acceptance. |

## Genuine Group 6A missing capabilities

**P0 — Repository identity / read-only baseline (next implementation):**

1. **Branch inventory:** no typed native list of bounded local branch names, tip SHA, upstream, checked-out state, detached/unborn state, ahead/behind and uncertainty flags. Existing status covers only the selected current branch.
2. **Linked worktree inventory:** no read-only listing of worktrees, absolute-root identity, main/linked/bare state, branch/head, locked/prunable condition, or external/missing worktree warnings. Existing `Repository::open` followed by exact-root check protects against parent discovery but does not enumerate other linked worktrees.
3. **Repository validation and lifecycle states:** detached HEAD is observed in status but denied for write; bare repositories and parent-discovered repositories are intentionally unsupported. A new read-only 6A API must explicitly classify these conditions without resolving arbitrary user-supplied filesystem paths or granting writes.
4. **Tests and installed verification:** no dedicated 6A branch/worktree inventory fixture acceptance. Add native libgit2 fixtures covering normal, linked, detached, unborn, renamed/deleted branch, stale tracking, missing/broken linked worktree, symlinked or outside-root path, huge branch counts and malicious ref names. Windows and Linux exact-head tests plus installed UI acceptance remain outstanding.

**P1 — Repository navigation:**

5. **Blame and history by file/ref:** no host-exposed bounded blame/path history; current history is 30 HEAD commits only.
6. **Refs and remotes inspection:** no typed read-only inventory of local/remote-tracking refs, tags, remotes, default/HEAD symbolic ref, stale tracking, duplicate/ref ambiguity or safe redaction of credential-bearing remote URLs.
7. **Repository health:** no dedicated conflict/unmerged index summary, ongoing merge/rebase/cherry-pick state, sparse-checkout/submodule/large-file metadata or safe repair guidance. Existing status indicates some conflicts but not their lifecycle.
8. **UI parity:** no branch/worktree explorer, selection controls, contextual local-only/provenance badges, truncated result continuation or accessible keyboard navigation for the new resource types.

## Explicitly later, not Group 6A

- **6B:** multi-file/hunk staging, amend and advanced reviewed commits; preserve existing one-file contract.
- **6C:** branch create/switch/delete, worktree create/prune, stash, merge/rebase/cherry-pick/revert.
- **6D:** conflict resolution and durable interrupted-operation recovery.
- **6E:** authenticated GitHub PR/review/remote operations and CI beyond existing public read-only observations.
- **6F:** credential broker, one-use authorization, branch protection and destructive action approvals.
- **6G:** cross-OS native, installed-app and crash/restart acceptance.

## Security/design constraints for first 6A source slice

- Read-only typed host operation using `IdeWorkspaceRegistry` and libgit2. The WebView passes only an already registered `projectId`, never a path, command or URL.
- Bound lists, bytes, time and memory; report truncation, unknown state and stale snapshots as uncertainty rather than truth. Avoid disclosing unrelated absolute paths or remote credentials in UI.
- Preserve exact registered root identity. A linked worktree may be listed as data, never automatically registered, opened or authorized.
- No Git CLI, subprocess, hook, fetch, network request, checkout, branch mutation, filesystem scan outside authorized scope, ambient credentials or Lu Core bridge.
- Distinguish local tracking SHAs from live remote values; an absent upstream or unknown ancestry must not be shown as equal.
- New code only after this audit; native tests and installed results must be independently executed before acceptance.

**Recommended next implementation:** 6A-P0 read-only branch + linked-worktree inventory, source-only with hostile fixtures, then one consolidated Windows/Linux native gate. Do not wait for Group 4/5 GitHub Ops to begin source development.
