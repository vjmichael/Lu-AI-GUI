# CORE-MERGE-C1 — exact-source reconciliation of #228/#229 and Observe, Reader, Browser (November 14, 2026)

**Scope:** Existing Lu Core PR #235, frozen start `2fb253ab35bc3b451b9c07b89f5549aec26c32e3`. This is a historical/source-behavior disposition record supporting **one canonical development ledger**, not a new task board, authorizing manifest or final acceptance.

## #228 read-only P0/R00 source bridge: worthwhile unique capability recovered
- The #228 tip `af56b2c09c9b5a1f0e59b290d58fcdc94d936855` had five paths absent from #235. Three are a *single read-only source audit unit*: `scripts/core01f-bridge-review.mjs`, `scripts/core01f-bridge-review.test.mjs`, `docs/LU_CORE01F_P0_BRIDGE_BLOB_REVIEW_20261112.json`. Restore the exact original Git blobs to the existing #235 tree without importing an older supervisor, tool or execution boundary.
- Historical manifest anchors immutable R00 `e61d4bf998bccf51074b01c864df208c233eb2f8`, P0 bridge `cc312e281b5989a0a9fb972eb04b9e34215a77da`, and frozen CORE-01E `97dd1e897c4cf73a1fd8d8bf36f1233b081322c0`: 125 bridge paths, 105 exact at historical CORE-01E, 20 subsequently modified, zero deleted. **These numbers describe the frozen historical three-tree audit, NOT current #235 byte parity.**
- The original verifier checks identity, sorted paths, excluded IDE/P7L/KoboldCpp paths, actual historical Git ancestry and original Git tree blobs, rejects forged merge/release/native flags, and does not write refs or start workers. Its original seven tests remain source tests pending batch execution on the newly frozen #235 SHA.
- The remaining #228-only documents are older audit status snapshots with now-obsolete main and CORE-01E heads. Preserve their original PR links and Git object identities as historical evidence; do not install an old backlog or pretend their earlier SHA accepted current C4.

## #229 integration gate: historical evidence, no missing production module
The #229 tip `6e9c4e4a7d30677d167119606dceb43800db6659` adds only `docs/LU_CORE01F_03_04_INTEGRATION_GATE_20261112.md` absent from the current candidate tree, besides changed older source snapshots. This original document records 55 Linux UI failures, missing worker provisioning and an installed-test skip at an **earlier** CORE-01E SHA. The canonical `main` ledger and current C4 original run are newer. Keep #229 as an independently queryable historical receipt, not a current acceptance result or a replacement for `main`'s ledger. Source-lock and protected-main reconciliation remain mandatory at merge.

## Observe & Learn #61 (exact tip `9b6a886caddaa221e13933446fe0100afe77cf90`)
All original production/test paths examined are already represented in current #235; no absent path requires a wholesale port. `native_observe_ansf.rs` retains 6/6 originally named functions; `observe_desktop.rs` retains 20/20 names and adds verified reaping, no-network observation and revoked-consent reconciliation. Current `supervised_tool_generation.rs` retains all 21 original named functions and adds nonexecuting feedback on malformed model replies. **Names and paths do not establish equivalent code behavior.** User consent, capture authorization, privacy, cancellation, crash receipts and forgetting must continue to be tested by their original native/desktop tests; source-only similarity cannot replace them.

## Reader #27 (exact tip `5fef956d6a81fb2c9843e116465847fa1251f690`)
No original Reader runtime path is absent from #235. `native_reader.rs` retains 12/12 function names; `native_reader_tool.rs` retains 24/24 and adds registered host observation and nested-symlink denial; `native_reader_pdf.rs` retains 30/30 and adds an independent page-count check; `native_reader_openxml.rs` retains 12/12 and extends EPUB support. All four original named desktop Reader tests persist, and the current desktop test also adds reviewed OCR, source-selection and format cases. Preserve file picker, UTF-8 prompt budget, source pinning, nonexecuting observation receipts and no implicit model path authority. Original format-parser source semantics and hostile fixtures still require the grouped native regressions.

