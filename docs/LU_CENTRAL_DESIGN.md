# Lu — Central Product and Technical Design
**Canonical edition:** 2026-11-03  
**Status:** Binding requirements and implementation map; NOT a statement that v1.0 is complete  
**Project:** Independent general-purpose local AI desktop assistant, Windows-first

> **Access is not permission. Permission is not authority.**
> Lu can operate on the native host; authority to act comes from the authenticated user and remains bounded by the delegated task.

## 0. Authority of this document and evidence
This is Lu's **single entrypoint design specification**, consolidating the recovered decisions from the `ChatGPT - Lu Convo xx` lineage (through Convo 21) and the current tracked engineering sources. It **does not claim** a verbatim reconstruction of every old message. Read this document before implementation; the task tracker, test evidence, and original design/decision records provide details.

**Conflict resolution:** latest explicit owner decision > later confirmed design decision > this reconciled specification > contemporaneous engineering acceptance evidence > historical proposals. A new owner decision must update this file and the relevant tests/task board. Implementation code is evidence of behavior, not permission to change the product goal. Never silently reintroduce superseded Jan behaviors or sandbox requirements. Mark any uncertainty as such.

**Key supporting records:** `LU_CANONICAL_HANDOFF_CONVOS_01_21_20261103.md`, `LU_V1_0_DESIGN_RECONCILIATION_20261103.md`, `LU_V1_0_TASKS.md`, `LU_ALPHA_01_ACCEPTED_BASELINE_20261103.md`, `LU_R22_STRUCTURED_CHAT_AND_ALPHA_V1_ROADMAP.md`, `src-tauri/lu-core/SECURITY_AUTHORITY_AND_OS_CEILING.md`. Earlier `Jan-Mod-Design-Document.md` / October audits are historical where they conflict.

## 1. Product intent and v1.0 boundary
Lu is a **single locally installed general-purpose AI assistant**, able to use locally installed LLMs and the Windows computer's actual programs and files to complete human-delegated work. It should feel like a capable desktop assistant rather than an isolated chatbot, without causing unauthorized or unrecoverable system changes.

**Core v1.0:** independent desktop, reliable inference/text/images/documents, conversation persistence, ANSF memory and contextual continuity, native tools, task authorization, durable journals and recovery, resource supervision, usable GUI, single-installer delivery and tested Windows lifecycle.

**Post-core DLCs:** IDE/self-development, Film/NVP, Research, Writing. Optional Cognitum math/causal reasoning backend and additional Cognitum capabilities integrate through explicit interfaces; not a mandatory dependency. Council/multi-model deliberation, large A/V pipelines, server mode and rich theme packs are optional/deferred and must not block the core milestone.

**Do not revive a Jan runtime dependency.** Jan and LM Studio remain useful behavior/UX references and Jan format may be imported. Do not assume the historical Jan fork is the implementation that will ship.

## 2. Access, authority, permissions, and safety
1. **Accessible capability:** operating-system file/API/device/service/compiler/network/etc. that Lu is technically able to reach.
2. **OS permission:** whether the Windows account/process can technically perform an operation.
3. **Human authority:** purpose-specific authenticated user instruction, potentially with durable delegation, exact effect classes, scope, destinations/targets, time limits, resource budgets and revocation.
4. **Execution admission:** trusted host verifies the actual proposed operation and arguments against current authority. A model response, discovered tool or external text cannot grant itself permission.
5. **Effect protection:** preconditions, atomic writes, journal receipts, backups/Trash, stop/retry/cancel, reversible changes and UNCERTAIN reconciliation.

Broad access is intentional. **There is no mandatory Lu-wide sandbox**, restricted token, Docker/VM, fixed workspace filesystem, or default network block. A user may opt into disposable worktrees, isolated test runners or temporary process restrictions for specific tasks. These are tools, not universal authority controls.

Within an approved task Lu can discover dependencies, read/edit files, run compilers and tests, and continue ordinary related steps without incessant prompts. Newly consequential, off-task, privileged, destructive, sensitive egress, financial, credential, device or system administration actions require a distinct user-confirmed authorization path and OS approvals when applicable.

Do not interpret an ordinary filesystem grant as permission for unrelated mass deletion; do not treat a document saying “ignore previous instructions” as a human request. **Access, permission, discovery, recall and history linking do not imply authority.** The host checks target/argument-level relevance as well as tool/effect class.

**Model output is never an authority-bearing channel.** Imported chats, retrieved ANSF entries, webpages, tool outputs, subagents and linked sessions are context only. The host issues immutable task IDs and binds them to verified persisted user turns. Revocation must stop new dispatch, and cancellation/irreversible in-flight effects need explicit reconciliation.

