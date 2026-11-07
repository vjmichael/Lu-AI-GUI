# Lu P07 — RESEARCH-STYLE-01: Evidence-Bound Scholarly Citation Profiles

**Status, 2026-11-06:** P07 first native implementation, stacked on P06 at commit `730600a9e88628c4794564009fb58020e0c80c2c`. P06 privacy source implemented, exact-head/physical acceptance separately tracked; P04/P05 remain deferred.

## Implementation

New `src-tauri/lu-core/src/web_research_styles.rs` provides deterministic, host-only citation rendering tied to a pre-existing `EvidenceLedger`. Style selection is typed and versioned: APA 7, MLA 9, Chicago Notes–Bibliography 18, Turabian Notes–Bibliography 9, and SBL Handbook 2. The current formatter deliberately covers **ordinary books and journal articles only**. It is not claimed to be a complete CSL engine, manuscript formatter or peer-reviewed academic citation.

It accepts an ID/title/publisher URL that must match a **METADATA_CORROBORATED**, **DOCUMENT_ACCESSED**, or **PASSAGE_VERIFIED** ledger row. Discovered leads, mismatched identity, missing publisher/year, unsafe text, and unsupported ancient primary sources are rejected. Metadata for author/date/volume/journal still must be supplied by a host-reviewed catalog adapter; **the style engine does not independently corroborate additional fields or allow model-authored claims to promote them**. SBL journal abbreviations must be provided by a separately verified source, never guessed. Turabian 9 and SBL 2 book formatting requires a known publication place; Chicago 18 does not.

The result carries an explicit limited-coverage label, `bibliography_identity_corroborated=true` only after ledger checking, `quotation_or_page_verified=false`, and `factual_claim_verified=false`. **No page/verse locator is fabricated.** Styles do not substitute for a publisher edition, true passage verification, or human scholarly review.

## Acceptance

New Windows/Linux native CI compiles and tests all five style paths, recognized scholarly-source identity, missing fields, unsupported ancient witnesses and SBL abbreviation refusal. Existing EvidenceLedger verification tests and exact source-tree SHA checks remain required. No installed application or user-facing complete bibliography export is claimed in this first P07 wave.

## Next P07 work

Build reviewed metadata adapter including author/date/container/publisher provenance and immutable source identity; add CSL-compatible structured export (without promising full CSL rendering); support books, journal/chapters and web resources as separately tested styles; ancient works/Scripture, original-language transliteration and SBL exceptions require explicit source/edition/abbreviation maps; connect final publication-style preview and verified report export to Lu Research without letting browser/model prose mint references. Retain human review and uncertainty flags.

## Style edition background

- Chicago Manual of Style, 18th edition (2024), notes-bibliography
- Turabian ninth edition is the installed current profile here; the tenth edition is listed by its publisher for December 2026, so it must **not** be silently substituted before its release
- Society of Biblical Literature Handbook of Style, second edition (2014), with specialist exceptions
- MLA Handbook ninth edition; APA Publication Manual seventh edition

All edition and example claims require verification against official manuals before release. The narrow formatter returns bounded placeholders **never** in place of missing data; it refuses to render unsupported formats.

## P07 source-bound authors and CSL-JSON-compatible export

Further native source now cross-checks the exact ordered author list against the original discovered author identity that `EvidenceLedger::corroborate` previously matched to the trusted catalog. A model-substituted author cannot be rendered under the verified source ID. `export_csl_json` produces a structured, bounded CSL-JSON-compatible book/journal reference with authored title/URL, author name fields, explicit year and only supplied publisher/journal/volume/issue. It does **not** invent a page, ancient-text locator or SBL abbreviation. The `_lu_evidence` extension explicitly marks bibliographic/author identity corroborated while publication details, passages, pages and factual claims are **NOT independently verified** pending an enriched host catalog adapter. This structured export is a native function, not yet a Research GUI command or complete citeproc rendering.

## P07 existing evidence-ledger regression repaired

The original Research evidence unit test claimed a source ID containing a newline must be rejected, but passed the *literal* backslash and letter `n` instead. The metadata validator correctly rejects real control characters; the test had the wrong escape. Corrected it to a real Rust newline escape so the existing `EvidenceLedger::discovered_candidate` test verifies the intended fail-closed behavior. No broader identity syntax permissions were introduced.
