# Lu Runtime & Dependency Manager — Deferred Design Specification

**Status:** Design documented; not authorized for implementation during alpha  
**Target milestone:** Lu **v1.05** (the user's designated halfway milestone toward v2.00)  
**Priority:** Deferred. **Alpha Lu first**: Windows x86-64 installer, verified persistent GGUF inference, native Protobuf tool execution, and an end-to-end usable conversation.  
**Scope:** First-party Lu core infrastructure service; **not** a separate DLC.  
**Decision date:** 2026-10-28

## 1. User-facing contract

The user manages **capabilities**, not package managers, dependency graphs, virtual environments, environment variables, or terminal commands. A healthy tool starts and works; if broken, Lu attempts a safe, deterministic repair and verifies the result. The default UI reports only: (a) working; (b) repaired, no action necessary; (c) action/authorization needed; or (d) repair unsuccessful with retry/restore options. Expose detailed technical diagnostics only on request or in opt-in developer mode. No general-user CLI requirement.

## 2. Problem statement

Python packages previously appeared to disappear after npm build/install workflows in a toolkit project. The exact cause is **not established**; possible contributors are output-directory cleanup, recreated `node_modules`, using the wrong Python interpreter, packaging omission, and unstable environment paths. Do not record any one cause as verified until build scripts and logs are inspected.

The larger failure mode is fragmented dependencies: first-party tools, core extensions, and specialized workspaces can depend on incompatible Python/Node/native packages. The agent also must distinguish an absent executable from an executable outside PATH or outside its authorized sandbox. Lu must not solve these problems by giving the model unrestricted host access.

## 3. Architectural decision

Use **uv** as the preferred Python environment and dependency-management engine, managed by a **Lu-owned Rust Runtime & Dependency Manager**. uv supplies Python version discovery/management, environment creation, project lockfiles and synchronization. Lu provides policy, ownership, auditing, lifecycle, health checks, retry, repair, and user experience. Continue using npm with pinned lockfiles for Node-based build/runtime components where appropriate; ship verified native binaries for Rust/C++ components. uv does not replace every package ecosystem.

Maintain separation between **build-time tooling** and **run-time dependencies**. Runtime environments live in Lu-owned versioned application-data storage, **outside** directories deleted/recreated by `npm ci`, package builds, and distribution packaging. Never mutate the system Python environment or the user's unrelated environments.

### Conceptual components

- **Runtime registry:** tool/CE/workspace ID, publisher, version, runtime type and version, package manifest hash, health state, install root, permitted capabilities and provenance.
- **Package resolver:** validate pinned manifests and reproducible lockfiles (`pyproject.toml` + `uv.lock` for Python); support verified local/offline caches.
- **Environment provisioner:** create versioned, per-extension isolation; invoke an approved `uv` binary through the trusted Lu host, not from model-generated arbitrary commands.
- **Integrity and health verifier:** hashes/signatures, expected executable/library existence, import probes, feature-level smoke tests, diagnostics.
- **Recovery coordinator:** bounded retry, reconstruct environment, verify, atomically switch active version; revert to last-known-good installation on failure.
- **Capability/status API:** expose usable capabilities and categorized failure states over **Lu-native Protobuf IPC**; JSON only where explicitly needed at external boundaries.
- **Audit and consent:** policy-controlled installation permissions, clear publisher and download scope, versioned durable audit trail. Models request actions; only host/user authority approves them.

## 4. Dependency manifest sketch (conceptual; not a final schema)

Each installable component declares: component ID, signed publisher identity, package version, supported Windows/architecture, Python version constraint, immutable Python lockfile reference and digest, package integrity metadata, environment feature probes, disk requirements, network needs, native libraries and compatible Lu API version. The protobuf schema and manifest format are to be specified and tested when implementation begins.

Sample on-disk layout (illustrative; paths configurable):

```text
LuData/
  runtimes/python/<version>/
  components/<publisher>/<component>/<release>/env/
  manifests/<component>/<release>/
  package-cache/
  receipts/
  recovery/last-known-good/
  logs/
```

Avoid storing mutable runtime environments in `node_modules`, generated `dist/`, temporary build directories or application resources overwritten during upgrade. Caches may be shared, but environments remain isolated.

## 5. Recovery state machine

**DISCOVER → VERIFY → HEALTHY** or **DEGRADED → PLAN_REPAIR → CHECK_AUTHORITY → STAGE → VERIFY_STAGED → ACTIVATE → HEALTHY**. Any staging/verification failure goes to **ROLLBACK → REPORT**, retaining sufficient logs and a previous known-good version. Do not claim repair until a capability-level smoke test passes.

Error distinctions: `COMPONENT_NOT_INSTALLED`, `EXECUTABLE_NOT_IN_ALLOWED_PATH`, `SEARCH_SCOPE_RESTRICTED`, `PERMISSION_REQUIRED`, `DEPENDENCY_MISSING`, `LOCKFILE_MISMATCH`, `INTEGRITY_FAILURE`, `INCOMPATIBLE_RUNTIME`, `NETWORK_UNAVAILABLE`, `RESOURCE_EXHAUSTED`, `REPAIR_FAILED`. Avoid disclosing protected host paths to unauthorized models. A restricted search is not evidence of global absence.

### Repair order

1. Validate capability record and the active conversation/task authorization.
2. Probe the current Lu-owned runtime and locate the failure deterministically.
3. Prefer a known-good local package cache; request network permission if downloads are needed and not already granted by policy.
4. Build a **new staging environment**, never synchronize unrelated user/system environments.
5. Verify exact dependency versions, integrity, imports, native compatibility, and tool smoke tests.
6. Atomically switch to verified staging environment; preserve prior release for rollback.
7. Update capability status and emit succinct user notification; retain technical diagnostic report.

Note: `uv sync` is **exact by default** and can remove extraneous packages; it must only target Lu-owned isolated environments. Avoid runtime network resolution of unpinned dependencies. Pin and audit the uv binary itself.

## 6. Security and permissions

- Maintain conversation-bound authority; every tool/runtime action is authorized by Lu's PolicyEngine, not by model assertions.
- Signed or otherwise authenticated publisher trust and integrity checks precede execution; trust/revocation state survives restart.
- Differentiate **discover**, **read**, **install**, **download**, **execute**, and **modify system settings** permissions.
- Restrict executable invocation to approved paths and arguments; sandbox subprocesses and cap CPU, RAM, disk, network and time.
- Never silently run arbitrary install scripts from untrusted dependencies with broad privileges.
- Recover a broken tool only within its own Lu-owned namespace unless user approval explicitly extends the scope.
- Maintain audit and cancel/revoke mechanisms; repairs must be restart-safe and idempotent.

## 7. UX behavior

- **Healthy:** “Mikey's Toolkit is ready.”
- **Self-repaired:** “A required component was restored. Everything is working.”
- **Approval needed:** “Lu needs permission to install a verified dependency for Mikey's Toolkit.” Offer Approve / Cancel and optional details.
- **Failed:** “Lu couldn't restore this component. Retry or restore the previous version.” Show a usable error reference.

Advanced view: dependency provenance, diagnostics, exact resolved versions, failure stage, storage usage, recovery history and manual opt-in controls. No mandatory terminal instructions to ordinary users.

## 8. Acceptance criteria for v1.05

1. Windows x86-64 clean install of a Python-requiring native tool with no globally installed Python.
2. Pinned lockfile reconstructs the identical environment from cache; offline behavior explicitly validated.
3. Simulated missing/corrupted package self-repairs and proves functionality via smoke test.
4. A failed upgrade restores a previously verified version without damaging user data.
5. Running `npm ci` or a package build cannot erase an active Lu-owned Python environment.
6. Two extensions using conflicting dependency versions work in separate environments.
7. Host permissions distinguish hidden/inaccessible/missing executables and never leak forbidden paths.
8. Permission denial leaves existing installed components intact; no unexpected privileged actions.
9. Restart mid-repair yields a recoverable, consistent state, not half-activated files.
10. Supportable user-facing messages and full developer audit trail; no general-user CLI required.

Performance checks: cold setup time, warm repair time, disk deduplication, steady-state process overhead, idle CPU/memory and package cache footprint. Set numerical thresholds when alpha hardware data exists.

## 9. Milestone and deferrals

**Alpha / v1.00 stabilization:** No new general dependency service requirement. Ship a self-contained Windows installer and prioritize first successful model load, inference, persisted conversation, native-tool call, permission gate and diagnostics. Fix only dependency issues that directly block alpha.

**v1.05:** Implement the minimal Runtime & Dependency Manager for Lu-owned Python environments with uv, signed manifests/lockfiles, health probes, self-repair, staging/rollback and Protobuf status integration.

**Later milestones toward v2.00:** More complete Node/native dependency orchestration, multi-platform support, sophisticated offline package distribution, fleet/workspace-level policy and DLC store lifecycle integrations. These are planned possibilities, not commitments.

**Explicit non-goals:** A 100 GB mandatory mega-update, rewriting uv, unrestricted scanning of host software, automatic system-wide package changes, Docker requirement, or making users administer Python environments.

## 10. Implementation questions to resolve at v1.05 kickoff

- Specific uv version, redistribution/update policy and cryptographic distribution verification.
- Exact installer-owned vs user-profile-owned storage and Windows ACL strategy.
- Compatibility with Lu's signed DLC and publisher revocation scheme.
- Dependency closure and offline package availability for native Python wheels.
- Transactional/atomic activation strategy on Windows with locked DLLs and running subprocesses.
- What can safely be repaired silently versus requiring user approval.
- Whether separate Python processes are required for isolation between untrusted plugins (venvs are dependency isolation, not a security sandbox).

## References

- uv documentation: https://docs.astral.sh/uv/
- Project locking and syncing: https://docs.astral.sh/uv/concepts/projects/sync/
- Project layout and universal lockfile: https://docs.astral.sh/uv/concepts/projects/layout/
- Python version management: https://docs.astral.sh/uv/concepts/python-versions/

**Decision summary:** Documented for Lu v1.05. **No implementation work starts now. Alpha Lu remains the active milestone.**