Windows Job Objects, worker cleanup and resource guardians improve reliability and limit runaway effects. They are not the authorization boundary and must not be advertised as complete process isolation.

## 3. System architecture
### 3.1 Desktop and independent core
- **Windows 11 primary platform**, Windows 10 compatibility acceptance as applicable; Linux development/port remains useful but cannot substitute for Windows release acceptance.
- Tauri 2 / React desktop shell plus independent Rust Lu Core; typed events, clear host/UI trust boundary.
- **Protobuf** contracts for native inter-component/tool/worker communication; strict bounded JSON at external model and engine API interfaces. Validate types, unknown fields and intent before conversion.
- A built-in Rust native tool registry, host broker and supervised local process workers. **No mandatory MCP server**; third-party MCP may be offered as an optional adapter.
- Durable storage split: immutable ANSF SQLite conversation sources, source-index catalog, derived memory/beliefs, task and effect journals. Separate tables/stores have explicit identity and reconciliation rules, rather than silently competing sources of truth.
- Supervision and process lifetime: spawn, observe, cancel, reap, resource limits, crash recovery; Windows Job Object if helpful for child cleanup, not mandatory sandboxing.
- Model backend manager, user-visible state and diagnostics, and a capability inventory for installed tools and optional provider adapters.

### 3.2 Model runtime and hardware
- Local GGUF loading with an explicit compatible **MMProj** for vision; no silent downgrade of images to text-only generation.
- llama.cpp native engine; Vulkan AMD GPU path on the tested Radeon AI PRO R9700; CPU and future CUDA/ROCm paths are capability-driven, verified and measured. No unverified layer-offload claims.
- Distinguish import, model admission, load start, engine PID, health, readiness, failure, cancellation, and unload. Progress percentages only from engine-measured progress; import percentage only from actual byte counts.
- Typed chat history (roles) into structured chat completion protocol; preserve instruction/assistant turns rather than flattening into fake `User:` / `Assistant:` text. Stream tokens and durable partial checkpoints.
- Model configurations, token/context budgets, GGUF+MMProj binding, per-model folders and safe load/unload/restart; model ability and installed capability must be reflected honestly in UI.
- Support alternate local models and backends when available without changing authority rules. Do not require multiple active large models by default.

### 3.3 Native tools, provisioning and execution
- First-party filesystem (read/search/write/rename/move, trash/restore, atomic files), Git, archive/compression, JSON/schema validation/repair, OCR, optional terminal/code runner and other installed tools; staged independently with acceptance tests.
- Native tool identity, schema and discovery are **inert** until human-task authorization + host policy approve exact effect/target/arguments. Protobuf-generated request metadata and model-visible small JSON schemas never constitute a grant.
- Host capability inventory discovers installed compilers, SDKs, PowerShell, Python, Git, portable binaries, WSL and local services on the *real OS*. When missing, Lu reports precise requirements, checks provenance/version/signature/hash and can provision the appropriate user/system dependency only within approved effect scope.
- Prefer single-click installer and native dependencies; no forced Docker, shell setup or repeated manual dependency transfer. User approval is required for newly privileged installs or changes outside the delegated task.
- Tool execution, cancellation, retries and failures must use a bounded loop and durable statuses: PREPARED, EXECUTING, APPLIED, FAILED, UNCERTAIN. Never blindly replay an ambiguous non-idempotent effect.

## 4. Memory, continuity and context architecture
### 4.1 Canonical ANSF storage
- ANSF/SQLite persists **exact immutable original messages**, ordered chapters/source IDs, role, revisions/edited links, tombstones/privacy controls, file/image/document refs, and provenance. An index/library may be rebuilt from canonical originals.
- Working facts, object relationships, interpretations, summaries, beliefs and contradictions are **derived and editable**; provenance connects them back to immutable source, with belief revision/supersession rather than silently overwriting history.
- Cross-chat source references and context transclusion are supported only with authenticated read permissions and explicit source attribution. Moving context is not transferring task/tool authorization.

### 4.2 Context and agent continuity
- Store intent, assumptions, decisions, completed work, blockers, next action, source-entry links, task and effect receipt IDs, revision history and state. Persist before long tasks/hand-offs and retrieve after crash/restart.
- **User is not the conversation transport layer.** Lu should retrieve relevant past work, browse provenance and reconstruct task state without requiring 21 manual recap prompts.
- Keep an explicit distinction among *source/quote*, *derived summary*, *current belief*, *approved goal*, *task authorization* and *effect receipt*. Source text, retrieved checkpoint or old instruction cannot renew authority.
- Retrieval is bounded by model token budget and evaluated for accuracy, latency and source fidelity. “Infinite memory” means durable storage plus bounded retrieval—not infinite model context.
- Memory-pane UX: search, browse original sources, show links/revisions and contradictions, edit current beliefs, make primary, link selected old chat content to current project with provenance.
- Existing `ContinuityStore` is the checkpoint implementation: conversation/project/task-scoped lookup, validated ANSF source-entry IDs and immutable checkpoint identity. Production desktop adapter, owner checks, full context projection and activation remain incomplete.

