# ADR-0001 — Standalone IDE application and optional Lu bridge

**Date:** 2026-11-04  
**Status:** Accepted architectural decision  
**Scope:** Lu IDE development on `ide/workspace-development`

## Context

The coupling model for Lu DLCs/workspaces is not yet finalized. Therefore the IDE **must not** depend on being loaded into Lu's Tauri/React process, inheriting Lu Core Rust modules, or sharing Lu's internal registries and in-process authorization objects.

## Decision

1. **One repository, two executables:** `vjmichael/Lu-AI-GUI` remains the shared source repository, while `ide/app` owns the **standalone Lu IDE desktop application**. The existing `lu-desktop` remains independently buildable and unchanged by IDE functionality.
2. **Independent native host:** `ide/host` owns IDE project registration, local metadata and future native build/edit services. It does not link against `lu-core` or require Lu running.
3. **Optional connection:** Lu-specific coding-agent capabilities are delivered by an **out-of-process bridge** negotiated after both applications run. The IDE remains functional when Lu is absent, incompatible, stopped, or permission-revoked.
4. **Versioned protocol:** Bridge messages shall be Protobuf on a verified local OS IPC transport. The transport and authentication model remain open until we align the interface with Lu's current host APIs. Mere knowledge of a socket path or request ID is **not** permission.
5. **Separate trust domains:** The IDE owns its workspace sandbox and cannot assume Lu's model or task permissions. Lu is a potential *client* requesting services. IDE actions require independent user intent, target validation, and OS-enforced security. Lu's own per-turn grants must also be checked by Lu before a request crosses the boundary.
6. **Safety first:** No code execution on project open. No automatic connection to a local process that merely claims to be Lu. No raw project host paths or credentials in untrusted messages. No "fallback to unsandboxed execution" when Lu is offline or an isolation mechanism is absent.
7. **No Docker requirement:** Local-first native app using a single installer per OS. Linux and Windows tests must be independent.

## Repository layout

```
Lu-AI-GUI/
  lu-desktop/                 # existing Lu application, unchanged
  src-tauri/lu-core/          # existing Lu Core, unchanged
  ide/
    app/                      # independent Tauri + React IDE executable
    host/                     # standalone native workspace registry
    proto/                    # IDE UI protocol; optional Lu bridge contract later
    docs/                     # canonical design, task list, acceptance and ADRs
    workers/                  # future sandboxed package/build/debug workers
```

## Planned bridge behavior (not yet implemented)

- **Discovery:** IDE may explicitly connect to a Lu host endpoint that is configured through a trusted local UI or authenticated launch broker. Do not auto-trust a magic localhost port or arbitrary unauthenticated IPC endpoint.
- **Handshake:** negotiate protocol/version ranges and independently identify the application endpoints using OS-backed peer identity/credential checks or an equivalent mutually authenticated scheme.
- **Capabilities:** distinguish model-chat, plan-propose, diff-propose, diagnostics-read, execution-request and task-cancel. Discovery only advertises functions; grants require explicit authorization.
- **Workflow:** Lu proposes a change -> IDE validates the bounded target and user authorization -> IDE shows preview -> IDE executes through its own supervised worker after consent -> IDE returns bounded results/receipts -> Lu may analyze and propose another step.
- **Failure handling:** timeouts, disconnects, duplicate effect IDs, crash recovery, cancellation and reattachment must be durable and unambiguous. When effects are uncertain, do not replay automatically.
- **Security:** source code, build logs and package metadata sent to Lu are **untrusted context**. Lu-generated commands, parameters and completion claims are untrusted proposals at the IDE boundary.

## Consequences

- The earlier experiment embedding an IDE project panel directly inside `lu-desktop` was rolled back on the IDE branch. It is superseded by `ide/app`.
- The earlier shared-`WorkspaceHost` Rust constructor was replaced by a standalone project catalog. Existing Lu Core APIs remain relevant for future interoperation but are not a build dependency.
- Task 1 is now **standalone host + native Tauri picker + Protobuf lifecycle + optional bridge design**. Windows/Linux checks are required.
- The canonical eight-stage task list in `IDE_WORKSPACE_DESIGN.md` remains unchanged in number and order. Stage 7 becomes *bridge-mediated Lu AI coding*, not in-process embedding.
- A successful Tauri compile is not yet proof of installed app launch; installer and real interaction checks remain separate acceptance gates.

## Current verified state

- Standalone host project metadata and strict UI Protobuf lifecycle implemented.
- Independent frontend UI implemented, exposing only project lifecycle commands.
- No executable code workers or actual Lu bridge active.
- See current `ide-standalone.yml` workflow and verified commit-specific CI results for platform test status.
