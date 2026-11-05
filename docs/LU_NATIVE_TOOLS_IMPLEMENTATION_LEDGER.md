# Lu Core native tools — implementation and acceptance ledger
Updated: 2026-11-05
Source integration branch: `lu/tools-all-source-consolidation-20261105` (PR #40)
Status: DEVELOPMENT ONLY. No claim of stable v1.0 or installed Windows acceptance.

## Non-negotiable acceptance states
- **Source** = a named implementation exists and has source review coverage.
- **Native verified** = exact-head platform compiler and behavioral tests passed.
- **Desktop integrated** = ordinary or explicit chat flow resolves a trusted user turn, schema, authority, host policy, engine, and cancellation.
- **Recovery verified** = interruption/uncertain effects are observed and never replayed without evidence.
- **Release accepted** = actual installed Windows app + GGUF/backend + real file tests passed.

Do not equate passing file/string assertions, having a Protobuf descriptor or receiving a GitHub queued CI job with any of the last three states.

| Domain | Source state in this stacked tree | Missing source or integration | Highest-value next acceptance |
| --- | --- | --- | --- |
| Native file read/list/stat/search | Existing Lu Core modules from #34 | Ordinary chat routing; broader target selection; permission UI | Host-selected grant, model call, multi-chat revoke |
| Create-new text Write | Existing journaled atomic creation | Ordinary model tool grant UI, exact approval receipts | Installed Windows real file + restart |
| Existing-text Edit/rollback | #5 source and #34 descendants | Host full-diff review UI, NTFS metadata and cross-process races | Windows crash injection across all publish boundaries |
| Copy/move/rename/trash/restore | #36 merged into this integration source | Typed Protobuf adapter, true persisted grant, effect journal, paths beyond flat same-volume source | No-clobber + ambiguous unlink reconciliation |
| Archive listing | Existing #15 / #34 | Genuine installed model call, broaden bounded formats only after safety review | ZIP adversarial tests across OS |
| Archive creation | #37 merged into this integration source | Model-facing exact-target grant, journal/recovery; folder and extraction support | Real ZIP compile and crash test |
| Spreadsheet creation | #39 merged into this integration source | Desktop approval UI, existing XLSX edits, style-preserving round-trip, sheets and formula recalculation | Exact-native Protobuf approval/write/receipt tests |
| Universal Reader | #27 / #34 | Rich PDF, scanned OCR, per-range XLSX, Office producer fuzzing, streams | Real GGUF Reader and Windows OCR |
| JSON/schema | #12 / #16 | Reconcile drafts, consistent strict MBRL boundary in ordinary chat | Exact-head source + platform matrix |
| Git inspection | #14 / #34 | Host-selected repo grant and Code-task integration; advanced coding reserved for IDE | Hostile config/worker and installed acceptance |
| Environment discovery | #17 draft | Real availability/version probes, approved provisioning | Host platform probes with no implicit authority |
| Vision/OCR | #18 draft | Real MMProj route, reliable Windows OCR, packaged Tesseract | Installed MMProj recognition and fallback |
| Tool/stream loop guards | #19 draft | Apply progress guard to all backends, durable checkpoints | Real looping GGUF and subprocess termination |
| Controlled execution | #34 foundations | Complete context-bound authorization, process cleanup, worker stdout contracts | Windows actual native worker cancel/restart |
| Background task control | #34 foundations | Durable result handoff, automatic model continuation with authority fence | Restart and multi-chat lifecycle |
| Browser/research | #26 draft | Verified browser binary packaging, worker crash recovery, larger research workflow | Real read-only browser + human challenge |
| Image Workspace | #35 draft | UI, crop/resize/annotation, masks, editing backend, immutable revisions | Installed native deterministic edit and crash recovery |
| Model/backend lifecycle | #20 draft | vLLM desktop integration, real llama cancellation and GPU release | Actual installed llama/ROCm/Vulkan stop |

## Exact branch graph and ownership
- #34 is the existing unified tools foundation. Its CI concurrency group was repaired to avoid cross-branch cancellations. It retains the original source.
- #38 (read-only multi-tool workspace and GUI `Ask Workspace`) stacks on #34.
- #39 (typed spreadsheet writer + exact Protobuf approval/receipt tests) stacks on #38.
- #40 combines the native feature files of #36 (copy/move/trash) and #37 (ZIP creation) onto #39; its merge commit preserves both parents. No release merge.
- #35 Image Workspace remains a standalone concept/validation slice; it is **not a working editor**.
- #26 browser, #18 OCR, #19 loop guard, #20 backend are distinct drafts and must be reconciled deliberately.

## Immediate next actions
1. Compile the exact SHA of #40 on Linux and Windows; repair compiler and test failures, including Protobuf and ZIP.
2. Add reviewed desktop write proposal for the spreadsheet creator: user sees filename, sheet/cells, hash and target; grant cannot be synthesized from model output.
3. Implement format-aware **existing** XLSX/CSV edits using safe revision baselines, prove unrelated sheets/styles preserved; do not silently fall back to destructive whole-workbook rewrites.
4. Complete safe ZIP extraction and filesystem transfer durable effect adapters with never-replay-on-uncertain semantics.
5. Finish Reader PDF/OCR and actual MMProj routing; unify default chat capability discovery and tools under consent controls.
6. Cross-platform crash/kill matrix and actual installed Windows/real GGUF evidence.
7. Only after all relevant gates pass, promote the integrated source into the release branch.

Known limitation: no working Rust toolchain is available in the current container, and GitHub Actions runs observed for this head were queued, so code publication is **not** compilation or test success.