### 4.3 Context change monitoring and observing/learning
- **CCM (context change monitoring):** track changes to declared goal, scope, assumptions, sources and constraints. A shift in user intent prompts an authority check, not silent reuse of an earlier grant.
- **Observe & Learn is OFF by default** until an explicit user consent flow enables it. Any persisted observation must have provenance and accessible user controls. Transient raw capture should be discarded after the approved encoding/derivation step unless the owner specifically authorized its storage. Never infer broad permanent authority from enabling observation.
- Search/summarization ranks sources by recency, source fidelity, topic and the current project; conflicting remembered decisions should be visible instead of silently replaced. The final model working set is only a projection of durable source records.
- Edits and cross-chat links must preserve parent/source identity and distinguish exact old text, corrected current text and derived interpretation.

## 5. Durable agents, workers, and Lu Dev
- Multi-step tasks have host-owned identities, checkpoints, visible progress, timeouts, pause/soft-interrupt/cancel, replan upon real goal or assumption change, recovery after app restart and worker death.
- Worker manager has resource guardian (CPU, RAM, GPU/VRAM, disk, concurrency), bounded queues, actual OS process cleanup and clear cancellation semantics. Idle/prewarm behavior, if added, is measured rather than assumed.
- No auto-resume of unknown/destructive effects until verified against journal receipts and the user's current authority. Task revocation immediately blocks subsequent effect admissions.
- Optional Lu Dev (later IDE DLC) may share the model context or infrastructure while developing its own source in a disposable worktree, run tests, produce reviewable diffs and build artifacts; replacement of stable Lu requires independent approval, verified promotion and rollback.
- Maintain separate notions: execution **access**, action **authority**, process **lifetime supervision**, and optional per-task **isolation**.

## 6. Desktop GUI and interaction
- GUI-first; user text and assistant responses visually distinct, full-width readable chats, collapsible thinking/status areas, model-picker/state and optional detachable or resizable input.
- Left pane: new chat, searchable history, folders/favorites and linked context. Reply actions for copy, retry, editing, quoting/transclusion and visible provenance.
- Model-loading widget uses actual phases, elapsed time and file size where grounded; spinner must animate, no fictitious percentage. Input remains usable while output streams; switching chats must not erase ongoing streaming or cross-chat queued work.
- Cross-chat FIFO: each chat retains its own generation ID, queue position, streamed buffer and cancellation. Soft interruptions/steering are accepted only at safe supervised boundaries; no reassigning output to another chat.
- Attachments: paste/drag PNG/JPG according to actual decoded format, documents and screenshots, durable preview and references, async refresh/rehydration. Vision history should reuse image references when supported or report why not.
- Output may be Markdown/code/structured with safe escaping and without the accidental giant-blob wipe on an engine error.
- User-visible statuses distinguish received, processing, applied, cancelled and uncertain; cards must reflect real events, not optimistic UI.
- Model loader belongs in settings/model manager; hide misleading debug-only panels by default.
- Token/context counter, read receipts and resource status are useful but must use measured data.

## 7. Packaging, workflows, and tests
- Windows NSIS single-click installation first; test icons in taskbar/PE/shortcut and AppUserModelID, WebView2 prerequisites, proper upgrade and uninstall preserving data.
- Pin/verify engine binaries and dependent DLLs; **build-only dummy files are not distributable**. Verify Vulkan load, model response, MMProj image QA and GPU offload on real Windows hardware before release.
- GitHub is source + CI, **not the development hub**. Develop on local source when possible. CI must verify the exact committed revision; do not claim its checks cover an unpublished local source ZIP.
- Accepted **`0.1.0-alpha.2`** remains immutable baseline after observed physical Windows tests. Current later source is a development branch, not implicitly part of that accepted build. Owner has deferred repeated physical testing until nearer v1.0.
- CI layers: source manifest, TypeScript/UI, Lu Core Rust, portable chat/ANSF/auth/diagnostics, Windows host compile, engine process tests, fault injection. Full release acceptance only with physically installed app and real hardware.
- Log evidence per gate: commit, source/engine hashes, test command/platform, pass/fail/skipped, known defects, and deferred manual scenario. **Do not label queued, cancelled, compile-only or mock-based jobs as PASS.**

