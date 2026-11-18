# Lu P0-TOOLS-03 — Shared native-source provenance in original ANSF

Date: November 11, 2026. Lu Core only.
Parent: 314e88e3a989eba02f9ec2620fbc607a026cb293
Branch: lu/p0-tools03-ansf-source-evidence-20261111
Status: source implementation candidate; native compiler, genuine model, installed Windows/Linux and regression acceptance deferred by owner.
No CI submission, PR creation, PR verification, merge or release authorized.

## Existing functionality reused

- Browser: exact public HTTPS-origin scoped real headed Chromium native tool; model proposes limited reads, host checks actual post-redirect URL and persists exact-page text SHA in the independent Research ledger. Citation audit is a separate step; a page observation is NOT a verified citation or fact.
- Reader: actual OS picker, pinned byte snapshot, bounded PDF/Office/EPUB/text extraction, source SHA, per-turn Protobuf model tools and cancellation.
- OCR: separately approved pinned offline Tesseract and, when installed, pinned PDF rasterizer; selected image, original attachment, bounded crop and 1–8 PDF pages. Results are UNVERIFIED TRANSCRIPTIONS.
- Observe: OFF by default, two-step native consent, per-capture OS selection and local-only process custody; human-reviewed enum-only diagnostic handoff. No network egress or automatic model training.
- ANSF: authoritative per-chat SQLite books and single profile ANSF Library database. No new memory database or command/execution permission.

## Source integration implemented

1. ANSF Library extension: an append-only typed native source index is created inside the original ansf/library.db. Each host-created opaque source ID links one existing conversation to a kind, original turn if applicable, source/observed SHA256, bounded locator, privacy flag and historical status. The original JSON record has its own SHA256 checked on read. No model or WebView can write records.
2. Browser: only after genuine native tool output, post-redirect HTTPS scope enforcement and original Research receipt does the host record the actual public URL, text digest and a bounded untrusted excerpt in ANSF. Both direct model reads and the cited research workflow are included. Navigation alone is not evidence of source text.
3. Reader: both genuine single-document Reader Q&A and the combined P0-TOOLS-01 workspace model turn record original document-byte SHA and a sanitized, byte-bounded display basename only after an actual, successful native Reader tool call. The workspace Reader invokes a new host-only post-dispatch source observer with the exact relative file it opened, after path/symlink checks and document parsing; the trusted Lu desktop binds that result to the current persisted ANSF human turn, deduplicating repeated pages of the same file. No source record comes from a discovery hit or final model answer. The model does not receive an actual OS root or another grant.
4. OCR: after actual successful, independently pinned local OCR from selected, attached, cropped or PDF-page input, the host checks the resulting UTF-8 transcription digest and records ONLY original-source SHA, recognized-text SHA and a fixed kind/page locator. No transcription, pixels, PDF path, uploaded original or fake human turn enters ANSF or future model context. The operator still must review original pixels and opt to insert text in a separate chat draft.
5. Observe: a separate explicit native confirmation is required to index the SHA of an enum-only private handoff. The host recomputes the same session and hash after the dialog. No screen bytes, window titles, clipboard, notes, text or verified GUI test verdict are stored in this source index. The index is private and never eligible for model context. Forget Observe Notes deletes indexed private Observe metadata BEFORE clearing the diagnostic notes; the UI discards its cached copy.
6. Model continuity: a NEW ANSF-authenticated human Send explicitly asking about earlier sources can see a bounded, historical, untrusted list of this chat's Browser/Reader receipts. The host never makes an inference turn, tool call, grant or network request from receipt history. OCR and Observe remain excluded even when requested. Large payloads are omitted rather than interrupting normal chat. Source freshness always says RECHECK.
7. Desktop: the existing Native Tool Activity pane now includes a read-only original ANSF native-source view and an optional native-only Observe source-metadata review flow. Private OCR/Observe rows display only source identity and digest. No renderer-created source is admitted.

## Non-authority and feedback semantics

A source digest proves that bytes *were observed* in one native operation, not that their factual meaning was correct, that a URL/document is unchanged now or that this model may execute tools. A Reader model answer is not an extraction oracle. OCR is unverified. Observe handoff is not a GUI test. All new records have source_is_untrusted=true, execution_authorized=false, and historical freshness.

The ORIGINAL Research ledger, Reader turn journal, OCR native worker, and Observe TaskStore remain the independent execution/effect sources. The ANSF source index is a read-only PROJECTION of authenticated, completed native observation paths. It never overwrites those journals or adds artificial User turns.

## Work left after this bounded group

- The multi-document workspace Reader now publishes individually observed source digests using a trusted native post-dispatch callback. A separately opened Reader document without an associated chat (the raw read-only picker) remains intentionally unindexed: no conversation identity can safely be inferred.
- OCR transcription must remain user-reviewed before any later model use. No automatic OCR-to-chat copying was added, deliberately.
- Observe source digests are private and explicitly approved; they do not attest independent GUI tests or pixel inference. Existing supervised capture/vision remains separate.
- P0-TOOLS-04: reviewed specialist app/worker adapters, host custody, durable cancellation, real independent semantic feedback.
- P0-TOOLS-05: source-rechecked, separately human-approved project-task acceptance candidates from independently witnessed outcomes.
- P0-MEMORY: unified project and source continuity, belief corrections, task recovery in normal model turns.
- P0-AUTH: cross-domain authority and crash/uncertainty reconciliation.
- Late P0-GATE: batch compiler, native physical Windows/Linux, genuine loaded GGUF, CI/PR and regression acceptance only after app build phase.

Source-level test cases were added for malformed source identities/digests, original read tampering, private OCR/Observe exclusion and forgetting. They are NOT executed under the owner's implementation-first development workflow.

Stop after P0-TOOLS-03 source work; no CI or PR verification.
