# LU-IDE-ARCH-02 — Capability-first architecture and build strategy

**Decision date:** 2026-11-05 (America/Phoenix)  
**Status:** Accepted development direction / design only; not an implemented runtime, acceptance result, or a rename of the product.  
**Canonical project:** standalone Lu IDE, `ide/workspace-development`.  
**Supersedes:** the strictly linear execution order attached to section 6 of `IDE_WORKSPACE_DESIGN.md`. The **eight existing canonical stage IDs remain** for history, reporting and acceptance.

## Decision

Build Lu IDE as an **authoritative development host with a versioned semantic operation layer** and two potential clients: a complete human UI **now**, and an optional authenticated AI client **later**. The IDE does not depend on Lu Core or an AI model.

A feature is not complete just because a button works: its host operation must expose stable object identities, explicit preconditions, typed success/failure, provenance, permission/effect classification, cancellation/recovery when relevant, and verifiable evidence. Design for humans and authorized AI together, but never let discovery imply authority.

```text
Human UI (React/CodeMirror, keyboard, accessible messages)
                |
        Versioned IDE host API
                |
    Semantic capability & object layer
   /         |         |          \
Project     Source    Evidence     Action authorization
catalog     snapshots ledger      / effect planning
   \         |         |          /
       Trusted standalone Rust host
         |        |          |
      Files/Git  resource    supervised worker
       (read/    inventory   lifecycle (future)
       writes)    + probes    |
                              LSP/build/test/DAP/package
                              (off until OS gates pass)
      -------------------------------------------
      Optional authenticated Lu adapter (DEFERRED)
```

The semantic layer is an internal Rust/Tauri boundary today. We may define transport-neutral Protobuf descriptions early but **must not activate a listening Lu bridge** until separate user authorization and cross-app acceptance.

## Core architecture: seven host-owned services

1. **Workspace registry** — persistent opaque project IDs, normalized root identity, Git repository/worktree and branch information, accessible project state, verified current folder identity; never trust an arbitrary UI-supplied absolute target.
2. **Source object and snapshot service** — `projectId`, `fileId`, relative path, disk revision, editor-buffer revision, line/column/byte offset, optional symbol. Distinguish disk from unsaved buffers. Retain revision-specific line maps; refuse or explicitly remap stale ranges.
3. **Capability registry** — bounded descriptors for actions such as `project.list`, `source.readSpan`, `source.find`, `proposal.preview`, `proposal.stageDraft`, `file.save`, `git.status`, `resource.probe`, `test.results`; versions, schemas, effect classes, pre/postconditions, errors and evidence outputs. Descriptors never grant execution.
4. **Operational resource inventory** — project-scoped toolchains, dependencies, datasets, fixtures, recipes and build artifacts. Local path/URI and digest, host-probed status, compatible platforms, last trusted use, current dependency health and provenance; use PR #28 as a *design reference* only, not a runtime dependency.
5. **Development ledger and context index** — host-backed append-only events + materialized current-state view; links to Git commit, local worktree revision, changes, verification runs, benchmark input versions, delivered outputs and decisions. `PROJECT_STATE.md` and `CHANGELOG.md` remain exported human-readable entrypoints, not an undifferentiated authority source. Local-only, remote-only, divergent and unknown must remain distinct.
6. **Operation and evidence journal** — prepared/applied/failed/uncertain effect receipts, operation IDs, causal links, start/finish timestamps, cancellation and reconciliation. The source commit and actual tested working-tree snapshot must be recorded. Re-running uncertain, non-idempotent effects automatically is forbidden.
7. **Execution supervisor (gated)** — explicit human authorization, process containment, capability-limited workers, resource and time budgets, cancel/kill and cleanup, OS-specific hostile-fixture verification. Read-only metadata discovery can precede this service; LSP, test, build, package and debug workers cannot.

## Human and AI experience must share the same object graph

| User workflow | Authoritative object/operation | Human-facing behavior | Future AI-facing behavior |
| --- | --- | --- | --- |
| Open a project | project register/list/resolve | Project browser with path and branch identity | Discover selected project ID and current verified state |
| Read code | source snapshot/readSpan | Line numbers, highlight, file and revision | Exact path + revision + range and cited source span |
| Search/replace | snapshot search; buffer edits | Find/replace with undo and explicit Save | Bounded proposals targeting exact buffer/disk revisions |
| Stage proposed changes | proposal preview/approve/stageDraft | Diff and staged unsaved tabs | Proposal data only; no implicit write |
| Save | file.save with revision precondition | Explicit Save, clear conflicts and receipt | Request only under current human authorization |
| Inspect Git | git.status/diff/history (read-only first) | Local/remote divergence with clear provenance | Typed result; no guessed branch or destructive action |
| Find a compiler or installer | resource.list/probe | Historical/present/verified status and exact location | Probe before redownload; cannot execute from memory |
| Test/build | job.preview/authorize/run/results (future) | Show scope, progress, logs, cancel, evidence | Same bounded job IDs and receipts under verified grants |
| Resume work | ledger.current/history/checkpoints | What changed/where/what passed/what is next | Structured context capsule, sources and uncertain outcomes |

Source objects and capability descriptors must use explicit API/schema versions. Avoid allowing frontend-only business rules to become a second, conflicting authority. The human interface is an accessibility and workflow presentation layer; the host validates every effect.

## Durable ledger: storage and portable views