## 8. Release gates and sequencing
| Gate | Objective | Present status |
| --- | --- | --- |
| REPO-01 / CI-01 | Exact published/source comparison; green CI on committed full tree | GitHub manifest verifiable; external unpublished source equality open |
| V1-01 GUI/installer | Responsive Windows shell, icon, import, install/upgrade/uninstall | Alpha accepted; refinements open |
| V1-02 Runtime | GGUF, MMProj, Vulkan, process status, profiles | Alpha accepted; full v1.0 evidence open |
| V1-03 DATA | ANSF + durable chat/media/doc, migration/revisions | Core pieces; production integration incomplete |
| V1-04 AUTH | Persisted verified USER origin, scoped approvals, revocation | Tests/foundation; live approval-dispatch incomplete |
| V1-05 LIFECYCLE | Worker/process supervision, cleanup, resource guardian | Partial |
| V1-06 TOOLS | Useful native read/write/search/Git/archive/JSON and provisioning | Read foundations; full native suite incomplete |
| V1-07 JOURNAL/AGENT | Effect receipts, durable orchestration, restart recovery | Core pieces; end-to-end open |
| V1-08 CONTEXT/MEMORY | Search, citations, beliefs, cross-chat continuity, handoff | Foundations and design; UI/automatic retrieval open |
| V1-09 UX | Streaming, queuing, attachments, accessibility, diagnostics | Partial |
| V1-10 RELEASE | Full Windows installed-app and adversarial E2E | Not accepted |

**Order:** Preserve alpha -> converge exact repo source and CI -> authorized native read-only GUI tool -> durable effects and host tools -> ANSF + context UI integration -> agent orchestration/worker recovery -> model, media and UX hardening -> Windows v1.0 E2E release acceptance.

### 8.1 Roadmap source reconciliation (R22 → owner-accepted alpha → v1.0)

This is the **operative v1.0 roadmap**, not merely a link to another file. It incorporates:
- `docs/LU_R22_STRUCTURED_CHAT_AND_ALPHA_V1_ROADMAP.md` (also present in Library), originally written before the alpha acceptance decision;
- `docs/LU_V1_0_DESIGN_RECONCILIATION_20261103.md` (V1-01 through V1-10 acceptance gates);
- `docs/LU_V1_0_TASKS.md` (live P0/P1 engineering ledger with deferred Windows tests);
- `docs/LU_ALPHA_01_ACCEPTED_BASELINE_20261103.md` (owner acceptance of the functional alpha).

**Status precedence:** The Nov 3 owner acceptance of `0.1.0-alpha.2` supersedes R22's earlier statement that functional alpha had not been accepted. It does **not** waive remaining v1.0 hardening, especially installed-app tool authority, backup/restart, verified GPU offload, signing and failure injection. The former alpha A5 native-tool and advanced worker-recovery obligations are tracked as v1.0 core gates rather than retroactively invalidating accepted alpha. Older default-deny sandbox/containment proposals are superseded as universal requirements; verified task authority and worker lifecycle safeguards are retained.

### 8.2 Phased execution roadmap

Each phase has concrete exit conditions; units and docs alone cannot close an end-to-end gate.

