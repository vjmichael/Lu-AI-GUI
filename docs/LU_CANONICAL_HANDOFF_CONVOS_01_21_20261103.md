# Lu — Canonical Design and Engineering Handoff (Conversations 01–21)
Date: 2026-11-03
Status: durable handoff; not a declaration of implemented functionality

## Scope and evidence
This is the stable entrypoint for engineering continuation across `ChatGPT - Lu Convo xx`. Reconciled from recovered prior-conversation context, the current v1.0 design and task ledger, Lu R22 roadmap, alpha acceptance report, Lu Core design audit, and the actual development repository. **It is NOT a line-by-line transcription of all 21 conversations:** individual older messages may be missing. Before treating historical assistant assertions as verified, consult source, tests and CI.

**Precedence:** latest explicit project-owner instruction > later owner-confirmed decision > current canonical docs > historical proposals > unverified assistant claims. Do not restore a superseded requirement merely because it appears in an old file. All significant changes should be entered here with dated source and rationale.

## Binding design invariants

### Host access, OS permission, human authority
- Lu is an independent native desktop AI running directly on Windows. **Broad access** to the real host environment is intentional. **Permission is not authority**; technical access, a registered tool or a discovered command is not permission to execute arbitrary actions.
- The trusted host validates an authenticated human-origin instruction and its task lineage, capability/effect, requested target/destination, duration and revocation. A model never issues its own grant; retrieved webpages, linked chats, repository text, tool output and ANSF search hits are context, not authority.
- Lu should act autonomously *within* the authorized task: common reads, file edits, builds, compilers, tests, retries, local dependency handling and tooling should not cause endless approval prompts when already delegated. Materially different/high-impact effects get review. Executable discovery and provisioning must be possible without the user manually transporting every dependency.
- **No mandatory sandbox or Docker/container/VM.** Sandbox/throwaway worktree can be optional on a per-task basis. Windows Job Objects and resource guardian control process lifetime, runaway usage and cleanup, not task authority or a guarantee that an arbitrary process cannot change the OS. Privileged operations require a separately human-approved route.
- Authority must not transfer merely because a conversation is linked or searchable. ContextEnvelope / continuity plane and AuthorityGrant are distinct concepts.

### Software and runtime
- Independent Tauri/React Windows shell + Rust Lu Core; Jan is migration/reference material, not a required running subsystem. Local GGUF model manager, explicitly paired MMProj, pinned llama.cpp backend, AMD Vulkan acceleration and CPU fallback with measured diagnostics.
- First-party native tools and local workers using **Protobuf internally**; bounded strict JSON/JSON repair at model/external boundary, never JSON as a substitute for authenticated authority. MCP is optional. Worker manager, durable cancellation, restart recovery, receipt reconciliation and no blind replay of uncertain effects.
- Host-visible capability inventory should find installed programs and portable executables, SDKs and libraries; exact package/version/provenance for dependencies, native install options, user approval for consequences, and rollback.
- Model selection and tool schemas must be truthful to current capabilities. No automatic claim of GPU offload, working MMProj, loaded model progress %, successful task, or actual Windows physical acceptance based only on a test mock.
- Streaming outputs persist intermediate text; error/cancellation should not erase a previously visible answer. Cross-chat queue continues while user navigates other conversations. Never silently downgrade a multimodal request to text-only.

### ANSF memory and continuity
- ANSF+SQLite is the canonical durable source for immutable originals; linked revisions, provenance and user-visible mutable working/object/belief memory are separate derived layers. Database indexes are rebuildable; don't create competing authoritative ledgers.
- Link source entries across chats and include relevant historical context without elevating quoted, imported or retrieved text to a new human instruction. Retain contradiction/supersession and provenance; context window remains finite and retrieval is bounded/measured.
- Continuity Plane belongs in Lu Core: task intent, assumptions, checkpoints, tool receipts, handoff and restart-safe recovery. The owner should **never be the transport layer between agent sessions**. Cross-chat retrieval must preserve access checks at read time and cannot inherit credentials/authority.
- Memory UI, context search and inspectable source references are user features, not just an internal database.