## Browser #26 (exact tip `19af8fe3e411e4a4175e285ce1c6c2130c36fe4d`)
Original browser worker tests retain all 15 named cases and add nine challenge/WebSocket/credential-handling negatives. `browser_worker_supervisor.rs` preserves 20/20 old function names and adds SEC-06C owned-worker, source-hash and durable recovery handling. `browser_model_codec.rs` preserves 8/8 and adds model-catalog exclusion of screenshot bytes and interactive controls. Older browser-desktop test names were reorganized (16 older top-level tests vs 11 current regrouped authenticated/reviewed-contract titles); **title mismatch is an audit gap, NOT evidence that all 16 original behavioral contracts were proved equivalent**. Direct implementation review confirms the current `BrowserOriginScope::from_host_grant` rejects empty or >32-origin grants, refuses noncanonical/publicly unsafe origins, and `allows` requires exact `https://` hostname membership after redirect; current `browser_model_desktop.rs` uses this scope for research and the native read result. The browser model remains read-only and final answers must pass verified citation audit. This is a source-level safeguard, **not** execution proof. Inspect actual authorization, human takeover, first-party process custody, origin firewall, verified citations and package provenance under original Windows/Linux tests. The original #26-only workflow and packaging note are historical; current browser manifest explicitly says `not-packaged` and cannot be promoted to installed/browser acceptance.

## C1 → C2 selected integration: original Browser behavioral guarantees

Reviewed the **16** original TOOLS-10 browser desktop tests against the current
**11** split source tests, **not just their names**. The current host's
`browser_origin_list` preserves the original one-to-eight manually supplied
comma/semicolon/newline-separated public HTTPS origins, validates every value
through `BrowserOriginScope::from_host_grant`, displays the exact approved set
in a blocking native dialog, and passes the unchanged scope to the owned
Chromium worker and post-redirect model result. The ordinary natural-language
browse classifier is deliberately **single-origin**, and cannot authorize
cross-site research through model/user-text inference alone.

The current read-only Browser authority, verified user-turn binding,
revocation/cancellation, human-only challenge recovery, `cgroup`/Job supervised
worker, verified read/accessibility source IDs, exact citation audit, and bounded
model prompt are stronger or meaningfully equivalent *in the reviewed source*.
The original single-file test expressions cannot be pasted into the current
split `lib.rs` / `browser_model_desktop.rs` / Settings implementation without
false failures or weakening the newer protection.

**Selective integration rather than wholesale restoration:**
- `lu-desktop/src-tauri/src/browser_model_desktop.rs`: genuine Rust unit tests
  for the original 1–8-site source parser (positive and hostile list cases,
  rejecting implicit multi-site ordinary chat), plus real bounded prompt
  behavior (omitted sources and model no-navigation-citations instruction).
- `lu-desktop/tests/browser-native-authorization.test.mjs`: three current-API
  regression tests for original explicit multi-origin native approval,
  human-only challenge with cancellation, and source/citation bounds.

These tests are additive to existing Rust/browser worker/native tests. This
source review **does not claim executed test or physical browser package
acceptance**. Original Observe/Reader source operations are preserved; their
existing current native/desktop negative tests cover the old boundaries plus
newer privacy, OCR/format and symlink constraints. A final original same-SHA
test batch is still required before acceptance.

## Disposition
- **INCORPORATE:** unique #228 exact historical bridge verifier, manifest and hostile source tests; add one current-source family-regression contract test to guard newer stronger boundaries.
- **PRESERVE CURRENT:** existing Observe/Reader/Browser runtime, newer supervision and native tests. Do not overwrite protected source with older branches or introduce a new generic tool/guardian.
- **HISTORICAL ONLY:** #229 integration-gate note, older #228 main snapshots, #26 packaging/CI narrative. No current runtime/OS acceptance deduced.
- **REMAINING BEHAVIOR REVIEW:** renamed Browser desktop tests and detailed same-path semantics; current grouped Windows/Linux full original suites, installed evidence and Linux delegated kernel physical gate remain independent. No new feature branch/PR; batch CI after a coherent source lock, not after each blob.