| Phase | Priority and dependency | Deliverable | Exit/acceptance evidence |
| --- | --- | --- | --- |
| 0. Baseline and source convergence | P0; prerequisite to claims about built source | Freeze accepted alpha.2; reconcile unpublished alpha.3-dev source snapshot; source hash/manifest, exact branch and commit; remove old CI paths | Full source comparison, immutable baseline artifact identities, CI on exact source revision; unmerged or missing local files explicitly reported |
| 1. Windows UX and model reliability | P0/P1; can run alongside phase 0 | Taskbar/PE icon, responsive loading and watchdog diagnostics, measured GGUF import, catalog/MMProj binding, thumbnail rehydration, clear loader status, per-model inference profile | Automated frontend + Windows compilation and instrumented lifecycle tests; physical Win11/Vulkan/model QA deferred to release |
| 2. Durable conversation, ANSF and media | P0; prerequisite for trustworthy replay/context | Existing chat journal migration to ANSF immutable originals; SQLite indexes, revisions, source provenance, attachment refs; PDF/DOCX/text parsing, backup/import/export, cross-chat read mediation | Restart/crash and migration/rollback fixtures; original IDs/text/media retained, citations source-correct; no legacy chat silently lost |
| 3. Human-authenticated native tool execution | P0; independent of model output | Current persisted USER turn + trusted task origin; host-issued tool/effect/target grant; expiry/revocation; model-visible small schemas and Protobuf; **first desktop read-only tool** with approval and denial UI | Real app dispatch; arbitrary user-authorized host path works; unrelated chat, injection, wrong target and revoked task fail; no model-granted authority |
| 4. Native worker and practical toolset | P0; builds on phase 3 | Process lifecycle and resource guardian; native read/search/atomic write/move/trash/restore, Git, archive, JSON validation, OCR and useful document tools; capability discovery for installed SDKs/compilers | Real tool invocation and per-target admission; atomic/reversible changes, command cleanup; no mandatory sandbox; OS permission distinct from authority |
| 5. Durable effects and agent continuity | P0; phases 2–4 integrate | Journal PREPARED/EXECUTING/APPLIED/FAILED/UNCERTAIN, idempotency receipts; work queue, cancellation, pause/soft interrupt, progress, bounded agent loops, checkpoint and recovery | Forced crash/restart and worker-kill matrix; uncertain effects reconciled, never blind repeated writes; meaningful task resumes with no authority inheritance |
| 6. Memory intelligence and context UX | P0; phase 2 and 5 foundations | ANSF library search; bounded token-aware source retrieval, cross-chat references, mutable beliefs/objects and supersession, user-editable memory pane, inspectable provenance, continuity plane and context change monitoring | Search/retrieval accuracy, citations, contradiction and revision tests; auth-scoped read, restart resume, no old chat acting as a fresh user grant; Observe & Learn explicit opt-in |
| 7. Production UX stabilization | P1; ongoing alongside all phases | Accessible interface, cross-chat FIFO, streaming partial checkpoints, model switching, attachment paste/drag, document/image history, responsive busy states, tool cards and read/processed/applied receipts | UI automated and soak tests; cross-chat cancellation attribution, no lost partial text, no false status or preview downgrade |
| 8. Windows release candidate and v1.0 sign-off | P0; depends on all required gates | Signed/reproducible NSIS, pinned engine binaries/DLLs, fresh install/upgrade/uninstall, full real-model and AMD Vulkan/MMProj tests, backup/restore, permission/effect recovery | Independent installed-app E2E: install → import/load model → text+image → authorized tool → restart/restore → effect replay/cancel → uninstall; negative and adversarial tests pass |

**Engineering order for the current branch:** correct any locked CI build errors → finish Phase 3 read-only GUI authority/dispatch → Phase 4 atomic operations + Phase 5 journal → Phase 2/6 ANSF and context integration → finalize Phase 1/7 reliability → Phase 8 physical release. This preserves dependencies without unnecessarily serializing independent work.

### 8.3 Original R22 capability list retained as explicit acceptance work

- **Memory:** ANSF immutable exact archive, searchable chapter/library, revisions and source links, bounded retrieval and current beliefs. An internal SQLite table is not equivalent to the memory-pane UX.
- **Media continuity:** content-addressed image references surviving restart; safe preview rehydration; actual model/MMProj-compatible image Q&A and follow-up without pretending old images were sent. Preserve document source identity and extraction limits.
- **Document intake:** PDF/DOCX/text parsing with provenance, bounded extraction, source/chunk references and negative security cases. Plain Markdown success does not establish arbitrary document support.
- **Model manager:** explicit load/unload/catalog, multiple model folders and MMProj pairing, real Vulkan/CPU profiles, physical offload evidence, resource usage and safe failure handling. No automatic weight loading merely because files were discovered.
- **Native operations:** filesystem, search, Git, archive, validation, JSON, OCR and command execution; first-party native registry and Protobuf. Discovery never grants effect authority.
- **Background agent:** durable tasks, progress, pause/stop/retry, cancellation propagation, worker/process supervision and no blind replay after uncertain effects.
- **GUI:** rename/delete/restore chats and folders, favorites, full text/image/document history, accessibility, virtualized large transcripts, tool/provenance cards, theme/input preferences; no Jan runtime inheritance.
- **Security and quality:** current broad-host-access/human-authority contract, true owner-origin verification, invocation target review, crash/resource safety, audit and revocation, adversarial tests. Mandatory AppContainer or universal network default deny are **not** adopted.
- **Release:** single installer, reproducible pinned native engine, real-model benchmark matrix, measured GPU/CPU, migration and rollback, installed-app acceptance.

### 8.4 Deferred and intentionally non-blocking
Advanced JSON repair (distinct from strict JSON validation), model councils, large real-time video/audio, DLC IDE/NVP/Research/Writing, optional native image generation, remote/server mode and theme ecosystems remain **after core v1.0** unless explicitly re-scoped by the owner. No arbitrary 100% model-loader indicator may replace measured engine progress.

