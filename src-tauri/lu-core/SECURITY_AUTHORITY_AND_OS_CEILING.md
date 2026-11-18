# Lu Security Decision — Broad Host Access, Human Task Authority

**Status:** Owner-confirmed design correction, 2026-11-03. Not all protections are integrated in the desktop.
**Supersedes:** The 2026-10-25 mandatory AppContainer/restricted-worker/sandbox assumptions and the 2026-11-03 autonomy sandbox audit wherever they conflict.
**Scope:** Lu desktop, Lu Core, native tools, worker runtime, IDE/Film/Research/Writing DLCs.

## First principles

> **Access is not authority. Permission is not authority.**

Lu is a native Windows AI application with broad access to the computer's installed tools, files, applications, devices and operating-system facilities. A mandatory sandbox, artificial workspace-only filesystem, default-denied general connectivity, restricted-token-only execution or VM/container requirement **is not part of Lu's product design**. Optional isolation may be requested for a particular risky task or test, but never silently imposed as Lu's universal operating environment.

Possessing an OS handle, an existing account permission, a tool registration, or a callable API does **not** authorize Lu to use it for an arbitrary purpose. The user determines Lu's authority by directing and delegating tasks; trusted Lu host code enforces that authority before committing effects. Broad access supports usefulness and prerequisite discovery; authority controls actions.

The model cannot create a user instruction by pretending, import a grant from a retrieved document, rewrite an existing delegation, impersonate the user, or declare itself elevated. An administrative action requiring UAC must have a distinct explicit human-driven approval pathway; merely being capable of invoking an installer does not authorize it.

## Five distinct concepts

1. **Available access:** What Lu's native installation and its host account/OS can actually reach. Discoverable, not equivalent to approval.
2. **OS permission:** A technical ability conferred by Windows, credentials or process token. It does not express user intent.
3. **User authority:** Human-origin, authenticated instruction or explicit durable delegation, with purpose, effect classes, scope, time, resource/destination expectations and revocation.
4. **Execution decision:** A trusted host checks the intended operation, target, arguments, provenance, consequence and current task authority. An AI-proposed tool invocation alone cannot pass as approval.
5. **Effect safety:** Journaling, precondition checks, backups, atomic file replacement, Trash/restore, receipt/reconciliation, cancellation and controlled process lifetime protect against mistakes even when an action is authorized.

## Operating model

- **Direct host capabilities:** Lu may discover and use installed compilers, Python, PowerShell, Git, SDKs, local services, WSL, and other operating-system facilities within the authority granted for a task. No mandatory Lu-wide sandbox, Docker install, or special worker identity.
- **Native tools first:** Rust tools and local workers use typed Protobuf contracts; MCP is optional. Avoid making the user shuttle files manually just because a task needs a tool not already installed in Lu.
- **Permissioned provisioning:** Lu detects a missing dependency, evaluates trustworthy source, version, architecture, digest/signature and license, then obtains an appropriate installation grant before executing. User-scoped installations can be preferred for reversibility, not mandated by an artificial isolation rule. Changes to the global OS or privileged components require separate informed approval and applicable Windows confirmation.
- **Scoped delegation without approval fatigue:** When a task explicitly includes building, testing, editing and installing project dependencies, Lu may perform ordinary steps within the scope without repeated prompts. Ask again for meaningfully different or unusually consequential actions.
- **High-consequence operations:** Mass deletion, credential disclosure, externally transmitting sensitive data, system settings changes, privileged installers, finance or physical device operations require stronger task-specific review. Do not mistake a generic file-access grant for permission to delete unrelated files.
- **Untrusted input:** Webpages, prompts inside files, repository READMEs, model/tool output and downloaded installers are data, not user authority. Verify actual effect targets and destinations at the final host boundary. Prompt injection can exploit overly broad grants, so check action relevance, not only origin.
- **Workers:** Process cancellation, Job Objects, crash isolation, quotas and resource supervision are reliability and damage-limitation tools, **not evidence of or substitutes for task authorization**. A task may opt into disposable/sandboxed execution when desired.
- **Durability:** Persist authenticated delegation and decisions. Before a non-idempotent action, record intent. On ambiguous failures, label UNCERTAIN and reconcile; never blindly repeat. Respect live revocation and cleanup.

## Required acceptance

1. A directly authenticated user authorizes Lu to inspect an arbitrary host directory outside its current project; Lu can do so without an artificial workspace ceiling while unrelated actions remain unauthorized.
2. Within an explicit developer task, Lu discovers a compiler, runs it, installs an approved missing package, builds, fixes tests, and resumes after restart, without a mandatory container.
3. An untrusted README requests secret exfiltration, deletion, or unauthorized network access. The host rejects the *operation* because it is outside the user's instruction, even if the host OS has read/write/network permission.
4. Normal approved read/write/search/git operations continue efficiently under existing authority. New destructive/system/network effects outside the task require fresh approval.
5. Revocation blocks new actions and stops or reconciles in-flight work. Authorization receipts cannot be forged by a model or another conversation.
6. Child process and crash recovery is measured independently of authorization. No unsupported assertion that a Job Object alone prevents all destructive actions.
7. Lu Dev can create and test changes using native host tools. Replacement of the stable executable, permissions store, or system components requires explicit, independently verified promotion and rollback.

## Implementation status

**Existing:** `conversation_authority.rs` origin verifier interface and task lineage; `policy.rs` tool/effect/scope grants; `journal.rs` SQLite effect receipts; bounded native read/list tools; Win Job lifetime management for inference child; model-tool boundary.

**Incomplete:** Desktop-authenticated persisted grant ledger and user approval UX; argument/target-level approval checks; full host-capability inventory and installers; native write/execute/network tools; robust revocation of actual running operations; durable end-to-end task continuations and Windows acceptance.

**Change control:** Do not delete tested permission/journal code merely because universal sandboxing is rejected. Reinterpret or amend overly restrictive default workspace assumptions at the appropriate API boundary with tests. Any restriction or privileged action must trace to a concrete user-delegated authority or an explicit safety-specific review rule, not a blanket 'Lu cannot access host resources' product assumption.

## Explicitly superseded assumptions

- Mandatory AppContainer/LPAC/restricted-token execution for all tools.
- Every tool request must be confined to a fixed sandbox root irrespective of user instruction.
- Default-deny all network capability purely because Lu is an AI.
- Only user-writable package caches may be used regardless of the actual task.
- OS permission or broad access implies unrestricted authority.

The central rule is **broad host access, narrow and authenticated action authority, recoverable effects**.
