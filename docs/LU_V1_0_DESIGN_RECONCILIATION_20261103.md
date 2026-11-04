# Lu v1.0 design reconciliation and implementation gates — 2026-11-03

## Sources and precedence
Reviewed original `Jan-Mod-Design-Document.md` (Oct 21–22), Lu Core `DESIGN_GAP_AUDIT_2026-10-25.md`, `MEMORY_AND_MEDIA_ROADMAP.md`, independent desktop docs and subsequent Lu decisions. The old Jan document is historical, **not automatically a current requirement**. The accepted changes below supersede it:
- Independent Tauri/React app + Rust Lu Core, pinned llama.cpp backends (Vulkan first). Jan data import only, not Jan runtime.
- **Owner-confirmed 2026-11-03:** Lu has broad native host access, but no independent authority to act. OS permissions and tool availability are not task authorization. Mandatory sandboxing was not a design option; use authenticated per-turn delegation, operation-specific review and recoverable effects.
- Native first-party tools with Protobuf host/worker contracts; MCP optional.
- ANSF + SQLite, immutable original conversation records with revision/link/provenance, mutable semantic/object memory with user inspection.
- Single installer; Github repository only; Windows physical platform first.
- Cognitum optional addon. Domain-specific IDE/Film/Research/Writing DLCs are after core v1.0, not prerequisites.
- Alpha `0.1.0-alpha.2` accepted as functional development baseline; known defects tracked separately.

## v1.0 capability gates (do not equate 'implemented in library' with 'integrated and accepted')
| Gate | Scope | Objective acceptance |
| --- | --- | --- |
| V1-01 | Windows shell and distribution | Taskbar/app icon and identity; responsive model load; measured import progress; installation/upgrade/uninstall preserve data; signed distribution plan; Win10/11 physical tests |
| V1-02 | Model runtime and profiles | Pinned engine verification; GGUF+MMProj import/load/unload; Vulkan GPU-layer and memory diagnostics; recoverable failures; per-model inference profiles; model catalog |
| V1-03 | Durable conversation and ANSF | Immutable originals, editable revision relationships, chat search/folders, image/document refs and recoverability; restart/replay; export/import/backup/migration |
| V1-04 | Native tool host authorization | Trusted persisted human turn identity; scoped grants/expiry/revoke; typed small model-facing schemas, Protobuf boundary; untrusted outputs never grant new authority |
| V1-05 | Native worker lifecycle and effect safety | Windows process start/stop/cancel/reap, Job Object cleanup where applicable, resource protection and crash recovery; no mandatory sandbox or restricted token. Authorization checked separately at action boundary |
| V1-06 | First-party toolset | Read/search/write with atomic file effects, safe move/rename/trash/restore, git, archive, validated JSON; optional web/tool adapters individually gated; no MCP required |
| V1-07 | Journal and agent orchestrator | Per-tool prepared/applied/uncertain receipts; idempotency and crash reconciliation; bounded loops; checkpoint/restart/cancel; long-running task progress and visible tool cards |
| V1-08 | Memory and context | ANSF searchable source-to-entry references, token-budget retrieval, provenance, working facts and object relations, contradictions and belief revision; editable memory UI |
| V1-09 | GUI and quality | Stream stability, per-chat FIFO, soft interrupts/steering at safe boundaries, diagnostics, performance, accessible controls, import and file previews, UX regression |
| V1-10 | Independent E2E acceptance | Fresh install→model→chat+image→authorized native tool→restart recovery→backup/restore→uninstall; adversarial prompts, corrupt journal, missing files and worker death tested |

## Recommended sequence
**Foundation (first): V1-01, V1-02, V1-03.** Fix taskbar resources, instrument actual load/host responsiveness, import progress and thumbnail rehydration. Protect the accepted baseline with build IDs and evidence hashes. Integrate ANSF with existing chat persistence without breaking old history.

**Native abilities: V1-04, V1-05, V1-06.** Prove direct host tool execution with authenticated user-turn authority, then native file mutation with verified targets, atomic journal and rollback/recovery. No artificial sandbox-only access model. Add archive, git, search and JSON tools behind the same authority boundary.

**Agency and continuity: V1-07, V1-08.** Wire durable task supervisor and user-viewable memory. Verify real task recovery and accurate retrieved context, not just standalone library unit tests.

**Stabilization: V1-09, V1-10.** Physical Windows installer acceptance, performance, security and failure injection. Close remaining known defects or document a deliberate scope reduction before labeling 1.0.

## Scope boundaries
**Core v1.0:** offline local model conversation, image/document handling, native host-controlled tools, memory, continuity, diagnostics, task orchestration, robust distribution.

**Post-v1.0 / DLC:** IDE self-development workspace, film/NVP, advanced research/writing workspaces, multi-model council, full streaming video/audio processing, extensive theme packs, self-updating development instance. Define APIs now but do not block core release.

## Technical risks / explicit rules
- WebView/UI can never forge the authenticated user turn or directly choose privileged file paths or engine binaries.
- Per-turn tool discovery is not authorization; effect grants must match actual target/arguments and remain revocable.
- A model-generated plan or imported chat has no authority; user permission only.
- Worker process lifecycle and resource control require OS-backed tests, but **are not** Lu's authorization boundary. Mandatory sandbox/isolated account restrictions are superseded by broad host access plus authenticated effect-specific authority.
- Unknown crash outcome is **UNCERTAIN** until state reconciled; never blindly rerun a non-idempotent write.
- Progress percentages must be grounded in measured bytes or engine progress, never wall-clock extrapolation.
- UI freeze remains an observed defect; root cause needs traces rather than assertion that the UI thread is blocked.
- Keep evolving schema migrations backward compatible with baseline alpha.2 attachments and chats.

## Immediate prioritized backlog
1. Win10 taskbar/shortcut icon: PE `RT_GROUP_ICON`, executable resource, AppUserModelID / installer shortcut identity.
2. Windows load freeze: worker boundaries, event loop watchdog, llama log capture, GPU/VRAM monitor.
3. GGUF import: streamed bytes and destination, atomic finalize, cancellation and error.
4. Image preview: asynchronous rehydrate and retry placeholder, durable content-addressed attachment references.
5. ANSF/SQLite desktop integration and backup/migration smoke.
6. First native read-only tool via trusted human-turn verifier and native host executor; test out-of-workspace user-authorized paths while refusing unauthorized effects.
7. Durable writes, effect journal and restart reconciliation.
8. Model context and memory UI with provenance and belief revision.

## Owner correction — access, permission and authority (2026-11-03)
Lu is not sandbox-first. Lu may access the actual Windows host to find and use compilers, SDKs, files, network and ordinary desktop tools. OS **permission** means technically able; task **authority** means the authenticated user has directed Lu to perform the operation. Only the second justifies action. Do not use AppContainer or workspace-only storage as a mandatory design requirement. Retain process supervision, user approval for high-impact operations, explicit effect auditing and rollback. See [canonical authority decision](../src-tauri/lu-core/SECURITY_AUTHORITY_AND_OS_CEILING.md).