### 8.5 Change tracking
Live statuses and receipt IDs are maintained in `LU_V1_0_TASKS.md`, with exact source/CI evidence and deferred Windows scenarios. This central design owns **requirements, dependencies, phases and exit criteria**. Roadmap status must be updated here when the owner changes a feature's inclusion or stage; routine per-commit CI updates stay in the task ledger. All future Lu sessions should read this section before starting new work.

## 9. Explicit supersessions and deferred items
- **Superseded:** mandatory AppContainer/restricted-token/workspace-only sandbox; Jan runtime inheritance; blanket claim that an OS permission equals authority; forcing every operation to ask approval despite explicit task delegation.
- **Deferred:** advanced JSON repair loop (strict validation remains mandatory), council, large A/V pipeline, DLC workspaces and self-modifying stable installation, arbitrary fake load-percentage telemetry.
- **Not a product requirement:** the assistant's own Debian environment or external tool availability restrictions. Lu must discover and use host capabilities under delegated authority instead.

## 10. Engineering change control and handoff
1. Read **this file** and `LU_V1_0_TASKS.md` before touching Lu source. Search prior context only for facts not captured here.
2. If new owner design conflicts, edit this canonical file and its affected implementation/acceptance docs in the **same change**; mark old statements superseded. Avoid design drift through multiple competing RFCs.
3. Distinguish **decided**, **implemented**, **CI verified**, **Windows physically verified**, and **deferred**. The docs must say which.
4. Keep `0.1.0-alpha.2` immutable. Work on the active development branch and commit source+tests+tasks+lock.
5. No agent should manufacture project history from its own suggestions. Attach source refs to factual recovery claims and clearly label unverifiable historical details.
6. Continue with the first currently testable gate, currently V1-04 AUTH/UI dispatch and V1-08 continuation; do not let the documentation task stall implementation.

This design is the authoritative product-level map; detailed protocols remain in linked specifications and source code.

## 11. Current CONTEXT-01 implementation receipt (2026-11-03)
Added `ContinuityStore::latest_for_verified_user_turn` so the trusted desktop host can require a verified current user turn before looking up a checkpoint by exact project/task/conversation. A remembered checkpoint is only context; it is never an authority grant. Updated regression tests reject stale/quoted instructions and cross-chat impersonation. This code is **not yet connected to desktop UI or automatic retrieval**, and CI acceptance of this commit remains to be established.

## 12. TOOLS-01 milestone — first real native desktop file inspection (2026-11-03)
- Added an explicit `Inspect file` desktop action. The trusted host obtains the file via a native OS picker; the WebView/model cannot supply a path, scope or effect grant. This action works without loading an LLM.
- The native host persists a new authenticated user turn, opens an exact `lu.files.read_text` authority and separate policy grant, dispatches the Protobuf file read through Lu Core's registry, immediately revokes the task, and persists the answer in chat history.
- The current text adapter reads up to 64 KiB from the **selected file only**. The root is selected per call and **is not** a universal Lu application sandbox.
- Portable integration tests cover read correctness, restart persistence, duplicate turn denial, other-conversation isolation and bounded read. This is not a general model-driven tool-approval flow, and complete CI / installed-app acceptance remains outstanding.
- Next: verify the committed revision, repair CI, implement reviewable model-proposed exact-target read grants, then effect-journalled atomic writes.

## 13. AUTH-02 model-proposed read approval (development, 2026-11-03)
- A successfully completed LLM reply containing one strict `propose_read_file` object may create an **inert pending request**. It does not automatically authorize or execute anything.
- The trusted Rust host stores the proposal bound to its actual originating chat/generation; the desktop shows a review card listing purpose, exact filename and bounded maximum bytes. Approval opens the native OS file picker; mismatch, cancellation, wrong chat/generation, repeated approval and malformed JSON fail closed.
- A separately created authenticated human approval turn dispatches the existing native Protobuf read. Reads never exceed the specific proposed byte budget (64 KiB hard cap); the temporary grant is revoked. Model text never conveys independent tool authority.
- Tests are added in the portable desktop-auth harness, but full GitHub Rust/Windows CI and installed-app acceptance are **not yet established**. This is not a claim of automatic schema-advertised model tool calling, transactionally durable proposal queue, race-free file pinning, or full authorization for write/execute/network effects.
- Next: verify and repair CI on exact revision, pin native resource identity and durable approvals/receipts, then add the schema-advertised typed model tool loop and broader native operations.

