# CP-06C — Provenance-aware exact observation presentation (2026-11-19 UTC)

Status: **Local exact-observation factual presentation and protected finalization PASS; full behavioral/installed acceptance OPEN.** This is not a general semantic reasoning verifier. Qwen2.5 still abstains from browser selection and fails the complete 17-case fixture's terminal assertion.

## Implementation
- Current baseline f9be5bb1ae182a55ab855fbaccbf8c1614a1d790 (CP-06B-R1). Preserved existing authenticated original turn, live scope, model tool selection, typed native invocation and user review. No grants added; no automatic redispatch. The restored offline Linux build, Rust 1.99.1, pinned dependency caches and actual CPU llama.cpp engine from R1 were reused.
- Twelve explicit single-observation read-only capability contracts now render verified result facts and limitations deterministically: exact file text, folder listing, metadata, text search, discovery, archive members, artifact SHA/size/structural level, Git status/log/diff, environment file presence, and opt-in exact Reader excerpt. Existing two-way strict schema-validation verdict remains separate. Unmatched or multi-step/summary Reader paths retain their original reasoning and pagination; they are not certified by this change.
- The approved exact Reader file is bound by both the current-input catalog and an independent byte-identical Protobuf authorization check. No model path, inspected-only claim, fabricated selector or offset. Exact excerpt requests report incomplete or truncated content. No change to ordinary multi-page Reader Q&A.
- Host builds a bounded factual presentation from the one actual host-decoded current native feedback, checks result and call identities, and asks the model to reproduce the presentation. Strict final envelope and exact text required. Returned data is quoted/fenced, and control characters escaped for display; incomplete searches, source truncation, archive integrity limitations, metadata-only discovery, bounded history, structural-only validation, and environment non-execution are explicit. Oversized presentation reports that a complete answer could not be verified rather than inventing completeness.
- A nonidentical or malformed received final after successful current observation is rejected, journaled, and replaced by the labeled host evidence presentation only after a fresh continuation/authorization check. The fallback is not model-generated success and adds no native calls. Invalid/absent native evidence, cancellation/revocation and inference/runtime errors do not authorize fallback. They retain the existing failure path. Exact matching is deliberately conservative, even with harmless formatting differences. Exact reproduction demonstrates reproduction, not independently verified free-form reasoning.
- New EVIDENCE_PRESENTATION journal kind binds current task, tool call ID, tool name and SHA-256 of canonical decoded feedback, final displayed text, and origin (model_exact_reproduction or host_evidence_fallback). On host fallback no FINAL_ANSWER row is minted. Existing SQLite audit table migration preserves original records and sequence IDs. History never treats fallback as a completed model-generated final. No new engineering controls in end-user UI.

## Real local results
| Check | Qwen3 0.6B Q4_K_M | Qwen2.5 1.5B Q4_K_M |
| --- | ---: | ---: |
| Inert capability selection (17 cases) | 17 | 16 |
| Native local operations | 13/13 | 13/13 |
| Factually verified presentations | 13/13 | 13/13 |
| Exact model reproduction | 10 | 5 |
| Strict native-grounded schema model verdict | 1 | 1 |
| Labeled host evidence fallback | 2 | 7 |
| External selection-only (no external execution) | 4 | 3 |

R1 pin: Qwen3 SHA256 7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d; Qwen2.5 SHA256 6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e; CPU llama-server SHA256 e80da2e586dda007c0e0616a945745cb5682891788b0da5d58d7cc29477897be. Both genuine matrix processes actually loaded the original models through Lu's supervisor, used real native registry/codecs, and were run sequentially on the same runtime source. Qwen3 exited PASS, 87.41s; Qwen2.5 original full matrix exited FAIL solely on browser abstention, 181.85s. Neither actual browser nor specialist execution was qualified. Each local operation executed exactly one selected model-directed tool request and preserved the fixture facts. Test-host approval is not installed OS picker consent.

Independent read-only reopen of 34 selection/tool pairs (68 audit SQLite files) and 34 ANSF conversation-book SQLite files: integrity OK; independently checked live synthetic file bytes/size, root inventory, search, paths, archive member list, artifact hash, Git repo state/log/diff, schema verdict and environment evidence limitations. Matched call ID and canonical feedback digest, exact presentation origin, raw model output, no fabricated FINAL_ANSWER for fallback, one original human record, one completion event for each observed local case, persisted displayed text. No external dispatch. Both prior environment overclaims are now blocked; prior archive omission and partial-search omission corrected. Qwen3's two rejections were benign quoting differences; Qwen2.5 needed seven conservative fallbacks. No fallback counted as model reasoning.

## Regression
- Core lib: 678 passed / 0 failed / 28 ignored (including added scope, cancellation, literal, migration and fallback tests).
- Desktop lib: 412 passed / 0 failed / 13 ignored.
- Four existing native integration cohorts: 27 passed / 0 failed (host_reviewed_route_durable 8, native_tool_lifecycle 4, native_workspace_read_turn 4, supervised_native_turn 11).
- 1,117 passing ordinary unit and selected integration test functions total; the real dual-model matrices counted separately. No claim all ignored suites passed. Exact published runtime source blobs match tested local files, with test-only additions validated by Core suite. The local reconstructed worktree contains historical metadata from prior recovery, not the new remote doc/lock tree; code identities transferred and compared individually.

## Remaining
CP-06C exact local grounding subgate complete; open-ended free-text/multi-step grounding remains a separate design and acceptance concern (CP-06C-R1). True OS-installed approval/negative lifecycle, large-document bounded pagination/semantic summaries, qualified external browser/specialist grounding and Qwen2.5 browser selection remain open. Then CP-06D host-confirmed effect continuation, CP-06E latent capability dispositions, CP-06F consolidated installed acceptance. No main merge, new branch, PR, manual Actions dispatch or release authorization.
