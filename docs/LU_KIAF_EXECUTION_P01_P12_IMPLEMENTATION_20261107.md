# Lu AI Execution Layer — P01–P12 recommendation implementation closeout

**Date:** 2026-11-07  
**Base:** canonical [f(K,I,A,F) design](LU_AI_EXECUTION_LAYER_PHILOSOPHY_20261107.md) and [P01–P12 tool review](LU_AI_EXECUTION_TOOL_REVIEW_20261107.md).  
**Branch:** \`lu/ai-execution-p01-p12-recommendations-20261107\` stacked on documentation PR #86.  
**Rule:** Implement only verifiable missing foundational/utility functionality; SKIP further domain expansion when existing functionality is appropriate. Keep the distinction between source implementation, native CI, actual local model execution and installed-app acceptance. Do not weaken existing human-origin authorization, host-owned resource admission or effect journaling.

## 1. Actual implementation in this batch

### F — model-facing, independent artifact structural inspection
- **\`native_artifact_inspect.rs\`:** introduced the read-only \`lu_artifact_inspect\` native/model tool inside the existing exact human-approved workspace turn. No model-selected absolute path, shell, source overwrite, execution or external network operation.
- Source bytes are read through cap-std's scoped handle; reject traversal/hidden paths, symlinks, zero-byte/missing files, unstable size/mtime, over-8 MiB files, bad SHA-256 and malformed/unsupported formats.
- Supports **DOCX/XLSX/PPTX** bounded ZIP and required-part inspection with entry names and inflated-size caps, generic ZIP manifest sanity, JSON syntax parsing, UTF-8 CSV/text, PDF header only and bounded PNG/JPEG full decode.
- Output receipt includes inspected byte length, SHA-256, validation *level*, ZIP member count and **\`semantic_verified=false\`**. Passing is NOT proof of expected worksheet cells, formulas, document pagination, security of later extraction, citations or application compatibility.
- Native tests: zero-byte XLSX fallback, missing workbook parts, malformed JSON, wrong SHA, invalid model parameters, unapproved dispatch, symlink escape, source immutability and three-tool workspace read integration. A reported \`create_file\` success cannot override an invalid or empty result.
- This is the smallest reusable independent output check for P07/P08/P09/P10 workflows; no new Office editor, spreadsheet generator or citation engine added.

### I / A — reviewed application interface contract, **not yet a general program runner**
- **\`native_external_app_action.rs\`:** typed host-reviewed operation registration and a narrow model-facing input schema. The host supplies the implementation, not the model; the call has **no executable, arguments, working directory, URL, credentials or arbitrary shell field**.
- The initial adapter deliberately accepts **ReadWorkspace only**. All WriteWorkspace/ExecuteCode/NetworkRead/NetworkWrite/Device/Elevated actions are rejected at registration until the host can prove journaled effects, process isolation, credential/egress boundaries, cancellation and recovery in an implementation-specific bridge.
- Native registry separately requires task-scoped host authorization *per invocation*. Integration tests verify a denied call never reaches the host backend, unknown JSON fields are rejected and a synthetic backend result is explicitly **unverified**.
- **No external application or executable is registered by default.** No model, browser, CE, package or WebView is granted a new privilege or executor as a result of this implementation. This is a reusable interface foundation for a later separately approved, OS-protected executor, not a claim of complete general app control.

### I / A / F — honest tool inventory
- **\`native_capability_inventory.rs\`:** currently installed, registered and per-turn-granted tools now expose a reviewed foundational/utility classification where mapped, with *unclassified* represented by absence, not an invented fourth meta-class.
- The inventory explicitly reports **registered_and_turn_granted_only**; external dependencies and task outcome verification are **false** until independently established. Tool schema registration must not be sold as installed app availability or successful effect.
- This does not affect dispatch permission logic.

### K — grounded benchmark failure attribution
- Existing `benchmark_contract.rs` keeps its v1 record schema and previous failure labels. Additional `ExecutionInterface`, `AuthorizationPolicy`, `ResultVerification` and `Recovery` failure labels now map deterministically to K/I/A/F, while ambiguous native-tool failures stay **undetermined** instead of being falsely attributed to the model. This prepares honest target-local-model comparisons without claiming the benchmark has run on the user's GPU.

### CI
- Dedicated native matrix for **Linux and Windows** compiles the registered tool bridge, artifact checker, model/read-only workflow and malicious/no-authority regressions; source-tree verification required on each OS.
- Passing CI can establish source/native fixture checks, not physical installed NSIS/GGUF/MMProj, host-owned external-app execution, Python isolation or GUI/E2E acceptance.

## 2. Recommendation-by-recommendation disposition

| Pxx | Owner classification mapping | This batch | Next real gate |
| --- | --- | --- | --- |
| **P01 Filesystem** | Foundational I/A | **SKIP expansion**; current scoped discovery + filesystem API already present. New artifact inspect composes existing approved I/O. | Verify installed source freshness and scoped path cases |
| **P02 Scheduler/Guardian** | Foundational A/F | **SKIP duplicate scheduler.** Existing host worker, resource admission and journals preserved; no reuse bypass. | Independent process-tree/restart installed tests for any future external executor |
| **P03 AI Operations** | Foundational K/I | **IMPLEMENT** truthful authority-bound tool classification/readiness metadata; no hardcoded Office/Git reasoning. | Target local model actually uses inventoried tool correctly |
| **P04 Reader/OCR** | Foundational observational I/F | **SKIP parser duplication.** Existing Reader/OCR handles source extraction. New inspector tests produced files independently. | Full large-file/visual extraction remains separate and may require owner prioritization |
| **P05 Browser/Transfers** | Foundational I/A | **SKIP new network write/transfer privilege**; keep existing scoped browser and transfer paths. App adapter cannot provide egress. | Actual source/origin/credential/transfer acceptance before adding general network operations |
| **P06 Observe & Learn** | Optional foundational I/A | **SKIP expansion.** Consent, privacy and source-identity scopes unchanged. | Physical capture → local GGUF/MMProj and consent/egress tests |
| **P07 Research** | Foundational source evidence + utility formatting + specialist publishing | **SKIP native citation expansion.** Retain evidence and limited style export. Inspector can check output bytes/format, not citation truth. | External Zotero/CSL app operation only after reviewed app adapter and independent source/style verification |
| **P08 Archives** | Utility + foundational verified I/O | **SKIP format expansion.** Existing split/manifest/inspection preserved; new ZIP structural inspection reuses ZIP package layer without extraction. | Original archive hash/reassembly and actual installed filesystem acceptance |
| **P09 Office** | Foundational document I/O + utility edit + specialist authoring | **SKIP Office clone.** New read-only DOCX/XLSX/PPTX structural inspection detects zero-byte/bad package outputs. | Real Office application open/render/formula/semantics acceptance remains OPEN |
| **P10 Image** | Foundational image observation + utility transforms + specialist graphics | **SKIP graphics expansion.** Inspector validates bounded decoded PNG/JPEG; current basic immutable edits kept. | Installed preview/export, opaque redaction, cancellation/restart |
| **P11 Git** | Utility Git inspection; IDE specialist workflows | **SKIP generic Git command/write features.** Current host-pinned, read-only Git remains. | Native Git exact-head and installed auth/adversarial tests; full GitHub workflow stays IDE |
| **P12 JSON/Protobuf** | Foundational A/F boundary | **SKIP expansion.** Strict existing JSON/Protobuf input/output validation retained. New tools use typed canonical Protobuf and strict model argument schemas. | Future compiler/runtime regressions only; no standalone JSON app |

## 3. What is still genuinely outstanding (do not misreport as finished)

1. **General effectful code/application execution (I/A):** The host-only worker supervisor currently runs trusted reviewed plans, not arbitrary model-selected commands. This batch establishes only an inert/read-only app action interface. Actual supervised Python/CLI/application workers, dependency discovery, OS-specific isolation, egress denial, no-overwrite artifact publication, task-scoped effect approval, process-tree cancellation and durable crash recovery must be implemented and independently tested before enabling ExecuteCode/WriteWorkspace.
2. **External result semantics (F):** ZIP structure and source hash are not the same as workbook correctness, rendered slide layout or a successful GitHub API mutation. Independent application-based oracles, version comparison and output inspection remain to be composed.
3. **Installed-app acceptance (I/A/F):** GitHub CI Windows runner is not an installed Lu instance with operator approval, local GGUF/MMProj/AMD GPU, application-picking UI and real OS process containment.
4. **Local-model benchmark (K):** No independent benchmark result for Qwen local quantization, live app use or complex Office/Git tasks was obtained in this source-only batch. Run model-vs-general-exec-vs-utility comparisons only in an isolated approved environment.
5. **P04/P05 broad scope:** Existing source paths are substantial; the previously deferred huge-document streaming and shared transfer manager are not silently reclassified as complete or included in this change.

**Outcome:** Cross-cutting model-facing artifact validation and a reviewed app interface contract have actual source implementation. P01–P12 specialist micro-app expansions are intentionally skipped where the source audit found sufficient existing utilities. The broader general execution layer remains a major separately gated implementation, not a completed feature.

## 4. No unreviewed behavior changes
No autonomous GitHub operations, no credential use, no general shell, no new native execute permission for Lu sessions, no unapproved network, no automatic installation, no existing tool deletion, and no modification of earlier PR acceptance evidence.
