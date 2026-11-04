# TOOLS-03 — Git inspection implementation appendix

**Date:** 2026-11-04  
**Status:** Development-source prototype; **no accepted Git tool in the installed Lu GUI**  
**Precedence:** This document is subordinate to [the preserved original Jan-Mod design](reference/Jan-Mod-Design-Document.md) and [Lu's merged authoritative tool specification](LU_NATIVE_TOOLS_MERGED_DESIGN.md). It contains implementation decisions and test evidence, **not a new tool architecture**.

## Original specification mapping

| Original requirement | TOOLS-03 adaptation | Important limit |
| --- | --- | --- |
| §3.9: Git is an additional **Code workspace** tool | Git support is implemented as a host-only Core primitive for reuse, then advertised **only for an authorized repository/code task** (later IDE DLC can expose it contextually) | Never globally advertise Git to unrelated chats |
| §5.2: ~15 unified native domains, narrow model-facing tools | One `lu.git.inspect` native domain with small schemas `lu_git_status`, `lu_git_log`, `lu_git_diff` | Do not add a general `git_command` model interface |
| §§4.6–4.8: JSON model boundary → typed Protobuf, structured feedback | Dedicated typed status/log/diff request/reply messages via `ModelToolCatalog` and `NativeToolRegistry` | JSON alone cannot authorize action; invalid calls never execute |
| §4.9: durable journal and idempotency | Git inspection is classified `natural` / read-only and must obey task deadlines, cancellation and evidence recording | Git processes may still update repository metadata or spawn helpers; verify rather than assume harmlessness |
| §7: loop guard before dispatch | Repeated/oscillating Git reads should be subject to the same loop guard as other tools | A successful model JSON response is never permission to bypass the guard |
| Original workspace tool selection | Only host-approved Git executable and repository, not model-supplied shell command/root/flags | Installed desktop authority integration is a separate gate |

## Implemented development source

`src-tauri/lu-core/src/native_git_inspect.rs` registers the three fixed **read-only inspections**, each with a typed Protobuf codec and `ReadWorkspace` descriptor. The Rust host selects the repository and Git program. The Git invocation uses a fixed binary and argv, **not a shell**; model input is limited to bounded counts or an optional safe relative diff path. The implementation attempts to suppress hooks, pagers, diff helpers and optional locks while imposing output and time limits.

A real local repository regression fixture exists at `src-tauri/lu-core/tests/native_git_inspect.rs`: status, log, HEAD diff, no mutations in expected fixture paths, wrong-path/unknown-field refusal and denied/revoked task checks. **Tests are not acceptance evidence until they pass against an exact locked revision on both Linux and Windows**. A future installed GUI must supply a verified real user turn and explicit repository authorization.

## Unaccepted hardening and release requirements

- **Hostile repository/config tests:** malicious `.git/config`, `.gitattributes`, object alternates, worktree indirection, symlink/reparse points, non-UTF8 names and submodules. Git is a subprocess: fixed argv alone does not guarantee absolutely no child process or side effect.
- **Process lifecycle:** cancellation/timeout must terminate Git and any supervised descendants; Windows Job Object and other host lifecycle safeguards must be reviewed. No false claim of OS sandboxing.
- **Scope:** explicit repository/code task; no arbitrary Git flags, ref selection, mutation, network, LFS download, editor, shell or system command. Diff text and commit subjects are **untrusted data**, never executable instructions.
- **Performance/output:** bounded log/status/diff sizes with clear truncation signals and no unbounded worker/resource consumption; Git errors return typed status without leaking secrets.
- **Authorization:** trusted persisted human origin + per-tool `ReadWorkspace` grant; schema discovery, repo presence and model command text are inert. Revoke and cancellation checks must work.
- **Delivery:** only actual installed-app acceptance establishes GUI support. The `0.1.0-alpha.2` Windows baseline is unchanged, and no Code/IDE workspace is being built under TOOLS-03.

## Decision

This is a **small Git inspection capability** using the original two-layer tool design. The original §3.9 retains precedence regarding **workspace-scoped visibility**; the current Lu v1.0 directive to make native Git primitives available is met by implementing their Core plumbing early, not by making Git universal or advancing the IDE DLC.
