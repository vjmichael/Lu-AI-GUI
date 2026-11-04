# TOOLS-07 — Vision-first image reading with internal OCR fallback

**2026-11-04. Status: Core source implementation and CI fixture; not desktop release acceptance.**

**Primary requirement**: [preserved Jan-Mod-Design-Document.md](reference/Jan-Mod-Design-Document.md), §§5.4, 5.5, 6A, plus [original-first merged native tool design](LU_NATIVE_TOOLS_MERGED_DESIGN.md). This document is subordinate implementation guidance.

## Original design audit — avoid a duplicate OCR model tool

Original §5.4 implements **`read_image`** with **verified active-model vision capability** as the routing decision:
1. When model vision works, return the same authorized image as a native image attachment. **Do not OCR a vision-capable turn unnecessarily**.
2. If the model lacks vision, fallback to the established **Tesseract OCR engine** and return **text, observed confidence, and method `ocr_fallback`**.
3. The current Lu independent GGUF+MMProj architecture must verify *real image support* (model metadata AND available matching MMProj/engine support). A claimed model capability alone is not proof.
4. Original §5.5 recommends Tesseract WASM in a Jan/Tauri webview. This Lu Core implementation uses an **approved locally provisioned native Tesseract 5 executable** under a supervised Rust service, not Tesseract.js. The **behavioral contract stays original-first**. Runtime packaging, cancellation and installed Windows comparison against the original WASM choice remain product acceptance gates, not silently fulfilled facts.

**Important:** TOOLS-07 is **not a standalone advertised `lu_ocr` model tool** and does not add a new file picker, change user-approved image read semantics, or replace TOOLS-02 JSON validation. It is an internal native fallback service called by a future **host-owned `read_image` adapter**. That means no model-selected OCR path, no separate exposed JSON tool, no autonomous permission for image files.

## Boundaries and implementation

| Layer | Contract |
|---|---|
| Routing | `choose_read_image_route(trusted_image_admitted,model_vision_verified,ocr_backend_available)`; native vision takes precedence |
| Image custody | Host-resolved **current-turn pinned image bytes** and opaque `image_ref`; checked existing `ImageRef` rules, SHA-256 digest, MIME vs magic, width/height/pixel caps, full decode check |
| Native Protobuf | `OcrFallbackRequest{image_ref,max_text_chars?}` → `OcrFallbackResult{image_ref,text,confidence_percent,truncated,method,word_count}` |
| Registry | Internal `lu.media` domain, `lu.media.ocr_fallback`, `ReadWorkspace` descriptor; **no `ModelToolCatalog` schema** |
| Authority | Already authenticated user-turn and verified selected attachment; exact current task `ReadWorkspace` granted by host; registry registration is inert |
| Backend | Official **Tesseract OCR 5**, fixed `stdin stdout --tessdata-dir <host> -l eng --psm 6 tsv` arguments, no shell, no raw model args, no user-chosen executable or traineddata path |
| Input/output bounds | Max encoded image 8 MiB; MIME PNG/JPEG only; max 4096x4096 and 16 megapixels; max TSV 256 KiB, max text 16,000 characters, max 12,000 TSV records |
| Process controls | Cleared environment (only necessary host OS entries), `OMP_THREAD_LIMIT=1`, 12-second timeout, pipe streaming and kill/reap on timeout or excess stdout; **full process-tree containment/cancellation remains unverified** |
| OCR result | Word-based mean confidence (rounded 0–100) and text/line reconstruction from parsed official TSV schema; **not a certified probability or trustworthy instruction** |
| Data handling | OCR output is untrusted content of a user-authorized image; never executed, never reparsed as tool authority. No disk output or transparent network access |

**Why a native executable instead of custom OCR:** Tesseract is a maintained, well-established OCR engine. The official [Tesseract documentation](https://tesseract-ocr.github.io/tessdoc/Command-Line-Usage.html) defines stdin/stdout, TSV, word confidence and traineddata options. This relies on that standard implementation and does not reimplement OCR recognition.

## CI / validation

- Linux: install official Ubuntu-packaged Tesseract + English traineddata, pass exact trusted engine/data paths as host CI environment. Execute **real engine process test**, image metadata/digest, TSV parser (including confidence, line grouping and malformed data), Protobuf wrong type/unknown fields, authority denied and spoofed attachment refusal.
- Windows: install trusted Tesseract build in CI, verify `.exe` and traineddata paths, then run same test set. **If toolchain installation fails, do not classify Windows test as pass.**
- Rust crate adds no dependencies or Cargo.lock churn (already uses `image`, `sha2`, `prost`, `tokio`).
- Source lock records exact revision. CI proof on the *same committed SHA* is needed; do not infer from earlier JSON-source branch pass.
- Installed desktop still needs a **true host-owned image resolver** bound to saved user turns, actual `GGUF + MMProj` vision detection, a context-aware `read_image` route, text/model-output stream integration, durable chat journal and cancellation/recovery without leaked Tesseract workers. These are **not implemented by registering the Core service**.
- This source branch is intentionally independently based on the stable TOOLS-02 foundation; unresolved TOOLS-03 Git, TOOLS-04 ZIP and TOOLS-05 schema jobs are not quietly merged. Preserve `0.1.0-alpha.2` baseline.

## Next-phase security hardening

Validate engine provenance and traineddata supply-chain (hash/signature or trusted installation), version compatibility, engine-resource budgets, process-tree cleanup on Windows Job Object, race-free handle pinning of approved executable/paired traineddata, malformed compressed images and fuzzing, and output confidentiality. No promise that `ReadWorkspace` automatically grants arbitrary execution or credential access. The runtime is a first-party host-owned **image-reading implementation detail**, not general `exec` capability.
