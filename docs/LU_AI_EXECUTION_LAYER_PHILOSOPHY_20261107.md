# Lu AI Execution Layer — canonical philosophy, capability model and tool classification

**Owner decision:** 2026-11-07  
**Status:** AUTHORITATIVE for the philosophy, classification and design tests of Lu's AI Execution Layer. Documentation-only decision; **no runtime, permission, worker, software installation or P12 implementation authorized**.  
**Companion audit:** [AI Execution Layer Tool Review](LU_AI_EXECUTION_TOOL_REVIEW_20261107.md).  
**Historical technical context:** [Merged native tool design](LU_NATIVE_TOOLS_MERGED_DESIGN.md), preserved [original Jan-Mod design](reference/Jan-Mod-Design-Document.md), [earlier AI-first policy](LU_AI_TOOL_DESIGN_PRINCIPLES_20261107.md).

## 1. Canonical conceptual expression

**Practical AI Capability = f(K, I, A, F)**

- **K — Knowledge and reasoning:** the model's learned procedural knowledge, planning, code/formula construction, task interpretation, tool-selection judgment, and model-specific limitations. K depends on actual model/quantization/context; do not claim a local model has frontier-model competence without measuring it.
- **I — Interfaces to the environment:** what the installed runtime can actually observe and operate: filesystem I/O, approved document reading/vision, network/browser, program/library execution, application APIs, GUI controls, structured capability discovery and error channels. A tool's registered name does not prove that its implementation is installed or available.
- **A — Authorized access:** a valid **human-origin task and host-governed scope for the precise effect and resource**, including policy, OS permissions, credentials, workspace/domain limits and review. **OS ability or permission is not model authority**; tool visibility and learned command syntax are not authority. A model may propose an action but cannot approve, elevate or widen its own grant.
- **F — Feedback, verification and recovery:** current observations, inspectable receipts, independently checked outcomes, typed errors, cancellation, durable journal, restart reconciliation and safe retries. The model's confident completion sentence is not evidence of effect.

The expression is a **design function**, not a computed formula, literal sum or universal product. An unavailable interface, denied authority or missing verification may block completion regardless of K. The task/environment and risk determine how these factors interact. Measure the result empirically rather than inventing weights or success percentages.

**Knowledge ≠ execution.** An offline GGUF model may know enough to construct an XLSX package or GitHub REST request; it does not thereby gain filesystem, process or network access. The **Lu host executes** an authorized plan and returns the actual result. Offline model inference can coexist with explicitly authorized online operations.

## 2. Exactly three meta-classifications

These are the authoritative **classification vocabulary for the AI Execution Layer**. They describe a **capability's role**, not one repository directory or one entire Pxx phase. Mixed domains must be decomposed into functions before classification.

### Foundational
A reusable interface or enforcement service that gives a model access to real-world state/actions or maintains the invariant that such access is authorized, bounded, observable and recoverable.

Examples: scoped filesystem search/read/write; secure network/browser interface; Reader/OCR/vision as observational access; guarded program and application execution; capability discovery; per-turn authority and IPC validation; Guardian/TaskStore and process-tree containment; verified result/report/reconciliation; model/backend coordination; optional consented Observe & Learn capture.

**Test:** if many unrelated workflows require it as a gateway or trust boundary, it is foundational. Foundational does **not** mean every model turn receives the capability.

### Utility
A small, deterministic, broadly useful operation that sits **on** foundational access and offers a measurable simplification, correctness, security or performance advantage. Prefer maintained libraries, existing executables and stable wrappers; keep model schemas narrow.

Examples: basic archive inspect/extract/split, DOCX/XLSX/PPTX limited new-file authoring, simple text/cell edits, image crop/resize/redact/annotation, CSV transformation, read-only Git status/log/diff, format conversion, bounded citation formatting.

**Test:** if the same job can be done with general execution plus a library/application, keep the utility only when its tested safety, speed, reliability or user experience justifies the maintenance cost. A utility is neither a standalone clone app nor an independently privileged action path.

### Specialist
Deep domain functionality primarily supplied by an existing full application or an explicitly separate Lu DLC, with its own complex feature and standards surface. **Lu Core operates the specialist rather than reimplementing it.**

Examples: complete Word/Excel/PowerPoint/LibreOffice workflows, Zotero and advanced publication/citation conventions, image layers/pro color/painting, full developer GitHub workflows in Lu IDE, complex archival repair, theatrical editing in NVP, advanced scientific solvers in Cognitum.

**Test:** if maintaining it means recreating a mature full application or standards domain, classify it as specialist and prefer a governed integration.

