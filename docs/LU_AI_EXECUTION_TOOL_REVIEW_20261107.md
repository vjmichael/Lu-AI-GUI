# Lu AI Execution Layer — P01–P12 tool audit under f(K, I, A, F)

**Date:** 2026-11-07  
**Method:** documentation and read-only source review on P11 stacked development head `7f2476d82a05be745ae4a514449072c3d5f15971`.  
**Authority:** [Canonical AI Execution Layer philosophy and three meta-classifications](LU_AI_EXECUTION_LAYER_PHILOSOPHY_20261107.md).  
**Scope:** NO runtime code, builds, new dependencies, new operational grants, installation or P12 implementation. This is a proposed direction and classification audit, not a verified release report.

## 1. Executive finding

Lu's existing code already contains many **foundational** pieces: scoped filesystem I/O/search, a bounded Reader, browser tooling, real Git read-only calls, capability inventory, authorization-aware native tool routing/Protobuf, guarded selected native worker lifecycles and durable task journals. Many **utility** handlers have also been developed (archives, images, Office, limited citations). Their continued existence is reasonable.

The largest observed **interface (I) gap** is **not evidence that a knowledgeable model needs more domain micro-apps**. It is that the reviewed host supervisor explicitly says it is *not* a model-facing arbitrary program runner. No audited source proves a general, trustworthy application API/CLI/code/GUI execution surface available to the local model for arbitrary installed productivity tools. Browser scoped operations and approved Git inspection are specialized examples of execution, not an installed universal app operator. Treat the latter as a **capability design/evidence gap**, not a declaration that the whole repository lacks any code execution path.

The main **feedback (F) gap** is task-specific independent verification and executable end-to-end recovery evidence across installed apps. A zero-byte XLSX or a tool's success message should be rejected by result validators; a nonempty OOXML zip should still be reopened and checked against expected contents. Existing receipts/snapshots are valuable but do not prove output correctness in external applications.

**Security is not a blocker to wave away:** a general execution interface has a larger attack/effect surface than typed read-only utilities. Foundational must not mean unrestricted.

## 2. Four-factor maturity review

| Factor | Reviewed evidence | Assessment | Recommended change (design only) |
| --- | --- | --- | --- |
| **K — model knowledge/reasoning** | Qwen local experiment reported DOCX creation, failed/zero-byte XLSX detection and fallback to Python standard-library OOXML; P03 routes models/tools | Demonstrates a useful example of model-driven planning/recovery, **not** a representative competence benchmark | Establish repeatable local-model tests with strict distinction between plan and executed result; never embed generic Office/Git know-how in Core merely because no benchmark exists |
| **I — execution interfaces** | `native_files.rs`, `native_file_discovery.rs`, `native_reader_tool.rs`, `native_browser_tool.rs`, `native_git_inspect.rs`; `native_background_process.rs` explicitly disclaims model-facing generic executable invocation | Strong **scoped individual paths**; general safe installed-app/code/GUI/API execution not established | Inventory real host capabilities, app versions and tool prerequisites; design (not build yet) a reusable high-assurance execution interface for approved apps/libraries with clear effects and observable outputs |
| **A — authorized access** | `conversation_authority`, `native_capability_inventory.rs`, `native_workspace_read_turn.rs`, scoped BrowserTool and Git fixed executable; native dialogs/task receipts | Existing strong *source-level* design: registration/discovery ≠ authority, trusted human turn and host-selected roots; installed acceptance varies | Preserve exact origin, effect, resource, credential and egress grants; explicitly separate OS permission from owner/task authority and code/network privilege from file read |
| **F — feedback/verification/recovery** | Typed result envelopes/JSON–Protobuf boundary, hashes, `TaskStore`, native background result/status, PREPARED/APPLIED/UNCERTAIN image/archive/Office receipts and source-test suites | Good infrastructure, uneven **external result oracles**; installed app success and many crash/restart gates remain open | Add independent verification criteria per task type and observable error attribution in evaluation; never auto-replay uncertain writes/sends |

**Interpretation:** this is a source-architecture review. Source presence or CI PASS is not proof of complete installed-app or local-model acceptance.

## 3. P01–P12 classification and course correction

The class column identifies role **per capability**. A Pxx containing two or three classes does not imply it should be deleted or split immediately.

