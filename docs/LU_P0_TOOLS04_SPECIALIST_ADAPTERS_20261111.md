# Lu P0-TOOLS-04 — Protected specialist application adapters

**Date:** 2026-11-11
**Development branch:** lu/p0-tools04-specialist-adapters-20261111
**Parent source:** 2de2845eb4fac4382642b864971118379c56387c
**Status:** source implementation candidate; compilation and runtime acceptance deliberately deferred.

## Implementation

Lu's genuine model-to-native reviewed Git/FFprobe worker pathway already has a
strict tool registry, original ANSF human-turn binding, one-use ExecuteCode +
ReadWorkspace authorization, supervised Windows Job Object / Linux process
custody, durable TaskStore and EffectJournal, cancellation, and independent
source oracle. This work extends that pathway to **two additional installed
specialist utilities** without building another PDF or image application.

1. **Poppler PDFInfo:** The user selects one local PDF with an OS picker,
   sees the exact SHA-256 and PDFInfo executable digest in a native approval
   dialog. The fixed worker runs PDFInfo with an ISO-date option and this
   single selected PDF only. Normalized output has page count and PDF version;
   independent original-byte parsing verifies PDF header and the existing
   conservative classic PDF page-tree count. Encrypted/object stream and other
   unsupported PDFs are marked UNSUPPORTED rather than invented as verified.
2. **ExifTool:** The user selects one local PNG/JPEG. The fixed worker requests
   only file kind, dimensions and file size with numeric JSON formatting. The
   host rejects arbitrary metadata fields, malformed/oversized output, tags,
   errors, and conflicting facts. An independent image parser reopens original
   approved bytes to check image type, width and height, then records a
   method-specific VERIFIED, CONFLICT or UNSUPPORTED outcome.

Both tools use exactly one source and a strictly compiled argument vector:
no model paths, scripts, binaries, shell, arbitrary execution or filesystem
writes. The reviewed process supervisor enforces the exact argument pattern,
source type/hash, 8192-byte stdout and 15-second deadline, and preserves
existing cancellation and post-restart uncertainty policies. Raw paths and
worker stdout are not exposed as a model result. External applications are
optional and must exist at narrowly allowlisted installation paths; there
is no PATH or download fallback.

An explicit original-human request about PDF pages/version or PNG/JPEG
dimensions can now offer the existing native picker and separately reviewed
specialist workflow from normal chat. The loaded local GGUF only selects
a zero-argument tool from the fresh per-turn catalog. It cannot authorize
the next operation, grant network egress or bypass the native review dialog.

## Independent evidence and safe state

The first process receipt is a historical worker outcome, not independent
fact proof. The original source is sampled *before* execution and rechecked
after worker completion. A second host parser compares the utility's
normalized output to independently opened file bytes, records its verdict
in the original EffectJournal and only then allows model feedback. Source
drift, missing/uncertain worker custody, unexpected data or conflicting
content fail closed. No automatic retries are introduced.

The existing planned agent-step vocabulary remains Git/FFprobe only. New
specialists are fresh standalone requests until a NEW reviewed plan revision
admits their exact method and independently verifiable criterion. Historical
grants/plan approvals cannot be reused. The existing Git/media memory
projection does not yet classify specialist outcomes; their original
TaskStore/EffectJournal/ModelTurnJournal records remain authoritative until
P0-TOOLS-05 links them to fresh project task evidence.

## Remaining limitations

This is **source implementation only**. PDFInfo and ExifTool are optional
installed programs. Actual source compilation, native Windows/Linux UI,
physical Radeon/local GGUF selection, process custody and oracle behavior,
cancellation, restart preservation and regression tests have not been
executed or claimed. Lu Core is not itself a sandbox and supervised fixed
arguments alone do not prove an external application has no network egress;
no network capability is granted in this workflow.

## Next engineering sequence

- **P0-TOOLS-05:** Match independent exact-method worker results to current
  project-task requirements with new source review and separate human
  acceptance; never auto-complete tasks.
- **P0-MEMORY:** Extend ANSF goal, procedure and corrected-belief continuity
  to new specialist evidence, without importing historical permission.
- **P0-AUTH:** Close cross-domain worker custody, revocation and restart gaps.
- **P0-GATE (deferred):** Consolidated compiler/GGUF/installed Windows/Linux,
  CI, PR and regression review only after application implementation.

**No CI, PR verification, merge or release was requested or performed.**