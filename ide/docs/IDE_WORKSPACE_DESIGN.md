# Lu IDE Workspace — Consolidated Design Document

**Document ID:** LU-IDE-DESIGN-01  
**Date:** 2026-11-04  
**Status:** Approved direction / implementation pending  
**Repository:** https://github.com/vjmichael/Lu-AI-GUI  
**Dedicated branch:** ide/workspace-development  
**Canonical task list:** Section 6 — Recommended Implementations (eight stages)

> This document consolidates the decisions for the Lu IDE DLC. It records intended behavior, boundaries, priorities, and acceptance tests; it does **not** assert that those features are already implemented or security-certified.

## 1. Mission, goals and constraints

Lu IDE Workspace is an optional, professional integrated development environment within the Lu desktop application. It must be useful for ordinary human software development and provide Lu with supervised capabilities to read, reason about, edit, build, test, debug, review and maintain software. The longer-term goal is a **disposable Lu Dev instance** capable of safely working on Lu itself.

Goals:
- Open existing projects with an effective, keyboard-accessible human IDE: explorer, multi-tab/split editor, search, diagnostics, symbol navigation, integrated source control, tasks, terminal, debugging, testing and project settings.
- Make Lu an assistive coding agent using the **same** trusted authorization, native tool host, durable journal, worker supervisor and model/memory infrastructure as Lu Core.
- Provide one **master package and toolchain manager** that coordinates the appropriate native package managers for Python, C/C++, Rust, JavaScript/TypeScript, Go, JVM, .NET and future ecosystems.
- Treat untrusted projects, dependency installers, build tools, test suites, language servers, debuggers, extensions and generated shell commands as potentially malicious executable content.
- Develop and test first in the local Linux environment; preserve Windows as an independently tested target. Do not require Docker, a separate Git repository, or a complicated end-user installation.
- Keep GitHub as source control and optional CI. Local builds, OS-level tests and measured evidence are required for acceptance.

Non-goals for the first version: a replacement for every language compiler or dependency resolver, guaranteed malware-free execution, a full extension marketplace, mandatory cloud services, or merging IDE work into the accepted Lu Core alpha before integration tests.

## 2. Repository ownership and development boundary