### User experience and packaging
- Single-click Windows installer, no mandatory Docker/CLI/multi-installer setup. Optional terminal developer mode, GUI-first native tasks.
- Chat history, folders/favorites, reliable attachment/image/document persistence and preview, full-width/clear speaker bubbles, movable/resizable input, model status and load progress when measured, accurate read/processed/applied receipts, prompt queue and soft interruption, resource guardian and activity trace.
- Accepted **0.1.0-alpha.2** functional Windows baseline is frozen. No routine physical retests during v1.0 development; use GitHub CI to test the committed revision, preserve a deferred Windows E2E acceptance ledger. Do not claim source identity with unpublished local alpha.3-dev archive until exact comparison is complete.
- GitHub is repository and CI verification, not the user's development hub. Local development is preferred; do not block work solely because the hosted environment is missing a compiler.
- No scope creep: v1.0 general-purpose core first; IDE/Film(NVP)/Research/Writing DLCs later, with the IDE enabling Lu Dev self-development. Lu Dev can build/test in a disposable worktree with resource guardian and separately verified promotion/rollback; no surprise overwriting stable installation.
- Cognitum is an optional add-on and must not block Lu Core or alpha/v1.0.

## Historical conflicts resolved
1. Oct 25 non-admin/restricted-token/mandatory sandbox proposals and historical `DESIGN_GAP_AUDIT_2026-10-25.md` are **superseded specifically on mandatory isolation** by the owner's Nov 3 clarification. Preserve trusted conversation authority, error recovery, resource protection and refusal of self-granted authority.
2. Earlier Jan-fork desktop pathways are superseded by the independent Tauri/React app; Jan import and GUI reference remain relevant.
3. Earlier alpha gate checklists are historical; owner accepted functional `0.1.0-alpha.2` after physical Windows testing. The former A5 native-tool alpha gate moved to v1.0, not a reason to revoke acceptance.
4. R22 says JSON repair is deferred. Strict JSON validation still applies at the tool boundary; no blanket permission to parse permissive model output.
5. Any historical claim that generic filesystem-access permission automatically conveys authority is rejected. Any old hard workspace-root restriction should be reconsidered as a tool's explicit argument scope rather than Lu's universal OS environment.

## Current v1.0 engineering state (2026-11-03)
- `REPO-01`: GitHub tree frozen and source-identity CI can pass, but unpublished alpha.3-dev local source is not yet synchronized. **Open**.
- `WIN-LOAD-01/WIN-JOB-01`: actual engine PID/health lifecycle observations added; Job assignment API repaired; Windows compilation and installed-app behavior still require green evidence. **Partial**.
- `AUTH-01`: production chat journals carry host-generated user turn IDs and fail-closed verifier; separate portable harness exercises actual `ChatStore` → `ConversationAuthority` + host `PolicyEngine` → `NativeToolRegistry` read. This is NOT yet wired to user-facing tool approvals/live model tool dispatch. **Partial**.
- `ANSF/DATA-01`: Core stores/importers and portable source exist, but full current desktop integration and memory UI unaccepted. **Partial**.
- `TOOLS-01`: read-only native tools and registry are available; production GUI authorization wiring, arbitrary host-approved resource access, write/git/archive/JSON and execution are v1.0 work. **Partial**.
- `JOURNAL-01/ORCH-01/PROVISION-01`: building blocks and plans; end-to-end durable effect receipts, model orchestration and dependency provisioning not accepted. **Open**.
- `RELEASE-01`: signed, properly packaged and independently tested v1.0 Windows installer is not accepted. **Deferred**.

## Execution order and proof
1. Finish exact published-source CI (source lock, frontend, portable native-chat/auth/dx, Rust native core and Windows host compile). If a job fails, fetch logs and fix the actual cause; don't relabel pending/cancelled as green.
2. Integrate trusted persisted human-origin verifier and host-issued effect grants with a **real GUI read-only native tool**. Authenticate the task, choose/verify target and effect in trusted host state, dispatch through Lu Core, show evidence/denial, and test revocation. Model-generated JSON never creates a task or approval.
3. Native effect journal, atomic file mutation, move/trash/restore, search, Git/archives, process execution and toolchain provisioning—all with concrete authority and failure injection.
4. ANSF host-only adapter, search/context projection, source/revision links, belief working layer, desktop memory pane and crash-safe migration.
5. Resource guardian, durable task manager/agent continuity, streaming and multimedia robustness, then near-v1.0 physical Windows upgrade/install/uninstall/AMD Vulkan acceptance.

## Handoff protocol for each new Lu conversation
- Read this file first, then `docs/LU_V1_0_TASKS.md`, `docs/LU_V1_0_DESIGN_RECONCILIATION_20261103.md`, `src-tauri/lu-core/SECURITY_AUTHORITY_AND_OS_CEILING.md`, and the latest GitHub CI run on the active dev branch.
- Check the exact source identity and repository branch/commit, live test status, unpublished local source caveat, user-decided priorities and current task gate.
- Continue code and tests in that same branch unless a verified new branch is necessary. Keep alpha.2 immutable.
- Record every change with task ID, commit SHA, exact test results and what is **not** proven.
- Amend this handoff only for significant owner decisions or newly verified scope, not speculative assistant proposals.