**Planned storage:** application-profile SQLite with migrations and WAL for structured event records, indexed by workspace/project/task/source revision/artifact; use filesystem atomic publication for exported Markdown. A ledger write succeeding does **not** prove an external effect succeeded; reconcile with host receipts and filesystem state.

**Minimum event:** event ID, UTC time, origin (human/host/import), workspace/project/task IDs, source refs and Git HEAD, local working-tree snapshot/dirty status if known, action, outcome, evidence refs, affected artifact IDs and parent event. Record explicit `not_inspected`, `unknown`, `unverified`, `uncertain`, not guesses.

**Source precedence:** current host probe + genuine tool/CI receipts > unmodified source/decision records > verified persisted event > summary. User-approved project canon can supersede earlier decisions but cannot pretend a stale binary is currently available. External repo Markdown is imported as untrusted reference material.

**Export/compatibility:** `PROJECT_STATE.md`, `CHANGELOG.md`, `ENGINEERING_HANDOFF.md` are navigable human/AI on-ramps. Do not require projects to expose private local paths or secrets publicly. Local-only inventory belongs in user-controlled profile storage or an explicitly excluded workspace file. Git-hosted ledger exports must redact secrets and machine-private details by default.

**Do not conflate:** source commit, build result, installer package, user-installed executable, acceptance. Preserve artifact hash and external run URL/ID. Benchmarks require fixture/data version, measurement environment, methodology and actual result, not just a success label.

## Revised build order (existing eight stages kept, dependencies made explicit)

| Wave | Deliverables | Existing stages | Stop condition |
| --- | --- | --- | --- |
| A. Orientation & foundations | State/ledger and artifact discovery; object identity and revision-aware read API; internal typed capabilities; no AI bridge | 1 + 3, cross-cutting | New conversation finds exact source and current state; stale source is detected |
| B. Human editing hardening | Search/replace, file tree navigation, review/proposal, drafts/save/reopen and recovery; accessible UX | 3 | Current Windows app reliably edits/saves/reopens without unexpected source writes |
| C. Development truth | Read-only Git status/diff/worktree identity, local-vs-remote reconciliation; verified inventory/artifact locator and task context | 6 (read-only subset) + 5 (discovery-only subset) | Missing/changed compiler or package not called 'verified'; divergence surfaced |
| D. Execution boundary | Dedicated OS-supervised worker, hostile tests, quotas, permissions, durable receipts on Linux and Windows | 2 | Worker containment proved on intended OS; no fallback to unsafe execution |
| E. Programming operations | Isolated LSP, build/test/DAP; structured diagnostics and source-span links; supervised recipe selection | 4 | Native test evidence + cancellations + worker death recovery |
| F. Advanced workflows | Package manager, dependency provisioning, Git mutation, remote PR/CI workflows, multi-project sessions | 5 + 6 | Explicit authorization, reversible operations, artifact/CI evidence |
| G. Optional AI | Authorized Lu bridge + coding proposals + source/test feedback; disposable Lu Dev self-development | 7 + 8 | **Deferred**, independently gated and user-authorized; stable Lu remains unchanged |

The historical eight-stage checklist remains the official acceptance accounting. Wave ordering **does not** claim all of stage 2 is complete and does not permit running executable untrusted code before its native gates pass. Work on read-only Git or inventory is safe to start separately. Branch protection and actual CI for continuity remain unverified until demonstrated.

## Next three concrete coding slices

**A1 — Authoritative source-span read:** add a host Rust request/response for selected project/file/revision with 1-based line/column plus explicit byte offsets, bounded output, newline metadata, and stale-revision rejection. Test CRLF, blank lines, multi-byte Unicode, traversal/symlink denial, and different saved vs unsaved editor state (buffer ownership established separately). Current code editor UI can consume this incrementally.

**A2 — Bounded capability descriptors:** make a static, host-validated registry for *existing* read/review/stage/save operations. Test descriptors against commands, schema versions and effect class; no bridge, dynamic tools or auto-authorization.

**A3 — Project continuity read model:** represent current state, source and artifact provenance as host-owned read-only objects and define stable export. Probe and reconcile local worktrees and Git read-only. Never assume GitHub contains uncommitted local work. For the first slice, file-based ledger readback and validation can precede SQLite storage; avoid two competing canonical truths.

## Acceptance experiments

1. Restart the conversation without its chat history: project orientation reaches the correct branch, next task and last true verified artifact.
2. Produce the same source span in the UI and semantic API; an old revision yields explicit stale failure, not an edit on new bytes.
3. Unplug or change a remembered compiler: presence is not readiness; an old build does not become verified.
4. Keep unique local work after Git diverges; show both sides and require consent to reconcile.
5. Simulate incomplete build/package: report unknown/failed/uncertain, never “installer ready.”
6. Confirm no Lu process, IPC bridge or AI runtime is needed for the human IDE workflow.
7. Test unauthorized operations and unexpected worker death when those services are implemented.

## Decision log and anti-goals

- **Preserve** working R3.x source editor and proposal safety; refactor behind contracts incrementally rather than rewriting the GUI or replacing CodeMirror.
- **Do not** build a second general-purpose shell that guesses commands from AI text.
- **Do not** require Lu Core/ANSF operational-memory code as an IDE runtime dependency; adapt conceptual contracts independently.
- **Do not** claim human click paths are equivalent to semantic host operations if they bypass validation.
- **Do not** require always-on telemetry or leak local resource paths to GitHub by default.
- **Do not** treat this design as shipped code or change the historical R3.x acceptance labels.
