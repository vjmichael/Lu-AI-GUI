# Lu PLAN-01 — selected project discovery and requirements baseline
**2026-11-10.** Source implementation on `lu/plan01-project-baseline-20261110` stacked on current Lu desktop PR #205. This is **Lu Core/desktop only**. No IDE, NVP or other product code is modified or developed.

## Intent and bounded native route
The only project root comes from an explicit Settings → Tools & permissions → **Inspect project folder** native OS directory picker. The host accepts neither a model-supplied root nor a WebView path field. It runs a bounded, local, read-only CPU inventory without starting GGUF inference, invoking arbitrary shell programs, making HTTP requests, modifying Git state, writing tasks, or granting execution authority.

`project_discovery_desktop.rs` traverses a capability-scoped `cap_std::fs::Dir`; symlink/reparse traversal is refused. The scanner ignores heavyweight build/dependency directories; limits traversal to 256 directories, 1,600 files, depth six and five seconds, at most 36 textual documents, and bounded bytes. Results explicitly flag incomplete inventories, unavailable local Git evidence and unqueried remote issues/PRs/CI. A search miss is **never** proof that a feature does not exist.

It lists candidate source paths, test paths, workflow paths, project documents and issue/PR templates. It hashes the **actual bytes opened** for project documents. It records source path, line, digest and tentative mandatory/deferred/out-of-scope/undecided requirement leads. These leads are **reported, untrusted document text**, not acceptance, status findings, or executable instructions. Checkboxes cannot certify implementation.

Local Git identity reuses `native_git_inspect::ApprovedGit`: host-selected installed Git, fixed read-only argv, sanitized environment, no fetch/hooks/pager, bounded output and timeout. The returned branch, exact HEAD, tracked changed/untracked counts, and dirty state are a local observation, not proof of remote PR state. Nonstandard linked Git worktrees are rejected rather than silently probing outside the approved folder. Software is the initial classification; the typed baseline permits generic future non-software projects.

## User interface and tests
The separate native Settings surface displays local project root, observed UTC epoch timestamp, exact local Git commit/branch/status when available, bounded file and document counts, source/hashes, requirement leads and limitations. No project task scheduler, verdict engine or model planner is enabled.

PLAN-01 Rust fixtures cover local docs, four disposition kinds, SHA-256/line provenance, no-remote claims, code-fence exclusion, symlink escape refusal and rejection of a non-folder. Native Git parser fixtures reject malformed/hostile branch ref strings. Existing Node host/UI structural contracts additionally verify native picker, host-scoped read-only API, exact bounded constants, refusal to accept renderer-provided paths and no model/network spawn.

## Acceptance distinction
This commit is a **source candidate** until the exact-head Windows/Linux native Rust and frontend tests finish, and a real installed Settings picker/inventory is observed. PLAN-02 evidence graph, PLAN-03 acceptance classifier, PLAN-04 genuine-GGUF task synthesis, PLAN-05–07 task register/UI/refresh, optional remote GitHub metadata, and physical-owner release testing are **not** claimed complete.
