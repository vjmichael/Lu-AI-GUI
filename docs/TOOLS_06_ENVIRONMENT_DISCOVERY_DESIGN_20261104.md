# TOOLS-06 — Environment and approved-program discovery

**Decision:** 2026-11-04. **Status:** Lu Core development source, not a shipped GUI tool.  
**Primary source:** [Original Jan-Mod-Design-Document.md](reference/Jan-Mod-Design-Document.md) **§6A.1–6A.6 and §5.2**; [merged native tool specification](LU_NATIVE_TOOLS_MERGED_DESIGN.md). This is a subordinate implementation review, not another independent tool architecture.

## First resolve the original intent

The original design §6A **explicitly says** the model should be **told** about the OS, installed languages, selected model, current authorized tools and time through **ambient host-injected context**, updated when those facts change. It **should not call a tool repeatedly** merely to find the operating system or list installed software.

Original §5.2 **also** includes `check_env` in the internal unified `exec` domain, with a task-specific model facade for *exceptional on-demand environment checks*. These requirements are complementary, not duplicate commands. Unlike TOOLS-02, this tool is **not** a JSON repair layer or a user-facing GUI. It is an on-demand information capability whose host inventory is the source for later ambient context.

## Implemented mapping

| Original requirement | TOOLS-06 design |
|---|---|
| §6A.3 OS/runtime data checked at startup | `HostProgramInventory` produces `AmbientProgramInventory` with OS/architecture and preapproved program observations; no model call |
| §6A.3 active tool list changes | Actual host `NativeToolRegistry` + per-turn grants determine model-available tools, *not* the candidate program inventory |
| §5.2 combined `exec` domain + small facade | Native module `lu.exec`, operation `lu.exec.check_env`, optional narrow model tool `lu_check_env(program)` only |
| Protobuf/trust boundary (§§4.6–4.8) | `EnvironmentCheckRequest` / `EnvironmentCheckResult` typed Protobuf; strict duplicate-safe JSON arguments; unknown fields/call type refused |
| User authority | Existing `ConversationAuthority` and trusted host `ToolAuthorizer` bound to saved user turn. Schema discovery ≠ permission |
| Host scope | Host configures program-ID → exact absolute candidate-file list; model cannot supply any path, shell flags, command or environment variable |
| Safety | Filesystem metadata only. **Never execute**, mutate, read files, resolve arbitrary PATH, inspect `.env` or return executable paths. Symlink/unexecutable files are not accepted observations |
| Factual reliability | `candidate_present` + evidence `file_present_only` or `not_found`; no invented version; `execution_verified:false`. A file on disk **does not prove** a runtime works |
| Resource budget | Maximum 12 named programs and 6 approved candidate files each. Missing candidates are ordinary negative results |
| Core / IDE distinction | `git` may be observed locally; **`gh` is not enrolled in Core**. GitHub CLI authentication and GitHub workflows are future IDE DLC scope |

Selected standard Core identifiers: `python`, `node`, `rustc`, `cargo`, `git`, `ffmpeg`, `tesseract`, `cmake`, `ninja`, `gcc`, `clang`, `7zip`. A host instance may enroll **only a subset**. It supplies exact candidate paths based on trusted package/runtime provisioning; no model-directed executable-name search takes place. Names are model data, **never** an executable command.

This first stage **intentionally does not implement shell/process launching**, version detection, PATH enumeration, SDK correctness, actual runtime execution, or automatic inclusion in the installed desktop system prompt. Those require separate host-owned, supervised probes, packaging, integration tests and the owner's approval of execution policy. Likewise `ToolEffect` currently has no pure information effect; `ReadWorkspace` is a conservative read-only admission class, even though the operation does not open work documents.

## Acceptance and sequencing

- Source: `src-tauri/lu-core/src/native_environment_discovery.rs`, typed Protobuf messages, `src-tauri/lu-core/tests/native_environment_discovery.rs`.
- Real Linux/Windows CI tests: dummy host-provisioned candidate files are **never launched**; observe positive and negative statuses, changed filesystem state, absent `gh`, no path disclosure, unsafe user/model arguments rejected, direct Protobuf spoofing, no authority/revoked task.
- **No dependency or Cargo.lock change:** this TOOLS-06 branch uses the standard Rust library, existing native registry, Protobuf, serde and tokio.
- **Branch intentionally based on verified TOOLS-02** rather than unfinished TOOLS-03 Git, TOOLS-04 archive and TOOLS-05 schema validation. Future integration must reconcile branches deliberately, not assume they are already merged.
- Full real installed GUI integration requires a host-provisioned candidate inventory, startup and per-turn refreshed system context (time, model, capabilities, tool allowlist), human-task admission and real Windows acceptance. **These are not proved by Core tests alone.**
- No change to accepted `0.1.0-alpha.2` or the v2.0 IDE DLC.

## Important risk boundaries

A malicious file named `python` can be present on disk without being Python, so the result must say only `file_present_only`. A future trusted executable provenance/hash/version check must **not reuse this presence bit as execution permission**. The host must not install or execute an arbitrary candidate merely because the model asks whether it exists. Any upcoming terminal/tool process command is a distinct `exec` effect class with process supervision, independent user authorization, durable outcome and cancellation.