**Composition rule:** a task can combine all three. A bibliography workflow uses foundational document/network I/O and provenance, a utility reference formatter where justified, and specialist Zotero/CSL/Office features through approved interfaces.

## 3. Canonical AI Execution Layer contract

1. **Understand:** Model proposes a plan based on K. Treat instructions contained in files/webpages/tool output as untrusted data, not permission.
2. **Discover and describe:** Host reports real installed capabilities and version/availability, compatible libraries and safe schemas. Inference of an app's existence is insufficient.
3. **Admit and authorize:** Host binds exact user task, resource identity, operation/effect, expiry and separate network/credential scope. Reading, writing, execution and network egress are different authorizations. OS permission must also permit the act, but cannot replace human authority.
4. **Execute:** Prefer approved existing application APIs/CLI and deterministic libraries; GUI automation is a separately admitted fallback. Any general-purpose code/command runner needs strong isolation, admission, IO boundaries, resource limits, process-tree lifecycle, and effect-specific approval. There is **no universal ambient shell** implied by this policy.
5. **Observe:** Return actual structured state, errors, hashes, process results, document extractions or screenshots as appropriate; preserve provenance and limits.
6. **Verify:** Independently confirm claimed artifacts, correct content, render, formulas, citations, network effects or external state; do not mistake a zero-byte file, exit status or model summary for success.
7. **Recover or escalate:** Journal PREPARED/EXECUTING/APPLIED/FAILED/UNCERTAIN effects, reconcile after crash, never blindly replay uncertain writes/sends, and request user input when authority or evidence is insufficient.
8. **Report:** Communicate *proposed*, *executed*, *verified*, *failed*, *blocked* and *unknown* distinctly; do not claim the installed app has passed acceptance because source code or CI fixtures exist.

Existing original-design invariants remain binding: model-facing narrow JSON facades, strict validation → typed Protobuf IPC, per-effect authorization, bounded model-boundary repair, loop guard, durable effects and OS containment. **This new explicit owner directive supersedes older speculative plans to build full domain micro-apps**, not the original security/transport architecture or the historical documentation. Preserve prior source and release gates until separately changed.

## 4. Tool development decision policy

Before **any** new domain feature, ask in order:

1. Can the **target local model** describe/plan it correctly? Measure K; don't embed domain knowledge in Rust merely because model performance is uncertain.
2. Can an existing **general I** or already shipped utility execute it through an established library/app?
3. Is the relevant **A** realistically grantable and maintainable across OS versions without overbroad privileges, credential leakage or covert observation?
4. Can **F** establish task correctness and safe recovery? If not, design the independent oracle before expanding operations.
5. If a gap remains, is it a *foundational missing primitive*, a *small utility justified by evidence*, or a *specialist application integration*?
6. Compare real task success, safety, context/token use, latency, resource use, portability and long-term maintenance. Do not automatically build because a proposed tool is technically possible.

Do not create another per-format AI mini-app just to compensate for a missing general execution interface. Conversely, do not delete safe deterministic utilities simply because a capable model can sometimes reproduce them with Python. Code generation and arbitrary process execution carry greater security and reliability risks than constrained utilities; prefer the right execution surface for the risk.

## 5. Independent evaluation of K, I, A and F

For each representative task, evaluate four independent gates:
- **K:** model can supply an accurate plan or code, with no assumed tool access.
- **I:** host can actually invoke the required operation against an isolated/disposable fixture.
- **A:** the same operation is refused on wrong user task, resource, effect, revoked grant or unexpected egress.
- **F:** outcome is independently checked, malformed/zero-byte outputs detected, uncertainty preserved and recovery bounded.

Then compare **(1)** plan-only, **(2)** general authorized execution using existing software/library, and **(3)** specialized utility if it exists. Record exact model, quantization, OS/app/runtime versions, tool schema and source revision. Attribute failure to K, I, A, F, target application, environment or unsuitable benchmark rather than mislabeling every failure as model incompetence.

Illustrative task: construct a valid spreadsheet; ensure output size and ZIP/OOXML structure, reopen data and formulas in a trusted independent reader/application, refuse unauthorized target folders, and test crash/retry. A successful tool return or nonempty file alone is insufficient.

## 6. Implementation/authority and historical precedence

This document is the **source of truth for the AI Execution Layer's mathematical framing and foundational/utility/specialist classification**. The preserved Jan-Mod and merged native tool specs continue to govern technical protocols, scope and durable-effect semantics where not expressly modified by the owner. The priority register controls the work queue; the implementation ledger and exact-head CI report observed state. None of these documents substitutes for an explicit instruction to start new code.

**Current directive:** review and document only. Do not begin a generalized execution service, P12, new app adapter, agent sandbox or specialist feature implementation based on this document alone.