| Work order | Relevant roles under new policy | What Lu already has in source/docs | Recommendation / change |
| --- | --- | --- | --- |
| **P01 Filesystem** | **Foundational** discover/read/write and identity; **utility** narrow transforms | Capability-scoped root, bounded metadata/content discovery, safe file operations, freshness heuristics | **Retain and prioritize**. Focus on user-friendly scoped access and independent source identity, not yet another file-manager application; don't silently crawl all drives |
| **P02 Task/Guardian** | **Foundational** admission, lifecycle, budgets, cancellation/recovery | Durable scheduler, trusted worker supervisor, resource policy and receipts; physical GPU/installed evidence not universal | **Retain.** Reuse for any future app execution; test process-tree cleanup, restart and actual OS resource caps. No model-granted admission |
| **P03 AI operations** | **Foundational** model routing, capability discovery, tool context; **utility** model catalog UI | Model/provider search and pinned metadata, scoped tool inventory, context identity checks, routing | **Retain, simplify reasoning duplication.** Show only real admitted tools and their prerequisites; benchmark target model's ability to select the cheapest suitable interface |
| **P04 Reader/OCR** | **Foundational** observational access; specialist extraction algorithms remain internal adapters | Text/PDF/EPUB/Office/RTF readers, selected OCR and multimodal paths; initial bounded reader caps; broad P04 work previously deferred | **Retain as one public Reader.** Use existing parsers/OCR libraries. Improve large-file selectors/provenance **only when approved**; don't develop an e-book/Office viewer |
| **P05 Browser/transfers** | **Foundational** network observation/controlled interaction and file transfer; **utility** per-site shortcuts | Browser-origin scoped native operations and research fetch; transfer components; complete shared transfer work was deferred | **Retain; audit coverage.** A browser reader does not imply authenticated API writes or generic desktop automation. Separate network read/write, credentials, consent, downloads and publisher effects |
| **P06 Observe & Learn** | **Foundational optional observational input**, with *utility* selective diagnostics | Consent-gated Windows observation/WGC source flow, privacy/egress policies and task handoff in source; physical acceptance open | **Retain OFF by default.** Do not interpret screen capture as right to click/control apps. Explicit second consent, no silent surveillance; independently test visual handoff |
| **P07 Research/citations** | **Foundational** source/provenance observation; **utility** limited bibliography formatter/export; **specialist** full CSL/SBL/reference management | Crossref/OpenAlex source evidence, pinned catalog, limited styles, bibliography export, revision audits; no complete CSL engine | **Retain bounded evidence and formatter.** For expert use, operate mature CSL/Zotero/Office software under future approved interface; don't make Lu a bibliographic publisher |
| **P08 Archives** | **Utility** format inspect/create/extract/split; **foundational** integrity, scoped file effects; **specialist** niche repair | Existing format handlers and no-overwrite SHA-verified byte splitting/reassembly | **Retain safe wrappers.** Prefer established archive libraries/programs for new formats; do not expand toward a repair suite |
| **P09 Office** | **Foundational** structured document observation/I/O; **utility** simple create/read/edit; **specialist** expert Office authoring | Bounded DOCX/PPTX/XLSX reads, limited new files and source-pinned edits; layout and formula fidelity excluded | **Retain P09 as adequate Core scope.** Advanced tasks via external Office engines and independent render/formula inspection; no in-Core Office clone |
| **P10 Image** | **Foundational** image input/region observation; **utility** deterministic crop/resize/redact/annotation/filter; **specialist** professional editing | Pinned PNG/JPEG source admission, bounded operations, staged preview and new PNG export, history | **Retain.** Distinguish opaque redaction from pixelation; prefer external graphics apps for layers, generative and professional work |
| **P11 Git** | **Utility** basic repo observation; **foundational** trusted process/authority pattern; **specialist** GitHub/IDE dev workflows | Real host-approved Git status/log/diff; fixed argv; no arbitrary shell, push, commit or account credentials | **Retain read-only Core**; Lu IDE/general supervised execution can later combine richer approved workflows. Do not invent model-facing Git micro-application |
| **P12 JSON/Protobuf** | **Foundational trust boundary**, **not** a model-facing JSON productivity editor | Strict model JSON schema, duplicate-key rejection, bounded repair, typed Protobuf tool/result correlation | **Retain unchanged.** P12 is **not started** by this audit; later correctness/diagnostics only, not an automatic expansion to external JSON repair tools |

### Other original merged domains (beyond Pxx labels)

- **`memory`, `work`, `scratchpad`** → **foundational** context and state, but staging/scratchpad is not a substitute for the effect journal or authority. They can improve K's effective usable context without modifying model weights.
- **`exec`** → **foundational aspirational interface**. The original design *specifies* code/host execution under explicit authority; reviewed `native_background_process.rs` is currently host-plan-only and expressly not a model-facing generic runner. No implementation inferred.
- **`web`** → **foundational** observation/approved interaction; specialized research/citation services are **utility** on top. NetworkWrite is never implied by browsing rights.
- **`validate`, `json`, `loop_guard`** → **foundational** independent trust/reliability controls. Do not invite model text to decide whether malformed structure is safe.
- **`media`, `transcribe`, `video`** → media decoding/perception is **foundational observational I**; small deterministic transforms are **utility**; professional production is **specialist**, usually NVP/other programs.
- **`env`, `symlink`** → part of guarded foundational OS/file interfaces, not independent product micro-apps.
- **Lu IDE/NVP/Cognitum** → specialist workspaces/components may use the common execution contract but do not become mandatory Lu Core features.

## 4. Cross-cutting changes actually justified by this review

### Decision A — Establish a capability inventory over tool-count maximization
One record per **real runnable capability**, containing: host-owned identity, installed/version status, model-facing schema, read/write/execute/network/device effect, required grant, data-in/data-out format, resource/cancellation expectations, independent verification method and observed installed acceptance. Avoid confusing `source module present`, `schema registered`, `runner installed`, `model can invoke` and `end-to-end task passed`.

