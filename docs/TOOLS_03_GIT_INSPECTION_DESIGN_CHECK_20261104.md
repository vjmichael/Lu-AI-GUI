# TOOLS-03 — Git inspection: original tool-design cross-check

**Date:** 2026-11-04
**Status:** Development-source implementation and native CI fixture, not installed GUI acceptance
**Branch:** `lu/tools-03-git-inspection-20261104`, based on frozen TOOLS-02 MBRL `383a30fa`.
**No Git mutation or general shell execution.**

## Requirement hierarchy: why Git belongs in Lu v1.0

1. **Historical design source:** Library `Jan-Mod-Design-Document.md`, §3.9 treats Git, linter, debugger and test runner as Code workspace *additional tools*, not the base common-tool registry. §5.2 proposes **unified native implementations** with **small model-facing task-specific JSON schemas** that map into validated Protobuf rather than presenting a single complicated model JSON schema. §4.6 / §5.1 keeps native IPC and effect authority distinct from model output.
2. **Current owner-approved canonical design:** `docs/LU_CENTRAL_DESIGN.md` §§1 and 3.3 explicitly include **first-party Git and archive tools in the Lu v1.0 native core**. The Code/IDE *workspace* and dedicated development interface remain DLCs after v1.0. `docs/LU_V1_0_DESIGN_RECONCILIATION_20261103.md` (V1-06) similarly names Git; `docs/LU_V1_0_TASKS.md` specifically lists TOOLS-03 Git status/log/diff. The current design supersedes the old workspace-only scheduling for a minimal common Git capability, not for an IDE GUI.
3. **No scope confusion:** Unlike the JSON boundary validator (strictly internal), these are genuine *read-only native tools* that Lu may advertise to the model **only when the trusted host has selected the Git repository and opened an authenticated per-task `ReadWorkspace` authority**. Tool registration/discovery does NOT authorize invocation. The user does not invoke the Git binary manually, and Git itself is not the user's GUI; Lu is not building a developer DLC here.

## Exact contract and implemented surface

| Area | Implementation | Design reason |
|---|---|---|
| Internal native registry | `lu.git.status`, `lu.git.log`, `lu.git.diff` share a host-selected repository/executable, effect `ReadWorkspace` | First-party native tools; no MCP |
| Model-facing interface | `lu_git_status(max_entries?)`, `lu_git_log(limit?)`, `lu_git_diff(path?,max_bytes?)` | Narrow, simple model-visible schemas per original §5.2 |
| Internal transport | `GitStatusRequest`, `GitLogRequest`, `GitDiffRequest` and matching typed Protobuf replies | No untyped shell command JSON |
| Repository selection | Trusted host supplies canonical root with normal `.git` directory; the model never supplies repo path or Git executable | Model data is not permission |
| Operations | Read working tree status, bounded recent commit summaries, tracked diff vs `HEAD`; no stage/add/commit/reset/checkout/fetch/push/clone | Minimal v1.0 general-purpose Git |
| Execution | Fixed Git subcommands and arguments via Rust `Command`, **no shell**; cleared environment; no pager, user hooks, external diff or textconv; bounded output and timeout | Git binary is a trusted, host-selected program, never an arbitrary command runner |
| Authorization | Registered tool descriptors have `ReadWorkspace`; `NativeToolTurn` and independent host authorizer must admit each call | Git schema is never authority |
| Tests | Disposable actual Git repo, valid status/log/diff, no changed HEAD/index/working data, bad JSON/paths, Protobuf spoofing and permission denial/revocation | Real effects and failed effects must be distinguishable |

## Explicit exclusions and release gates

- **Not** the Code/IDE workspace, terminal, repository manager UI, autonomous pull-request writer, test runner or Lu self-updater.
- No generic Git command, arbitrary revision specifier, executable path, `-c` option, configuration/environment argument, host filesystem path or network operation from the model.
- No automatic assignment to a user or chat. The eventual installed desktop must offer trusted repository selection, tool approval, reporting and revocation under the existing `AUTH-01` model. The isolated Core tests do not constitute GUI integration.
- Git status/log/diff reveals potentially sensitive repository text and must be treated as untrusted *data* on return. Model instructions embedded in a diff cannot create permission.
- Windows/Linux Git must be installed or host-provisioned and selected independently; no forced shell workflow and no bundled unverified binary.
- **Acceptance:** exact source lock, Linux/Windows genuine Git fixture CI, tested Protobuf tool dispatch, denial/revocation, Windows installed-app final acceptance near v1.0. **Do not claim complete v1.0 native tool integration from Core tests.**

## Comparison conclusion

Implementing **read-only Git inspection in Lu Core** is consistent with the *current* v1.0 design, but its **small model-facing API, typed Protobuf, host-selected scope, effect authorization and absent generic shell** are required by the original implementation design. Earlier Git-on-Code-workspace language is retained for provenance; richer IDE Git tools remain DLC-only.
