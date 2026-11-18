# CP-13 — Governing Lu Core design trace and parity review kickoff

**Frozen entrypoint:** [4c16ba47e144921de0f56e54712246ee90714153](https://github.com/vjmichael/Lu-AI-GUI/commit/4c16ba47e144921de0f56e54712246ee90714153), existing CP-00E complete within scope. Owner direction: reconcile Automation and DX in the master ledger, then compare actual Core implementation against the latest governing requirements. No runtime code changes. This document is an initial authority/index and plan, not complete requirement reconciliation.

## 1. Governing design precedence

The current [Lu Central Product and Technical Design](LU_CENTRAL_DESIGN.md) identifies itself as the single entrypoint (§0, lines 39–44). Its canonical edition is Nov 3, with explicit later overlays, including Nov 8; it is not a single frozen latest design excluding subsequent decisions. Exact blob `0536adf6fb9bd696f600ed31051ad6082e603edd`.

- Latest explicit owner requirement controls actual conflict.
- For tool architecture/implementation, the original preserved Jan design remains primary unless superseded by explicit owner choice; independent Lu Core and approved authority/OS custody choices prevail. The merged tool specification is the interpretation/mapping, not permission to override original owner design.
- Later compatible owner-confirmed refinements supplement the central specification. The task ledger controls current sequencing, not design authority. Implementation notes, source existence and test passes are evidence, not design mandates.
- Preserve originals and date/quote owner decisions. If conflicting requirements cannot be resolved, record a specific owner decision needed; do not implement both, erase history, or prefer latest filename automatically.

## 2. Initial source index (all repository references at frozen SHA, not exhaustive)

| Date / authority | Document | Role |
|---|---|---|
| Oct 21–22 original | [Jan-Mod-Design-Document.md](reference/Jan-Mod-Design-Document.md) | Primary tools, MBRL, Protobuf, durable effects, memory and intended workflow; Jan-specific implementation and obsolete security assumptions not binding. |
| Oct 25 historical | [DESIGN_GAP_AUDIT_2026-10-25](../src-tauri/lu-core/DESIGN_GAP_AUDIT_2026-10-25.md) | Earlier Jan/Core comparison with explicit supersession warning, not current mandates. |
| Nov 3 primary | [LU_CENTRAL_DESIGN](LU_CENTRAL_DESIGN.md), [canonical handoff](LU_CANONICAL_HANDOFF_CONVOS_01_21_20261103.md), [v1.0 gates](LU_V1_0_DESIGN_RECONCILIATION_20261103.md) | Independent native app, v1 boundary, authority, memory, tasks, UX, packaged acceptance. |
| Nov 3 binding security | [SECURITY_AUTHORITY_AND_OS_CEILING](../src-tauri/lu-core/SECURITY_AUTHORITY_AND_OS_CEILING.md) | Access != permission != authority; distinguish ordinary native host from operation-specific protected worker custody. Reconcile later SEC/AGENT decisions. |
| Nov 4 tool specification | [LU_NATIVE_TOOLS_MERGED_DESIGN](LU_NATIVE_TOOLS_MERGED_DESIGN.md) | Original-first tool behavior, schemas, policy, recovery and tool-specific owner refinements. Nov 7 overlay. |
| Nov 6 scheduler/Observe | [LU_BACKGROUND_SCHEDULER_P02](LU_BACKGROUND_SCHEDULER_P02.md), [LU_OBSERVE_LEARN_V1](LU_OBSERVE_LEARN_V1.md) | Background worker scheduling/resource supervision, opt-in local observation, capture consent and source evidence. |
| Nov 7 owner philosophy | [KIAF philosophy](LU_AI_EXECUTION_LAYER_PHILOSOPHY_20261107.md), [tool review](LU_AI_EXECUTION_TOOL_REVIEW_20261107.md), [human-centered memory](LU_HUMAN_CENTERED_AI_MEMORY_FIRST_20261107.md), [decision register](LU_DECISION_MEMORY_REGISTER_20261107.md) | Practical capability = f(K,I,A,F), Foundational/Utility/Specialist; reuse real application capabilities and memory before unnecessary micro-app invention; verify actual goals/observations. |
| Nov 7–8 memory and agents | [MEMORY_AND_MEDIA_ROADMAP](../src-tauri/lu-core/MEMORY_AND_MEDIA_ROADMAP.md), [active execution ledger](LU_KIAF_ACTIVE_EXECUTION_LEDGER_20261108.md), AGENT-01A–L dated files | Immutable ANSF, memory provenance, bounded host scheduling, real model plan draft, fresh human-step permission, independent oracle, causality/recovery. |
| Nov 9–12 follow-ons | PLAN-01–07D, MEMORY-LOOP and PERF01/P0 records, SEC original source acceptance, [Core convergence plan](LU_CORE_CONVERGENCE_PLAN_20261109.md) | Later specific decisions and separate implementation receipts; review which were actually authorized vs source proposals. |
| Nov 16 latest | [canonical development ledger](LU_DEVELOPMENT_LEDGER.md), CP-00A–E reports and confirmed owner direction | Current tabbed UI, no technical jargon, actual tool reachability, unified execution, physical failures, grouped repair order. |

## 3. Automation and DX: original records to retrieve

Search original repository branches/PR metadata and user documents/previous decisions for original AX/Automation, DX-00–15, DX-IPC, Windows security handoff and actual physical/installed evidence. The current tree contains partial diagnostics/host scheduling source but no complete authoritative AX/DX lineage established in this kickoff. Prior recovered context distinguishes DX fault localization from generic logging, and AX/Cowork as separate; no automatic approval of general autonomous repair. Recover exact original records/SHAs and time order before classifying optional AF_UNIX bridge, automated tests/benchmarking or user-run diagnostics as binding or deferred. In particular, keep distinct read-only diagnostic IPC and deferred DLC IPC. No old branch merge merely because a component is absent. Use the existing CP-00E acceptance dimensions.

## 4. CP-13 deliverables

1. **Requirement register:** stable ID, domain, exact source document/section and immutable ref, owner decision/date and evidence, scope (Core current / deferred / superseded / optional / unresolved), precedence links and measurable acceptance.
2. **Implementation trace:** actual ordinary user entrypoint or explicitly host-only action → typed contract → registry/authorization → real handler/worker/resource → actual result/oracle/receipt → next model/user response. Map to CP-00A–E rather than repeat general source scanning. Separate source, reachable, executed, verified, installed/physical. Missing code, disconnected code, incorrect replacement, functional defect and untested path are different.
3. **Conflict/gap matrix:** code doesn't change the design; identify dropped owner requirements, stale proposals, unjustified duplication, deferred DLC, redundant historical implementation and requirements not satisfied by current tests. Preserve all actual original negative security assertions and historical evidence.
4. **Current design index and ledger mapping:** no competing new task ledger, no erasing the original documents. Map each confirmed gap to CP-01–12 or explicit follow-on only when justified. Update existing entries rather than cloning work. Record no-go design conflict and owner decisions required.
5. **Acceptance:** each binding requirement has a disposition and source/evidence anchor; complete confirmed coverage for the current Core scope, identify inaccessible originals, unresolved requirements and unverified behavior transparently. Implementation proceeds in grouped candidates, CP-12 instrumentation early where useful, CP-11/12 complete applicable requirements before CP-09 and CP-10. No installer or heavy acceptance before the grouped repairs. Keep separate physical Windows and qualified Linux proofs.

CP-13 is the immediate next task; previous CP-00 audit remains complete. This review is requirement-driven, not another open-ended general audit.