## 14. JOURNAL-01A — durable decision and native read effect records (development, 2026-11-03)
Lu's read-only tool now has **two independent durable traces**: a host-owned approval-decision ledger and a Lu Core tool-effect journal. The approval ledger stores exact model proposal origin, chat, filename, max bytes and atomic decision transitions. It is not itself an execution grant. The effect journal separately stores prepared, executing, applied, failed or uncertain native Protobuf calls, keyed and fingerprinted to reject mismatched replay. Approved read execution still requires a fresh verified human task and an exact host policy grant.

Neither the old model instruction nor a persisted approval decision is automatically renewed on restart. Until Lu has single-instance/exclusive database ownership and effect-specific reconciliation, pending approvals are not restored to an executable queue and unfinished calls cannot be blindly replayed. The durable read path is opt-in at its Rust API and enabled in desktop native file inspection. This milestone does **not** close the wider durability requirements for atomic writes, worker termination, external consequences, reparse-point race pinning or end-to-end Windows acceptance.

Next gate: validate exact-revision Rust/Windows CI and implement exclusive startup recovery and robust resource identity before enabling write operations.

## 15. JOURNAL-01A reconciliation key (development, 2026-11-03)
A host-generated effect call ID is now durably linked to an approved model proposal via a unique database field. This makes it possible to correlate the human decision and subsequent `EffectJournal` state without interpreting either record as a fresh capability. Duplicate and cross-conversation effect bindings are rejected. Startup reconciliation still requires independent ownership verification and is not automatically enabled.

## 16. JOURNAL-01B — cooperative single-instance ownership and startup reconciliation (2026-11-03)
The trusted desktop now holds a process-lifetime SQLite writer transaction on a **dedicated ownership database** before opening user stores. This prevents a *second cooperating Lu instance with the same app-data directory* from wrongly marking the first one's live effects as interrupted. Once the prior owner has exited or crashed, the next instance may reconcile old `EXECUTING` effects as `UNCERTAIN` and old unapproved `PENDING` read proposals as `INTERRUPTED`. Neither state grants execution, and no blind replay is permitted. The held lock is lifetime-owned by `DesktopState`; the lock is separate from the regular chat/effect databases, which continue to serve the app.

This is not an AppContainer, mandatory filesystem sandbox, or an OS-wide exclusion mechanism for unrelated programs. Exact picked-file handle identity and all write/execute idempotency gates remain future work. Production/Windows evidence is tracked in `LU_V1_0_TASKS.md` on the exact source-locked commit.

## 17. TOOLS-01 approved native handle pinning (development, 2026-11-03)
The host-selected read-only file is now opened once and retained as an OS file handle through the native Protobuf dispatch. Tool arguments can identify only that selected filename; a path replacement *after handle acquisition* cannot redirect the read to a different file. The original cap-std read/list tools remain available for other explicitly authorized scopes, but model-discovered paths do not mint authority.

Tests cover post-acquisition path replacement on Unix and mismatch refusal on all platforms. The interval between operating-system picker return and acquisition of the pinned handle, especially Windows reparse-point behavior, remains a hardening gate; no broader filesystem guarantee is claimed until OS-specific handle-identity acceptance is green.

## 18. WIN-FILE-01 and WRITE-01 stage 1 (development, 2026-11-03)
A user-selected read-only file is now opened through a Windows no-follow flag with reparse-point rejection and a pinned handle, refusing final reparse-point substitution. The native read is pinned to that handle. On Windows, Lu allows other readers but prevents competing writers/deletion sharing during that brief inspection. Unix validates device/inode. This is distinct from a global host-access sandbox.

The first WRITE-01 implementation is a host-only, **create-new-without-overwrite** atomic primitive. It stages bounded bytes to a unique private sibling, syncs the staging file, atomically publishes with a no-overwrite hard link and removes the staging alias. Existing files are never replaced. An unexpected post-publication cleanup error is explicitly UNCERTAIN, not safe to repeat. Tests cover normal publication, SHA-256 receipt, existing-file preservation, staging identity collision, traversal refusal and byte bounds, with Windows CI execution.

**Acceptance boundary:** The primitive is **not yet registered as a model-callable native write tool**, has no committed task authority/effect journal path, assumes an already trusted private directory and supports only hard-link-capable filesystems. Handle-relative directory publication, crash reconciliation, rollback/recovery and user review must be proven before enabling arbitrary user-selected file writes. Original alpha.2 remains frozen.

**Stable Windows API clarification:** The current Windows read admits an ordinary handle with no-follow and reparse-point rejection. The Rust standard-library volume-serial/file-index metadata methods are not stable on the pinned toolchain, so cross-handle Windows identity comparison is **not implemented**. Native Win32 identity comparison remains a separate acceptance gate.