## CONTEXT-01 implementation update — 2026-11-03
- Extended existing `src-tauri/lu-core/src/continuity.rs` rather than creating another authoritative memory store. `latest_for_conversation(project_id,task_id,conversation_id)` scopes resume records to all three trusted host identities.
- Added `tests/continuity_conversation_scope.rs` with restart, same-task/different-chat, no cross-chat data read, immutable checkpoint-ID conflict, and malformed requested identity regressions.
- **Not yet complete:** No production desktop retrieval/use of this lookup, no cross-chat user-facing provenance browser, no automatic source-grounded context assembly, and no conversion of any checkpoint into task authority. Tests are pending CI and do not imply authenticated owner login.

## WRITE-01E continuation (2026-11-03, after Convo 21)
- Work resumes on `lu/write-01e-approved-edit-gate-20261103` from `lu/win-load-01-host-trace-20261103`, not older `main`. Existing WRITE-01D `edit_safety` and `write_format` were retained. The separate `reversible_edit.rs` prototype added on `main` is **not** the active source of truth and must not be silently merged as a duplicate implementation.
- Core now has a source-committed native, effect-journal-dispatched existing-text edit and separately authorized rollback. It stores a durable intent and original-byte backup, stages replacement, validates exact reviewed bytes and expected source digest, publishes via pinned cap-std directory rename, then verifies result. Recovery classification is read-only and never revives authority.
- **Verification still required:** run all native-core tests and Windows host tests on the exact locked branch revision. Desktop full-diff approval/target selection, Windows NTFS durability/metadata preservation and concurrent noncooperative edits remain unaccepted. No physical Windows retesting until the deferred near-v1.0 phase. REPO-01 unpublished development snapshot remains a convergence caveat.
- Current specifics and limitations: `docs/WRITE_01E_DURABLE_EDIT_TRANSACTION_20261103.md`. Never describe source commits as production-enabled editing.

## WRITE-01E.3 desktop checkpoint
The branch `lu/write-01e-approved-edit-gate-20261103` now has native-picker review of a model-proposed existing-file edit, exact full before/after and line-change UI, SQLite host-only edit approval history, a second native confirmation, per-turn durable authorized commit, and read-only recovery that requires native re-selection to inspect uncertain disk artifacts. No WebView path or model-issued grant. The source changes require green full CI plus deferred Windows/NTFS and OS-modal acceptance; `0.1.0-alpha.2` remains frozen and the unpublished local-source convergence check remains open. See `docs/WRITE_01E_DURABLE_EDIT_TRANSACTION_20261103.md`.


## WRITE-01E.4B rollback handoff checkpoint
Development remains on `lu/write-01e-approved-edit-gate-20261103` / draft PR #5, based on the recovered complete WRITE-01D development branch rather than stale `main`. The latest source adds a separately approved, native-picked, journaled restore of previously approved exact text edits. Review is its own SQLite ledger (with restart interruption), complete diff plus native confirmation, fresh trusted user turn and typed Protobuf rollback effect. Restore preserves the displaced edited bytes and original backup, returns a verified new receipt, and shows read-only history after restart. Unknown effect outcomes are never replayed. CI and installed-app acceptance gates must be reported separately; source commits are not proof of correctness. Alpha.2 baseline remains frozen. See `docs/WRITE_01E_DURABLE_EDIT_TRANSACTION_20261103.md`.

## WRITE-01E.4C.1 handoff
Windows-specific existing-file transaction safety guard added: opened-handle 128-bit FileId/volume checks, hardlink/ADS/reparse/special-attribute refusal, identity comparison from original edit/rollback through the final prepublish barrier, and guarded recovery manifest reads. GitHub Windows CI has dedicated native tests, including identity substitution with identical bytes. It is a conservative fail-closed implementation; it does not preserve arbitrary ACLs or make the last check-and-rename atomic to non-cooperating writers. Check the exact new source lock and CI before promotion. The accepted alpha is still frozen; PR #5 remains in draft.

## CI accepted checkpoint — WRITE-01E.4C.1
The conservative Windows opened-handle guard passed exact implementation commit `8eb7fa51`: Lu Desktop CI `41773702637`, REPO-01 `41773702703`, WIN-LOAD `41773702696`, all successful. This is a development CI milestone only; Windows installed-app, metadata preservation, noncooperating-race CAS and power-loss safety remain open. FFI is isolated to `lu-win-worker-job`; Lu Core forbids unsafe. Continue from this branch/draft PR #5 rather than regenerating another independent file-edit implementation.
