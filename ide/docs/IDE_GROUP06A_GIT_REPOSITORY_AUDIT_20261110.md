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

## Group 6A-01 / 6A-02 source implementation (November 10, 2026)

**Implementation branch:** `ide/group06a-git-repository-audit-20261110` (stacked on Group 5 `2f1509f`).

The new `ide/host/src/git_inventory.rs` is compiled into the standalone Rust host and exposes a **single non-executing** `IdeWorkspaceRegistry::inspect_git_repository_inventory(project_id)` operation. It uses only the host-registered exact project root, refuses repository discovery from the parent directory, and reports:

- **6A-01 branches:** up to 96 local branches with bounded human-readable labels, tip SHA, current branch, stored upstream branch/SHA and local ancestry ahead/behind when independently computable. It labels upstream as *stored*, never live GitHub; absent upstream/ancestry remains unknown.
- **6A-02 linked worktrees:** up to 48 libgit2 administrative worktree names and state labels. It never exports an absolute linked path, opens another linked project, stats an outside root or considers a listed worktree an authorization to edit or switch. Missing/ambiguous admin records are explicitly marked unverified. Worktree lock state is intentionally `not_assessed`.
- **Repository state:** main vs linked repository, attached/detached/unborn HEAD, current commit SHA and partial/truncated metadata. It does not execute Git, run hooks or mutate branch refs.

`ide/app/src-tauri/src/lib.rs` has `ide_read_git_repository_inventory`, an ID-only, read-only Tauri command. `ide/app/web/main.jsx` refreshes the new evidence with existing local Git status and displays bounded branch/worktree cards under Source Control, clearly marking external worktrees unregistered. `ide/app/tests/g06a_git_repository_inventory.test.mjs` enforces the host/command/UI boundary.

Native Rust fixtures cover two local branches, unregistered project denial, a real libgit2-created linked worktree, detached HEAD, an unborn HEAD, and nested selection that must not discover the parent's repository. Their existence is **not** a passing Windows/Linux result until the matching exact SHA has actually compiled and run.

### Remaining implementation and acceptance gaps

1. The source-only 6A inventory is not yet independently compiled and tested on Windows and Linux. Test the exact source SHA, Rustfmt, `git2` API compatibility, Node UI boundary, real Tauri compile, and original registered-root behavior in **one** consolidated native source group.
2. The linked worktree listing deliberately does **not** validate the external directory, show a path, report the external linked HEAD, or classify lock/prunable flags. Those require a separate, explicitly registered and reviewed project identity rather than an ambient external path.
3. A bounded validated remote/tracking ref inventory, per-file blame, repository health states, branch/worktree switching, staging hunks and all mutation workflows remain in later 6A P1/Groups 6B–6F.
4. Windows/Linux installed GUI project picker, real linked-worktree browsing, restart state, accessibility and native security remain unverified. Do not label an unexecuted fixture as acceptance.

This is an independent Group 6A implementation and does not reconcile, promote or interrupt Group 4/5 pending native acceptance.

## 6A-05/06/07 continuation — source implementation and acceptance (Nov 10)

### Source delivered
- `ide/host/src/git_file_history.rs`: bounded HEAD-committed tracked regular-file blame (96 hunks, 512-KiB blob) and 40 first-parent path-change commits with a maximum 2,000-commit walk. Rejects unregistered project IDs, `.git` control paths, traversal, untracked files, symlink blobs and non-literal pathspec matching. Source trust is *local committed Git objects*, not worktree, live remote or signed author proof.
- `ide/host/src/git_refs_health.rs`: up to 120 displayed local, stored remote-tracking and tag refs, capped at 3,000 scanned refs; up to 32 remote **names only**, no remote URLs, config, tokens or network. Reports native repository operation-state, conflicts, shallow/unborn/detached flags. `metadata_clean_only` does **not** authorize Git activity; `operation_ready` remains false.
- New Tauri commands `ide_read_git_file_history` and `ide_read_git_refs_health` are host-only, project-scoped. Source Control shows read-only refs/health and an explicitly requested tracked-file history/blame panel. Async results are checked against project identity and request revision; no additional Git writes were introduced.
- Native fixtures now include local tracked-file history, hostile path denial and a URL-with-fake-credentials remote that must not be exposed; Node source-boundary tests cover Tauri/UI/no Git CLI.
- Project switch or source refresh invalidates old branch/history results and clears loading state. Installed project-switch/restart operator scripts:
  `ide/app/fixtures/Test-G06A-PhysicalWindowsGitInventory.ps1` and
  `ide/app/fixtures/Test-G06A-PhysicalLinuxGitInventory.sh`.
  They require a real interactive installed desktop, collect bounded operator observations and do not claim OS proof, Git immutability or release approval.

