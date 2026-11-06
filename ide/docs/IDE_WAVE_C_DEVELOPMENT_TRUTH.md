# IDE Wave C — Development Truth: Git evidence and resource verification

**Date:** 2026-11-05  
**Branch:** `ide/wave-c-development-truth-20261105`  
**Status:** Native/React source implementation; exact-head CI and physical Windows acceptance pending.

## Purpose

Make the IDE's human-facing view of development state more trustworthy without granting execution or repository-mutation powers. Wave B editor work may continue through its own PRs independently; Wave C's scope is read-only Git, resource verification, and provenance-aware continuity.

The existing A3 implementation already provides local Git dirty/untracked status, stored upstream comparisons, an explicitly triggered public GitHub SHA query, bounded project artifact inventory, and durable artifact snapshots. **This Wave C slice adds missing Git diff/history detail and an exact resource probe**, and reuses the existing Continuity and Source control pages.

## C1 — Read-only Git diff and history

New Rust module `host/src/git_evidence.rs`, Tauri commands `ide_read_git_diff` and `ide_read_git_history`, and typed capabilities `git.diff` / `git.history`:

- Accept only the currently open, host-registered, exact Git worktree root; reject repository discovery above that root.
- **Staged** means HEAD ↔ Git index; **Unstaged** means index ↔ worktree. Untracked-file *contents* are excluded from patch text, while the existing Git status view still identifies those files.
- Bound the displayed patch to 64 KiB and 80 files; report `truncated` explicitly. Patches are untrusted local text, not instructions to execute.
- Return up to 30 local HEAD commits, timestamps, SHAs and bounded commit summaries; local history never proves current GitHub state.
- Never invoke Git CLI, hooks, build scripts, network, fetch/push, stage, merge, checkout or file writes.

The Source control view exposes an explicit **Inspect diff and history** button and an Unstaged/Staged selector. Patches and commit summaries are displayed as text; no controls apply them. The separate **Check GitHub** action retains its own provenance and only compares observed branch SHAs.

## C2 — Targeted resource observation

New Rust module `host/src/resource_probe.rs`, Tauri `ide_probe_project_resource`, typed `resource.probe`, and Continuity **Recheck file bytes** action:

- Only a validated, relative file path within the registered/open project may be probed; no arbitrary absolute path, process-wide PATH search or global disk scan.
- Re-observe file presence, file type, metadata and exact SHA-256 for a regular file up to 64 MiB. Report missing, not-regular, too-large or changed-during-probe distinctly.
- Return the hash of bytes observed, but **always** report executable usability as `not_tested` and last trusted execution as `not_inspected`. A matching hash does not prove NSIS installer, compiler, interpreter or benchmark runs.
- No running tools, invoking package managers, touching credentials, or uploading file contents to GitHub.

The existing baseline/history capability preserves earlier observations. Resource probes are a fresh read and do not assert that the earlier snapshot was immutable or globally complete.

## Provenance and limits

| Evidence | Meaning | Does not prove |
| --- | --- | --- |
| `git.status` | Current local tracked/untracked state observed by libgit2 | GitHub is current |
| `git.diff` | Bounded local staged/unstaged patch | Full patch when truncated, untracked file contents |
| `git.history` | Bounded history reachable from local HEAD | Remote ancestry without remote fetch |
| `git.compareLive` | Time-stamped public GitHub branch SHA check | Private repo access, live working-tree sync |
| `inventory.*` | Known project artifact locations and earlier snapshots | All user tools/resources are found |
| `resource.probe` | Current bytes/metadata at one registered relative path | File executable, safe, signed, installed or working |

There is no activated Lu bridge. Future resource probing outside a registered project, authenticated remote comparison, durable evidence receipts for real execution, and runnable developer toolchains require separate user choices and OS-supervised worker gates.

## Verification gates

Native tests in `host/tests/git_evidence.rs` and `host/tests/resource_probe.rs` cover staged/unstaged diff separation, untracked-content non-disclosure, local HEAD history bounds, refusal of parent repositories, changed/missing SHA, symlink/traversal rejection, oversized resources, and non-executing semantics. Frontend source/schema checks cover the actual native commands and UI displays.

**Source implemented; not yet accepted:** exact-head Linux cargo test/rustfmt, Windows native desktop checks, frontend build/test, installed Windows UI smoke. Any formatting/compiler failures must be fixed in the isolated PR. Treat a successful CI build separately from a verified installed application.