### Decision B — Evaluate generalized code/application execution **before** new P07–P11 micro-app development
The Qwen experiment shows the *potential* of a model using a general Python runtime to produce office artifacts; it does not prove that unrestricted execution is safe. Conduct a **design/security review** of a host-owned, opt-in execution facility with disposable workspace/allowlisted resources, bounded process lifetime, dependency/version discovery, captured stdout/stderr/structured outputs, effect classification and non-replayable remote side effects. Then run isolated proofs if separately authorized. Do not conflate with currently shipped `BackgroundProcessSupervisor` or use raw shell access as a shortcut.

### Decision C — Make feedback an independent product requirement
Every effect class needs a testable oracle. Examples:
- file write: actual size, expected SHA, parse/open, source untouched, no-clobber;
- spreadsheet: workbook opens and expected cells/formulas/values are present, formula evaluation by a trusted engine where required;
- DOCX/PPTX: package opens, text/structure and rendered layout checked as required;
- API/GitHub: subsequent authorized GET or authoritative response verifies durable remote state;
- GUI: current app identity, observed state before/after, reliable cancellation, no clicking unreviewed targets;
- security: wrong-grant, revoked grant, symlink/path race, network egress, helper execution and uncertain-effects tests.

The model should interpret these observations, not self-certify them.

### Decision D — Prefer composite workflows over cloned domain intelligence
Allow the model to compose authorized foundational primitives, small utilities and external specialist apps. Model tool descriptions should reveal exact capabilities, limits and failure semantics; exposing an absent `execute_code` or fabricated XLSX support is a correctness defect. Utilities may remain as safer shortcuts/fallbacks even when general code execution exists.

### Decision E — Preserve user control, portability and single-installer preference
External integrations must be optional, discovered and license-aware. No silent dependency installation, privilege escalation, network access, credential capture or Docker requirement. The installed Lu app should communicate clearly when a specialist program is absent. Packaging decisions and supported app compatibility need a separate owner-approved specification.

### Decision F — Reconcile scope documentation without rewriting past accomplishments
The existing original native-tool design and Pxx ledgers are historical and technical evidence; some older lines say a phase is *unstarted* even after later owner decisions. Add links to the new canonical philosophy rather than deleting prior records. Preserve exact-head CI and installed application gates as independent states.

## 5. Proposed evaluation order — no coding authorization

1. **K baseline:** selected local GGUF models describe workflows in Git, Office, XLSX, source citations, archives and image tasks without asserting execution.
2. **I inventory:** identify what Lu actually exposes and test an isolated, existing utility on disposable files; inventory missing API/CLI/GUI interfaces and dependencies.
3. **A adversarial review:** wrong task, denied root, denied network, revoked grant, untrusted document prompt injection, OS privileges, credentials, arbitrary process.
4. **F end-to-end review:** artifacts opened and revalidated, errors surfaced, crash/timeout/cancel uncertainty preserved, no invented verification.
5. **Comparative decision:** choose foundational extension **only** if independently justified; keep a utility only if it materially improves safety, quality or efficiency; integrate a specialist app when replacing it would be wasteful.

**Priority conclusion:** audit the **model-facing supervised execution/observation/recovery gap**, not rewrite P07/P09/P10/P11. P04/P05 remain explicitly deferred as development phases; existing Reader/Browser source still counts as reviewed infrastructure. P12 remains unstarted. Do not begin implementations based solely on this audit.

## 6. Review anchors (source checked)

- Prior policy and owner scope: `docs/LU_AI_TOOL_DESIGN_PRINCIPLES_20261107.md`, `docs/LU_NATIVE_TOOLS_MERGED_DESIGN.md`, `docs/LU_CORE_TOOLS_SCOPE_AND_PRIORITY_20261106.md`.
- Workspace and capabilities: `src-tauri/lu-core/src/native_workspace_read_turn.rs`, `native_capability_inventory.rs`, `native_file_discovery.rs`, `native_reader_tool.rs`, `native_browser_tool.rs`.
- Execution and lifecycle: `src-tauri/lu-core/src/native_background_process.rs`, `native_git_inspect.rs`; corresponding P02/P11 ledgers.
- Domain behavior and release warnings: `docs/LU_AI_OPERATIONS_P03.md`, `LU_P06_OBSERVE_PRIVACY.md`, `LU_P07_RESEARCH_STYLES.md`, `LU_P08_ARCHIVE_SPLIT.md`, `LU_P09_OFFICE_BOUNDARY.md`, `LU_P10_IMAGE_BASICS.md`, `LU_P11_GIT_CORE.md`, `TOOLS_02_INTERNAL_JSON_BOUNDARY_20261103.md`.
- Observational user experiment: Qwen locally called a DOCX file creation tool, detected a reported zero-byte XLSX and reportedly repaired it by generating OOXML ZIP parts in Python. This is a useful **case study**; it has not been independently graded as a complete Office productivity benchmark by this audit.