### Still open and not conflated
- Exact-head GitHub Action job IDs and Windows/Linux compiler/runtime results **not accessible** via the current connector (it exposes PR runs only, whereas this group is push-triggered). The public Actions page also could not be fetched. No native pass/failure is asserted without real job evidence.
- Native Rustfmt, Rust compiler/API parity, test execution, JavaScript frontend, Windows and Linux Tauri compilations, installed application picker and restart behavior must be verified at a single source SHA.
- No physically installed NSIS/Debian operator test has run in this session; the witness fixtures merely prepare the future acceptance.
- Blame does not follow renames; first-parent path history is explicitly limited. Ref/remote inventory is not live authenticated GitHub access. Conflict remediation, branch/worktree mutations, credential broker, signing, Lu bridge remain later groups.

## November 10 continuation closeout — G06A-05/06/07 source work

**What is now in source:** Native `git_file_history.rs` implements project-scoped committed-file blame and bounded first-parent path history. It limits blob size to 512 KiB, blame response to 96 hunks, 40 path-changing commits and 2,000 traversed commits; rejects symlink blobs and Git wildcard pathspec interpretation. Native `git_refs_health.rs` lists bounded local branch/remote-tracking/tag refs and remote *aliases only*, without URL access, reports merge/rebase/conflict/shallow/detached/unborn metadata, and explicitly sets `operation_ready=false` even when metadata is clean. Both are registered by the host and exposed through read-only project-ID-based Tauri commands. Desktop Source Control now displays refs and health plus on-demand tracked-file blame/history, clearing pending evidence on project switch and refresh.

**New source tests:** A true initialized Git repo test checks blame/history and rejects traversal/untracked paths; a separate initialized repo includes a credential-shaped remote URL and verifies the serialized response does not expose it. Node tests enforce command and UI read-only boundaries. Physical installed Windows and Linux scripts require operator acknowledgement and do not simulate clicks; neither has been executed.

**Acceptance boundaries:** The available GitHub connector supports inspecting known workflow run IDs but does not enumerate push-triggered runs by branch; its commit-run lookup returns PR runs only. The public Actions page was inaccessible. Therefore this session cannot state the old consolidated run's terminal Windows/Linux results, nor yet prove the new SHA's compiler and runtime gates. The Group 6A two-platform native workflow remains the authoritative intended acceptance; maintain Rustfmt, Node, Rust unit and Tauri compilation checks rather than bypass them. The workflow also prepares the previously documented CI-only Windows Tauri ICO solely during headless tests, and performs operator-script **syntax** checks without invoking them.

**Installed acceptance remains open:** actual NSIS and Debian installs, project switching, linked-worktree UI, physical operator history inspection, restart, data preservation, accessibility and rollback/uninstall have no witnessed results in this session. The scripts are a handoff, not proof. Avoid promoting this source to finished product or enabling Git writes.

## Group 6A-07 interrupted Git recovery source branch (Nov 10)

Independent branch `ide/group06a-recovery-health-20261110` extends the read-only `GitRepositoryHealth` response with `GitRecoveryState`. It classifies native git2 repository states for merge, rebase, cherry-pick, revert, bisect, generic interrupted operations, conflicted index and unreadable index. Any abnormal/unknown state requires operator reconciliation. The explicit response flags `can_auto_resume=false`, `can_auto_abort=false`, `write_authority_granted=false`. Even a clean result is metadata only and is **not** permission to run Git.

Added native negative tests for denied auto-recovery in all categories and a fixture writing a real `.git/MERGE_HEAD` sentinel to verify that libgit2 reports a merge state after repository reopen; no rollback/abort/continue is executed. The Source Control panel now displays an accessible recovery warning instead of a misleading clean-authorization indicator. This is source-only pending grouped compiler and physical installed test evidence.

Not implemented: original multi-stage merge parent/reflog snapshot provenance, verified kernel/process custody for a Git subprocess (none is launched), a one-use human-authorized and durable recover/abort controller, protected index transaction, or full 6D conflict resolution. No unreviewed mutation is permitted.
