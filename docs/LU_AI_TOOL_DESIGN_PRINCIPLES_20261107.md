# Lu Core — AI-first tool design and external application execution policy

**Decision:** 2026-11-07. **Owner approved.** This is the current general rule for designing Lu's tools, including ongoing and future Pxx work. It supplements, and where necessary supersedes, earlier proposals to expand domain-specific Lu applications. Existing tools remain in place until evaluated; this decision is **not** authorization to delete a tool, weaken security or expand OS permissions.

## Fundamental distinction
- The **AI model** supplies general task knowledge, language, code/formula generation, procedural planning, interpretation and judgment. Actual performance varies by model, quantization, context and tool-use training and must be measured, not assumed.
- The **Lu execution layer** supplies access to the user's current world: scoped filesystem I/O, approved browser/Reader observations, approved process/application operation, machine-readable results, resource containment, durable progress, cancellation, recovery and verification.
- The **external application** (e.g. LibreOffice, Zotero, graphics program, package tool, Git, or library) performs specialist computation/editing using its supported interfaces. Lu should **operate** common productivity software efficiently rather than reimplement it.
- **Permissions versus authority:** a process having OS permission to touch a resource does not grant the model the authority to use it. Lu requires user-specific intent, explicit host authorization, scoped capability grants, and per-action checks as already designed. A model may propose actions but cannot self-authorize, raise privileges, or mint grants.

## Tool implementation decision hierarchy
1. **Model knowledge first:** Ask whether the model already knows the procedure; evaluate actual local GGUF/model performance rather than recreating knowledge as a micro-app.
2. **Universal I/O first:** Check whether existing filesystem, Reader/OCR, browser, structured source observation, app APIs, and supervised execution can accomplish the task safely.
3. **Reuse existing applications and libraries:** Prefer documented APIs/CLI/application automation (when supported), with GUI interaction as a guarded fallback. Detect installation, version, license and unavailable capabilities; never silently install or execute code.
4. **Native specialization only with evidence:** Add narrowly scoped host-native tools if they produce measurable security, reliability, throughput, format-admission, or deterministic verification benefit that cannot reasonably be achieved with existing interfaces.
5. **No feature cloning:** Do not make Lu Core an Office suite, bibliography manager, graphics editor, developer IDE, archive repair suite or equivalent replacement application solely because the model could use those features.
6. **Preserve verification:** A generated artifact, scripted operation or model-written claim is not success. Inspect resulting file bytes, structured outputs, application responses, rendered output or external authoritative records as appropriate; maintain uncertainty when unverified.
7. **No unrestricted command escape hatch:** General code/process execution is high-risk. It must remain capability-mediated, containment-supervised, resource-budgeted, auditable and subject to approval appropriate to effects. Discovery/OS permission does not create authority.
8. **Avoid redundant interfaces:** One general-purpose execution primitive should serve many tasks when it safely can. Specialized shortcuts are optional accelerators, not mandatory per-domain intelligence.

## Core responsibilities to retain
- Safe filesystem search/read/write, source identity and freshness, atomic new-file publication.
- Browser/network access with domain/path approvals, Reader/OCR and multimodal input where needed.
- Model/context management, host-owned policy and tool registry, durable job and effect journals.
- Guardian/resource and worker isolation, cancellation/process-tree cleanup, crash/restart reconciliation.
- Application discovery + compatible API/CLI/GUI operation with observable results; no assumption these are already fully implemented.
- Independent checks and transparent uncertainty for success and accuracy.

## Impact on ranked work
- **P07 Research:** Retain existing evidence/provenance, trustworthy source lookup and safe bibliography exports. Prefer a reviewed CSL/Zotero or other specialist application interface for expanded citation features; do not build a parallel scholarly publisher inside Lu Core. Citation formatting never establishes source truth.
- **P08 Archives:** Retain safe file/archive I/O, no-overwrite split and inspection features already developed. Prefer established archive libraries/tools for richer formats; niche repair does not justify a bespoke app.
- **P09 Office:** Existing bounded Office read/create/versioned edits are suitable safe fallback and validation primitives; use a supervised external Office engine for advanced document/spreadsheet/presentation work instead of building an Office replacement. No additional feature expansion requested.
- **P10 Image:** Retain existing immutable basic crop/resize/rotate/flip/solid redaction/annotation/filter, staged preview and new-file export. Advanced image editing, generative painting, layers and professional color work belong to external tools. Image decoding/region access remains helpful for Reader and multimodal inference.
- **P11 Git:** Reuse approved native Git capabilities; advanced IDE integration stays in Lu IDE.
- **P01/P02/P03/P04/P05/P06/P12:** Prioritize foundational I/O, observation, policy, reliable execution, model operations and validation rather than duplicating model reasoning.

## Required evaluation before adding a domain micro-app
Compare (a) model knowledge/planning, (b) approved general execution against an existing app/library, and (c) existing specialist native tool where available, on representative tasks. Score task result and objective correctness, tool calls, state errors, repair attempts, latency, memory/energy, failure attribution, permission boundary behavior and maintenance cost. Test local target models; do not infer competence solely from large cloud-model benchmarks. Only build missing abstractions after measurable deficiencies.

## Status and acceptance
This file is a design decision, **not** proof that Lu already has a safe universal desktop computer-use worker or access to every application. Continue existing OS/worker/approval/installed-application release gates; preserve already authored code and independent acceptance evidence. P10 starts as a bounded correctness/scope review, not a new graphics-product implementation wave.