## 19. WRITE-01B — authenticated durable atomic create-new tool (2026-11-03)
Lu now has a separately registered first-party native tool `lu.create_new_file.commit` using existing `CeWriteRequest` / `CeWriteResult` Protobuf, the host-only no-overwrite creation primitive, and the trusted durable dispatcher. Registration is **discovery**, not approval. The trusted host must produce a verified persisted USER turn and task/effect/scope, then apply an independent exact-target SHA-256 policy for the requested filename and full byte payload. This binds the model-proposed bytes to what was actually approved; substitution and oversized content are rejected before journaling or dispatch.

On success, the effect journal records the typed Protobuf receipt; repeating the same call ID returns the receipt, never a second write. Revoked grants deny cached receipts, contradictory call IDs fail closed, and interrupted/ambiguous effects remain UNCERTAIN until independently reconciled. Tests cover these properties using Lu Core and the production desktop ChatStore verifier.

**Release boundary:** This code is an internal host/test integration milestone, not a user-visible write tool. A durable **write-specific approval record**, actual GUI diff/target review, host-selected/writable directory identity pinning, handle-relative staging/publication, transactional abort/rollback behavior and real Windows installed-app verification are still mandatory before exposing write or overwrite capabilities. Lu does not yet permit arbitrary model-initiated writes through the desktop.

## 20. WRITE-01C — user-reviewed, fail-closed draft creation (development, 2026-11-03)
Lu's completed model output can now **propose**, but never authorize, a bounded new UTF-8 text file. The initial user-facing operation is restricted to a dedicated **Lu Drafts** directory: no untrusted model path, user-selected arbitrary filesystem root, directory traversal or overwrite is accepted. The React review card shows **all proposed content**, exact filename, purpose, byte count and SHA-256, and requires the user to reveal the exact destination before proceeding. Final authorization is confirmed again through a native operating-system dialog with the exact path/size/digest. Closing or declining it records a durable denial, not permission.

The trusted host persists a separate write-approval ledger, connects it to a host-minted effect ID, then creates a verified current USER turn and dispatches through the existing typed Protobuf registry, strict target/digest host policy and durable `EffectJournal`. Only a new file may be created; the tool refuses overwrite. Unknown outcomes become **UNCERTAIN** and require explicit examination rather than blind retry. On single-owner restart, pending proposals are INTERRUPTED and cannot be resumed with old authority.

The desktop also exposes read-only write recovery: audit state from the approval/effect journals plus a bounded, pinned read of the app-owned target to distinguish missing, differing and matching bytes. A match is not conclusive historical proof of authorship or authorization. The model cannot invoke recovery to mint a grant.

**Acceptance is pending exact-source Rust, native-auth, frontend and Windows CI.** App data directory handle-relative publication, broader writable host access, comprehensive undo/rollback, security testing and real Windows installed-app sign-off remain future gates. This does not alter the accepted `0.1.0-alpha.2` baseline.

## 21. WRITE-01D — model-selected text format, human-selected location and durable reconciliation
**Intent:** Lu is a native AI assistant with broad host capabilities. The AI should reason about *which text filename and format are appropriate* for a task—e.g., `README.md`, `settings.json`, `analysis.py`, `main.rs`, `config.toml`, `notes.txt`—and propose complete, coherent content. This is distinct from **permission**: neither a model-selected extension nor a model-supplied path creates filesystem authority.

**Implemented development path for NEW files:** The model receives a narrowly triggered proposal-schema hint for explicit file-creation requests, emits `propose_create_file`, and the trusted host validates text format and proposed bytes (including JSON/JSONL syntax, with more specialized validators to follow). The user reviews all content, digest and target, selects Lu Drafts **or any other folder through the native OS folder picker**, then confirms the exact target in a second native dialog. A pinned OS directory capability is retained throughout approval and Protobuf dispatch; staging, atomic no-overwrite publication and cleanup are directory-handle-relative. One authenticated human-authorized operation creates at most one NEW UTF-8 file. This does not impose a mandatory sandbox on the rest of Lu.

**Reconciliation:** Durable approval and effect journals remain separate, joined by the host-minted call ID. Recovery now classifies those records alongside bounded target observations without treating a disk-content match as proof of execution or an old approval as fresh authority. External target paths are displayed for review but are not automatically rescanned/replayed. Every UNCERTAIN write must remain non-retriable without a newly verified, specifically authorized recovery action.

**Scope not yet achieved:** Existing-file replace/patch, multi-file project updates, compilation/linting for every language, semantic file-format conversion, true binary file production and unattended writes. These require source-hash compare-and-swap, exact diff review, backups/undo journal, cross-process conflict detection, and failure/rollback tests. Source-locked Rust/Windows CI and installed-app acceptance still apply.
