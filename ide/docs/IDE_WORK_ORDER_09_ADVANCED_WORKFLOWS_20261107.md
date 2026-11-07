# IDE WO9 — Advanced packaging, native Git evidence and multi-project workflows

**Plan count:** 10 total IDE work orders. WO1–WO3 have historical native evidence; WO4–WO8 remain individually subject to open acceptance. WO9 is this phase; WO10 is the **optional, explicitly authorized Lu self-development bridge**. GitHub Actions are evidence, not a blocker for proceeding with new implementation.

**Branch:** `ide/wo9-advanced-workflows-20261107`, stacked on WO8 draft PR #74. **Status:** WO9-A–C implementation committed for native CI; further native installed acceptance, Git mutation and orchestration not accepted. No installed WO9 artifact, no signed installer, no Lu bridge.

## Required functionality

| Suborder | Scope | Current state |
| --- | --- | --- |
| **WO9-A — Multi-project coordination** | In one GUI, select up to eight independently registered/active native projects and inspect each local Git HEAD, worktree, chosen artifact inventory. No absolute paths from WebView or Git subprocess execution. | Implemented source in `host/src/advanced_workflows.rs`, Tauri preview, enabled Packages & Workflows UI |
| **WO9-B — Evidence, plan freshness and manifest** | Stable SHA-256 fingerprint over ordered per-project Git/artifact evidence; stale worktrees, changed artifacts and duplicates block; require one explicit native reinspection before an OS-picker destination can receive a new no-overwrite, fsynced review manifest. | Implemented source and native registered Git fixture tests; native CI pending |
| **WO9-C — Actual portable artifact bundles** | Explicit portable ZIP of *verified* bounded artifacts from registered project capabilities only, deterministic ordering, 128 MiB global/64 MiB per file limits, SHA validation before inclusion and after emission, secure archive-relative names, no overwrite, native destination picker, no project-root writes. | Implemented source and full ZIP-readback test; native CI pending; **not an executable source/installer release** |
| **WO9-D — Human-reviewed Git mutations** | If supported, stage/unstage/commit only explicitly selected, revision-pinned files after a separate human review; never auto-run Git hooks, fetch, push or repo scripts; deny on HEAD/drift/foreign worktrees. | Open; existing Git remains read-only |
| **WO9-E — Durable multi-project dependency workflows** | Explicit ordered dependent jobs, scoped approvals per project and task, dependency failure/quarantine propagation, restart/cancellation/reconciliation, bounded receipts. Reuse protected supervisors rather than introducing a second process launcher. | Open; current plan only observes, no execution |
| **WO9-F — Installer and independent native acceptance** | Real installed Windows/Linux GUI, ZIP + Git drift, no-overwrite and symlink/race negative tests, actual package/hash inspection, licensing/release gating and truthful OS receipts. | Open; native source CI not full installed acceptance |

## What is implemented and its security boundary

`IdeWorkspaceRegistry::inspect_multi_project_workflow` accepts **only catalog IDs**, not paths. It requires each project to be open and available. `inspect_git_worktree` uses existing read-only libgit2 and rejects dirty/truncated/unverified HEAD; `inspect_project_evidence` uses the existing bounded `cap_std` inventory. Unknown, unhashed, over-limit or truncated files block review. This design **does not** claim Git HEAD protects ignored files: only discovered and independently hashed artifact bytes are included.

`verify_multi_project_workflow` always re-inspects native state and rejects SHA drift. `export_workflow_manifest` and `export_workflow_bundle` only take a **native OS-selected destination**, deny symlink/reparse destination roots, deny writing into registered project roots, and refuse overwriting existing exports. ZIP entries contain `projects/<registered-id>/<verified-relative-path>` with no arbitrary traversal. The ZIP includes a machine-readable `workflow-manifest.json` with `readyForRelease=false` and records observed metadata. The bundle's portable ZIP, although containing byte data, does **not** execute code, stage/commit Git, sign an installer or create OS-worker authority.

The WebView can invoke only named preflight, fresh verification and native export operations. The native host never accepts arbitrary executable names, commands, selected file paths, package scripts, or Git operations through these WO9 requests. Users must explicitly choose projects, review native evidence, reverify, and select an OS export folder. A matching review fingerprint never constitutes approval to ship.

## Verification and remaining actions

- `.github/workflows/ide-wo9-advanced-workflows.yml`: Windows and Linux Rust native libgit2+filesystem tests, real ZIP readback, stale Git/artifact/drift and no-overwrite negative gates, plus React UI `npm run check`. Exact PR head must pass before source-complete acceptance.
- Follow-on: genuinely packaged installed UI, Git staging/commit workflow, durable dependent jobs, native cancellation, Windows/Linux release acceptance, and separate WO4–WO8 prerequisites.
- Keep WO9 PR in **draft** while these are open. No claims based on queued CI, preflight or unsigned development packages.
