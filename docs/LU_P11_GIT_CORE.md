# P11 GIT-CORE-01 — Minimal Git observation through Lu's AI execution layer

**Started:** 2026-11-07
**Branch:** `lu/p11-git-core-boundary-20261107`, stacked on P10.
**Policy:** [AI-first native tools and external app use](LU_AI_TOOL_DESIGN_PRINCIPLES_20261107.md).

## Owner intent
Lu's AI models already know Git concepts and commands. Lu Core should not become a Git application or teach Git workflows through hardcoded domain-specific logic. It should make an approved Git program available under **limited task capability and host authority**, returning observable read-only results to the model. More complex coding workflows and GitHub account/network permissions are Lu IDE concerns. An installed Git executable, the model's familiarity with Git, or OS permission does not confer authority to execute an arbitrary command.

## Audited existing implementation
`src-tauri/lu-core/src/native_git_inspect.rs` already provides:
- `lu_git_status`: bounded porcelain workspace status (max 100 entries);
- `lu_git_log`: bounded recent commit details (max 20);
- `lu_git_diff`: bounded selected tracked-file HEAD diff (max 64 KiB).
All use a trusted host-selected repository path and Git executable, narrow typed Protobuf inputs, explicit per-turn ReadWorkspace authority, fixed argument vectors without shell, cleared environment, timeouts and byte caps; model-chosen executable, flags, arbitrary refs, writes and network are not offered. A repository without a normal real `.git` folder is refused.

Existing read-only capability does **not** make the host a universal Git operator: `add`, `commit`, `reset`, `clean`, `push`, `fetch`, branch and credential operations are outside P11. GitHub/gh workflows are IDE-specific and separately authorized. Git output and repository content are untrusted source data; they cannot issue grants.

## P11 bounded closeout work
1. Add adversarial native regression with a repo local `core.fsmonitor`, external diff command and `.gitattributes`; the host's Git status and diff must remain usable without invoking local helper programs or creating index locks.
2. Run exact-head Windows/Linux native test matrix using approved Git binaries and real disposable repositories. Preserve existing wrong-turn/denied/revoked grant tests, path traversal and unknown Protobuf refusal.
3. Keep P11 restricted to Git observation and safety; do not add a Git GUI, generic arbitrary Git command execution, or duplicate Lu IDE features.

## Unresolved release gates
- Exact P11 branch CI result on native Windows and Linux, including helper non-execution regression.
- Real installed Windows and Linux Lu UI: approve/revoke repository read for a persisted user turn, inspect a disposable repository, deny access and verify source/repo identity on restart. Do not infer this from native registry tests alone.
- Process-tree containment for descendant Git helpers, unexpected malicious repository metadata, symlinks/reparse paths and time-of-check/time-of-use filesystem races need real OS adversarial acceptance. A plain subprocess `kill` is not evidence of killing every descendant.
- Model-assisted Git task quality, discovery of installed software, or future general execution layer needs separate end-to-end evaluation, **not** a new P11 micro-app.

**Status:** narrow source hardening and CI submitted; native exact-head and installed acceptance not presumed successful.