All IDE development is in **vjmichael/Lu-AI-GUI**, branch **ide/workspace-development**, under the top-level **ide/** directory. No additional repository is required.

| Owner | Responsibilities |
| --- | --- |
| Lu Core | Models and inference; ANSF/SQLite memory; authenticated human-turn identity; per-turn grants; native tool registry; host/worker Protobuf IPC; effect journal; worker lifecycle, cancel/recovery; basic Git functions. |
| IDE DLC | Source editor, project explorer, LSP/DAP integrations, workspace registry, package/toolchain orchestration, build/test/run configurations, advanced Git and GitHub UX, coding-agent workflow, Lu Dev workspaces. |
| OS execution layer | Verified Linux and Windows isolation primitives, quotas and process supervision; no untrusted worker should inherit host secrets or unneeded rights. |

**Branch isolation is not OS isolation.** No change may bypass the host permission broker because it lives in an IDE-only branch. Merge to main only after Lu-compatible interface review, tests and regression evidence. Existing Lu data, authorization semantics and accepted alpha artifacts must be preserved.

### Proposed layout

- **ide/ui/** — React/Tauri workspace views and editor integration.
- **ide/host/** — Rust workspace registry, project filesystem handles, Git services, manager adapters and host validation.
- **ide/proto/** — versioned IPC request/event messages and compatibility tests.
- **ide/workers/** — managed, restricted LSP/DAP/build/test/install/Git workers.
- **ide/tests/** — fixtures, cross-platform integration and adversarial OS security tests.
- **ide/docs/** — architecture, security model, manager contracts, design decisions, acceptance.

The earlier IDE-01A standalone Python Git inspection prototype is only exploratory. It is **not** evidence of Lu integration, merged code, or a verified sandbox.

## 3. Professional IDE experience and system architecture

### User-facing workspace
Provide project tree with scalable indexing/ignore rules; file tabs/splits; syntax highlighting; code completion; formatting; go-to-definition/references; rename/refactor where a language server supports it; multi-file search/replace; problems/diagnostics; tests and test failures; source control/diff editor; terminal/task console; debugger controls; keyboard shortcuts/command palette; accessibility; crash-safe file saves and history.

Evaluate **Monaco** and **CodeMirror** for editor UI rather than writing a syntax engine from scratch. Use **Language Server Protocol (LSP)** for language intelligence and **Debug Adapter Protocol (DAP)** for debugging, with process isolation for the servers/adapters themselves. These are design choices pending small prototypes; do not claim all languages have equal support.

### Host-service boundaries
The React WebView is not the authority for filesystem access or worker execution. A trusted OS directory picker registers a workspace; the host mints opaque workspace/file/job identifiers. Requests pass through versioned Rust and Protobuf contracts, verify the originating human turn or UI gesture, resolve the exact target and permitted effect, and then reach a host-supervised worker. Untrusted source text, LSP results, terminal logs and model-generated plans cannot grant new authority.

Workspace lifecycle:

1. **Register** selected project root and record provenance.
2. **Inspect** contents/manifests without executing project code.
3. **Assess trust** and show disabled automatic execution features.
4. **Approve** a specific action, scope, network policy and resource budget.
5. **Execute** via the verified OS isolation backend and stream bounded, structured diagnostics.
6. **Review** changes, test results, warnings and Git diff before accepting.
7. **Recover/close** with durable receipts, cancellation, orphan cleanup and uncertain-effect reconciliation.

Runtime events should contain host-controlled IDs, status transitions, progress evidence and redacted logs. They should support cancellation and recovery without losing already saved source changes.

### Advanced Git
**Basic Git remains in Lu Core.** IDE adds worktrees, staging hunks, branch comparison, signed commits, blame/history, stash, amend, cherry-pick, revert, rebase, bisect, conflict resolution, remote operations, PR review, GitHub/gh integration and CI inspection. Require review/confirmation for destructive rewrites, force pushes and remote changes. Prefer typed Git operations over assembling model-generated shell strings; handle credentials through a broker rather than exposing SSH keys or tokens to worker environments.

### Coding agent
Lu may inspect a project, explain options, create a plan, propose atomic edits, run approved lint/build/test actions, inspect diagnostics and revise. The user sees all active tasks, planned effects, diffs and results. Source files and build logs are untrusted context; prompt injection cannot expand privileges. A self-development instance must use a separate worktree and expendable processes, preserve the stable Lu binary, and request explicit review before any merge/update.

## 4. Master Package & Toolchain Manager

This is a **manager of managers**, not another general-purpose solver. It detects and delegates to native tools while presenting one integrated interface for dependency operations, runtime/compiler selection, environment health, advisories and reproducibility.

| Ecosystem | Initial delegated tools | Project metadata |
| --- | --- | --- |
| Python | uv first, with controlled compatibility paths to pip where needed | pyproject.toml, uv.lock, project .venv, Python version |
| C/C++ | vcpkg manifest mode or Conan 2; GCC/Clang/MSVC and CMake/Meson/Ninja toolchains | vcpkg.json or conanfile, profiles and lockfiles |
| Rust | Cargo and rustup | Cargo.toml, Cargo.lock, rust-toolchain |
| JavaScript/TypeScript | npm, pnpm or Yarn as project-selected | package.json, packageManager, respective lockfile |
| Go | Go modules/toolchain | go.mod, go.sum |
| Java/Kotlin/JVM | Maven or Gradle and JDK selection | pom.xml, Gradle build/wrapper metadata |
| C#/.NET | dotnet SDK and NuGet | global.json, project files and lock metadata |

Do not silently switch a project's manager or write multiple competing lockfiles. Prefer per-project environments and pinned toolchain versions; never modify system Python, OS package databases, or install as administrator/root by default.

**Adapter lifecycle:** Detect -> Inspect -> Resolve/Preview -> User Approves -> Apply in restricted worker -> Verify installed artifacts and manifest/lockfile diffs -> Reconcile/Roll back when safe. Each plan records the exact manager executable/version, requested operation, input digests, dependency sources, download sizes where available, hooks/scripts, network/privilege requirements, output paths, expected changes, and expiry. When resolution inherently executes a build backend or script, clearly mark preview incomplete and require stronger isolation or confirmation.

The UI should provide one **Packages & Toolchains** view with installed runtimes/compilers; environment profiles; dependencies and available upgrades; lockfile health; registry/source provenance; CVE/license advisories; offline mode; explicit install/update/remove approval; and manager-native diagnostics for advanced users.

Package fetching and installation are **not automatically safe**. Python build backends, npm lifecycle scripts, CMake, compiled code and Git dependencies can execute arbitrary instructions. The manager must not automatically run them on project open. Treat metadata network access, actual downloads and executable install hooks as distinct grantable effects. Scan/advisory checks and signed provenance improve trust but do not replace process isolation.

## 5. Security and reliability baseline

### Threat model
Hostile cloned repositories, README prompt injection, dependency postinstall hooks, custom build definitions, test runners, LSP servers, debug adapters, extensions, compromised toolchains, malicious Git hooks, exfiltration of credentials, deletion/overwrite of unrelated files, resource exhaustion, symlink races and interrupted native effects.

### Four trust levels
1. **Untrusted (default):** browse, inspect and edit explicitly authorized source files; do not auto-run project tasks, extensions, LSPs that execute programs, debuggers, package installs, terminals, Git hooks or scripts.
2. **Approved restricted operation:** execute one named task with exact workspace/file bounds, resource/network/time policy and revocable grant.
3. **Trusted development:** convenient day-to-day project operations under bounded, auditable grants; no implicit root/admin or blanket access to other projects and personal secrets.
4. **High-risk isolated execution:** stronger disposable VM/microVM or other verified OS-backed isolation when investigating deliberately suspicious projects. Fail closed when isolation promised by the UI is unavailable.

### Concrete implementation requirements
- **Least privilege:** isolated home and temporary directories; sanitized environment; no inherited SSH/GitHub/cloud credentials, browser profile, Lu private store, unrelated project roots or privileged sockets.
- **Files and writes:** host-brokered capability/root handles, race-resistant path validation, symlink/reparse escape checks, atomic writes/backup where appropriate and reviewable diffs.
- **Linux:** investigate bubblewrap-backed namespaces and mount restrictions, Landlock, seccomp, no-new-privileges, process groups and cgroup quotas where supported. Detect actual host capabilities; do not assume delegation or pretend bubblewrap alone is a complete policy.
- **Windows:** investigate restricted tokens/AppContainer where viable, explicit ACL/network restrictions and Job Objects for supervised cleanup/quotas. Job Objects alone are not a filesystem/network sandbox.
- **Network:** disabled for untrusted code by default; optional narrow fetch broker or approved egress rule. Credentials supplied only through trusted broker interactions and never copied into a broad child environment.
- **Lifecycle:** host-controlled spawn, bounded log capture, cancel/terminate, cleanup, timeouts and resource reporting; durable statuses: prepared, applied, completed, failed, cancelled, **uncertain** pending reconciliation.
- **Separation:** a trust badge, UI warning, antivirus scan, process-name check or Git branch does not enforce containment. Do not execute if the required sandbox cannot start.
- **Extension safety:** treat executable extensions as independent untrusted workloads. Installation, publisher trust, permissions and upgrade control are explicit.
- **Accidental effects:** Git rewrite and file deletes require suitable backups, scope checks and user review.

### Mandatory adversarial acceptance
Prove on **real Linux and Windows** that: opening hostile source executes nothing automatically; a package script cannot read SSH keys; a build attempting to delete files outside the project is denied; symlink/TOCTOU escapes fail; network exfiltration is blocked when disabled; fork/resource exhaustion is bounded; cancelled workers leave no descendants; malicious LSP/DAP execution stays confined; injected project instructions cannot create grants; sandbox-launch failure stops execution; interrupted writes have accurate durable receipts and never blindly re-execute uncertain effects.

No IDE can guarantee arbitrary native code is harmless: isolation failures, kernel/driver vulnerabilities and user-authorized high-risk operations remain risks. Security claims must be based on measured OS-level test evidence, not unit-test mocks.

## 6. Recommended Implementations — **canonical task list**

This **eight-stage sequence is the official Lu IDE task list**. Existing IDE-01B through IDE-10 identifiers remain *engineering subgates and cross-cutting release checks*, not a competing ten-item task list. Mark a stage complete only after its acceptance evidence is recorded.

| Task | Status as of 2026-11-04 | Scope | Minimum acceptance |
| --- | --- | --- | --- |
| **1. Rust workspace host + Protobuf interface** | Not started | Host workspace registry; trusted picker; opaque handles; lifecycle; Lu Core authority/journal integration | Linux tests prove open/close/reopen, path scoping, authenticated caller, durable state, cancel |
| **2. Linux sandbox and real execution tests** | Not started | OS isolation launcher, worker supervision, path/network/secret denial, quotas, adversarial harness | Real hostile-process tests on Linux; no silent unisolated fallback; logs and receipts; Windows strategy recorded |
| **3. Project explorer and code editor** | Not started | Human-first multi-file editor, search, tabs/splits, safe save, diff preview, shortcuts | Large project usability, undo/redo, recovery, no project auto-execution |
| **4. LSP, build/test and debugger integration** | Not started | LSP/DAP adapters; terminal/task runner; streaming diagnostics; run configs | Representative Python/C++/Rust/JS projects; bounded build/test/debug workers; cancellation and crash cleanup |
| **5. Master package manager** | Not started | uv and Python envs; vcpkg/Conan and C/C++ toolchains; Cargo, JS, Go/JVM/.NET adapters in staged rollout | Detect/preview/approve/apply/verify; lockfiles; no install-on-open; blocked malicious hooks; offline/recovery scenarios |
| **6. Advanced Git and GitHub workflows** | Not started | Worktrees, staging, history, rebase/cherry-pick/conflicts, PR/CI, remote credentials | Rehearsal fixtures; destructive-action confirmation; recoverable conflicts; no ambient secret leaks |
| **7. Lu AI coding integration** | Not started | Agent plans, diff proposals, per-turn tool grants, test/error feedback and task durability | End-to-end code change with review, verified tests, prompt-injection rejection and cancel/restart recovery |
| **8. Lu self-development environment** | Not started | Disposable Lu Dev instance/worktree, isolated build/test, version comparison and human-approved update | Stable Lu unaffected by failed build; terminate/rollback; merge proposal only after test evidence |

**Execution order:** 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8. Some research or read-only prototyping may overlap, but **no package/build/test/executable extension feature is accepted before Stage 2's minimal verified containment**. Stage 8 does not automatically authorize self-modification of the running Lu binary. Windows native sandbox parity, installer/upgrade tests and release hardening are mandatory cross-stage acceptance and final release gates, not an additional numbered task.

### Implementation discipline
For each task: (a) design/API and threat analysis; (b) implement on dedicated IDE branch, preserving core boundary; (c) unit/integration tests; (d) native Linux acceptance; (e) Windows parity where effects are OS-specific; (f) Git commit and evidence; (g) user-facing status and remaining defects. “Designed”, “compiled”, “tested under mocks”, “native tested” and “accepted” are distinct statuses.

## 7. Decisions, dependencies and open questions

**Decided:** one repository; dedicated IDE branch; IDE separate from Lu Core; advanced Git in IDE, basic Git in Core; one manager-of-managers; offline-capable local-first workflow; no automatic execution of new projects; host authorization and OS-backed isolation before executable automation; local Linux development and Windows acceptance; human approval for high-impact changes.

**To prototype/evaluate:** Monaco versus CodeMirror; first language server/debug adapter bundle; Linux isolation availability per distro; Windows restricted process architecture; package manager provisioning and signed executable provenance; extension policy; remote auth broker; exact versioned Lu DLC ABI/Protobuf messages; useful project-level trust UX. None of these should silently override existing Lu Core policies.

**Dependencies:** Lu Core's authenticated user turns, Protobuf host/worker framework, durable effect journal, process supervisor, native file operations, and existing Git primitives. If any are unavailable or incomplete in the checked-out branch, identify the gap before adding parallel implementations.

## 8. Reference documentation and research

Internal:
- [IDE architecture](ARCHITECTURE.md)
- [Package and toolchain orchestrator](PACKAGE_ORCHESTRATOR.md)
- [Execution security model](SECURITY_MODEL.md)
- [Engineering subgates and acceptance](IMPLEMENTATION_GATES.md)
- [Lu v1.0 reconciliation](../../../docs/LU_V1_0_DESIGN_RECONCILIATION_20261103.md)

External references (implementation guidance, not blanket certification):
- VS Code Workspace Trust: https://code.visualstudio.com/docs/editing/workspaces/workspace-trust
- Language Server Protocol: https://microsoft.github.io/language-server-protocol/
- Debug Adapter Protocol: https://microsoft.github.io/debug-adapter-protocol/
- uv project management: https://docs.astral.sh/uv/guides/projects/
- vcpkg manifest mode: https://learn.microsoft.com/en-us/vcpkg/consume/manifest-mode
- Conan 2 documentation: https://docs.conan.io/2/
- Cargo: https://doc.rust-lang.org/cargo/
- Bubblewrap: https://github.com/containers/bubblewrap
- Linux Landlock: https://docs.kernel.org/userspace-api/landlock.html

---
**Document control:** Changes to the eight task definitions or security invariants should update this document and its related subgate checklist in the same IDE-branch review. Do not label unverified features “complete.”
