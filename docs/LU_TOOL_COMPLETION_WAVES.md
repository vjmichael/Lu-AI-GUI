# Lu Core Tool Completion — implementation waves

2026-11-06. This work is intentionally **source development first**; GitHub native/desktop/installed-app tests are independent downstream acceptance. Stacked on draft #61, not merged to the alpha. Older PR and CI cleanup is deferred pending a replacement integrated candidate. This plan **does not** authorize weakening security boundaries or adding unreviewed runtime dependencies.

| Wave | Implementation target | Definition of source completion (separate from CI acceptance) |
| --- | --- | --- |
| A — Immutable Image Workspace | Deterministic image transforms, solid redaction, region mosaic, graphical annotations, revision history and safe operator UX | No source overwrite or model-selected path; canonical PNG output with precommitted receipt; return explicit unsupported for generative edits |
| B — Reader and OCR | Pinned offline OCR adapter with image handling, explicit extraction provenance and user-visible review | No executable provided by model, no network, no OCR-as-source-of-truth without reviewing captured text |
| C — Archive / document sophistication | Existing format engines and transactional grouping improved without loss of originals | No claim of blanket encrypted/UDF/PDF editing support where the source parser is insufficient |
| D — Browser and Research | Durable source/claim relationships, publisher provenance, bounded large research pagination and report-quality safe output | No model-authored citation URL; no claim that observed exact passage proves factual truth |
| E — Observe & Learn | Minimized event-driven observation, safe redaction and independently reviewable handoff to larger model | OFF by default; explicit chat/session/target scope; no silent surveillance or fabricated GUI pass |
| F — Unified host lifecycle | Process and cancellation receipt hardening, consistent return/timeout bounds and Development Ledger integration | Native and installed test gates remain OPEN pending real GH CI and physical Windows |

**Execution policy:** Implement each applicable component on this branch, commit source changes, update the central `LU_NATIVE_TOOLS_IMPLEMENTATION_LEDGER.md`. No native tests are being executed within this work order; tests and old PR housekeeping are deferred as the user requested. Do not conflate untouched scope, mock-based source, or unsupported archive/Office cases with complete, released capability.
