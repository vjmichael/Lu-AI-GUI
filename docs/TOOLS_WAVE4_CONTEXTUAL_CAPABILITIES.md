# Lu Wave 4 — contextual tools for ordinary conversations

Status: Wave 4 started; **WAVE4-GIT-01 source-implemented**, native and installed acceptance pending.
Branch: `lu/wave4-contextual-tools-20261106`, stacked on Wave 3 PR #57.
This wave **does not supersede** the owner-approved original Jan tool design or current Lu v1.0 constraints.

## Wave boundaries

- Wave 3: native Reader/Workspace, read-only discovery, user-reviewed file effects, progress, cancellation, audit, tool-loop guards — source integrated, exact-head CI and installed Windows GGUF acceptance remain separate.
- Wave 4: complete **Browser, Research, Background, Environment, Git, Vision and Image Workspace** integration into Lu's ordinary human-first conversation. Reuse existing host protocols/modules wherever possible; never create a parallel agent engine.
- Wave 5: native acceptance and hardening (Windows/Linux CI, actual GGUF+MMProj, permission/cancellation/recovery, installed desktop/NSIS). Earlier verification is welcome and required for merging; Wave 5 is not permission to skip present native regressions.

## WAVE4-GIT-01 — scoped local Git inspection from normal Send

- Human asks directly to see `git status`, `git log`, `git diff` or local repository history. The trusted host uses a conservative *current-human-message* classifier; creation, mutation, fetch/push, general shell, ambiguous explanation, negation, pasted attachments and model-control tokens do not route to Git.
- The native OS folder dialog selects one real local repository; Lu rejects symbolic-link/reparse folders and repositories with unsafe Git metadata. A *host-selected fixed-location* installed Git executable is required: Windows Git for Windows in Program Files; Linux `/usr/bin/git` or `/bin/git`. No PATH, model-provided command/executable, repo-config executable or network fetch is admitted.
- `WorkspaceReadTurnSession::new_git` **reuses the existing authenticated per-turn authority, supervisor, typed Protobuf catalog, tool-loop guard, lifecycle revocation and cancellation**. Its registry includes exactly `lu_git_status`, `lu_git_log`, `lu_git_diff` with `ReadWorkspace` effect and bounds. The ordinary Workspace mode retains its previous file-oriented catalog; Git is not globally advertised.
- The Git adapter reuses `native_git_inspect`'s allowlisted arguments, fixed Git process environment, local repo hardening, output caps and process timeout. Git commit messages and diffs are untrusted content. No model write grant, credential exposure or GitHub CLI access.
- The host tracks per-generation progress, writes a separate `lu-git-read-audit.sqlite` model/tool audit and includes content-free Git task summaries in ordinary chat's **Tool activity** pane. A selected repository is not persisted as future authority. Cancel and shutdown revoke task access and no remote side effect is implied.
- Tests: real local Git fixture, schema whitelist, wrong-chat/user-turn denial, and deterministic intent rejection. Exact-head Linux/Windows native and installed Windows+GGUF acceptance **not yet passed**.

## Next Wave 4 integrations

Background: reuse existing `TaskStore`, `BackgroundStatusTool`, terminal notices and scoped dashboard; do not permit model-selected processes or unverified artifact claims.

Environment: host-attested tool locations/versions/fingerprints and usable resource inventory. Separate recalled state from freshly verified facts.

Vision/Image Workspace: route to genuinely **verified** native vision capability and user-approved image scope; never substitute OCR or hallucinated descriptions for unsupported native vision.

Browser/Research: preserve human-visible controls, explicit navigation/authorization, untrusted page content, and structured evidence; the current repository does **not** contain a complete native browser engine, so do not mark these implemented or invent one.

Advanced GitHub operations belong to Lu IDE DLC, not the Core Git domain.

## WAVE4-BACKGROUND-01 — source integration (native acceptance pending)

Explicit human status questions in normal Send reuse the existing BackgroundProcess reconciliation and durable TaskStore. A new chat-scoped SQL query filters before its limit; only host-owned task IDs, state enums and recorded percentages enter a marked GGUF context snapshot. No worker-authored text, private paths, other chat data, scheduler, new execution tool, process cancellation or authority is introduced. Native and desktop tests were added. Installed Windows GGUF and Linux/Windows CI remain pending.

Browser, Research, Environment, Vision and Image Workspace remain distinct open Wave 4 domains.
