# CORE-01F-02 — P0 reconciliation bridge review and source disposition

**Decision:** SOURCE DELTA REVIEWED AND ACCOUNTED FOR. No runtime, native,
installed, physical-GPU, merge, or release acceptance is implied. No IDE,
KoboldCpp or P7L work enters the accepted Lu Core review scope.

## Verified immutable lineage

| Source | Commit |
| --- | --- |
| R00 Core / draft #200 | `e61d4bf998bccf51074b01c864df208c233eb2f8` |
| P0 reconciliation bridge | `cc312e281b5989a0a9fb972eb04b9e34215a77da` |
| P0-GATE / draft #223 | `e422d18cef10fdb1092578631b5fec692b3d7a4f` |
| P0-WIN-CHAT / draft #224 | `9b14dde7a01ad36208642be7536159ccb1f42e93` |
| CORE-01E / draft #226 frozen review | `97dd1e897c4cf73a1fd8d8bf36f1233b081322c0` |

The R00 → P0 bridge comprises **257 distinct ancestral commits** (253 on the
first-parent path, with additional ancestry in the reviewed merge), changes
**125 source paths**, and removes **zero** paths. The bridge has no separate
draft PR immediately preceding #223; these exact source hashes are its
explicit reviewed disposition. It must not be silently cherry-picked again.

A three-tree Git blob audit now inventories every changed path in
`docs/LU_CORE01F_P0_BRIDGE_BLOB_REVIEW_20261112.json`:
- All **125/125** bridge paths exist at the frozen CORE-01E head.
- **105/125** are byte-identical to the bridge Git blob.
- **20/125** were modified by later descendant commits; each has both the
  original bridge SHA and the reviewed CORE-01E SHA recorded.
- **Zero** paths were lost, deleted, or classified as IDE/KoboldCpp/P7L.
- Every source record includes the original R00/bridge/Core Git object
  identities. This is a stronger provenance statement than filename-only
  comparison, but it is not evidence of executing each feature.

The review contains **18** build/provisioning/acceptance paths, **20**
regression/model-witness paths, **19** design/acceptance records, **21**
cross-cutting Core/desktop integration paths, **9** ANSF/source-memory paths,
**6** llama/model lifecycle paths, **9** authorization/custody/effect paths,
**19** project/planning/human-review paths, and **4** desktop presentation
paths. Totals: **125**.

## Source-level subsystem dispositions

| Domain | Source review | Runtime acceptance |
| --- | --- | --- |
| Authorization and worker custody | `ConversationAuthority::with_authorized_worker_launch` keeps scoped reviewed Git/FFprobe/specialist registration inside the revocation fence; current desktop cancellation adds immutable intent/watermark and exact-chat stop with supervisor-owned workspace sweep. Separate scheduled tasks remain on their scheduler-owned lane. | Native cancellation, revocation race, Windows Job/cgroup independent proof required |
| Model-proposed filesystem effects | Whole-file UTF-8 create/edit enters existing durable write/edit review queues with native picker, byte preview and separate approval. No path, executable, or direct write authority is transferred from GGUF. | Installed OS picker and protected effects tests required |
| Original ANSF provenance | Browser/Reader/OCR/Observe receipts are untrusted historical observations, not reusable grants; private Observe/OCR excluded from model continuation and incomplete windows trigger review. Original correction and witness journals survive separately. | Original SQLite restart, tamper and privacy tests required |
| Project/task workflow | PLAN-01–07 sources establish project discovery, typed evidence, source-bound proposals, durable versioned review and approval/refusal without automatic task-complete or executor grants. Human review does not promote a verified-at-observation receipt to current-source verification. | Original installed UIAutomation and AT-SPI review/restart matrix required |
| Local model and source protection | Pinned llama.cpp GGUF/MMProj is the production runtime. Approved model transfer uses verified SHA/size publication and interrupted/review-required state; scripts for GPU acceptance are evidence-gathering, not GPU proof. | Physical Radeon/Qwen3.8 and model transfer/provisioning remain independent |
| Installer and CI | Source-pinned NSIS and Debian workflows, original ANSF witnesses, compiler/native matrices and separately admitted review tests are included. Browser is still `status:not-packaged`, `files:[]`, `platform:none`. | Hosted install/uninstall and reviewed browser bundle not accepted |

The 20 bridge paths changed downstream were examined in groups:
- **Authorization/recovery:** `ansf_chat_store.rs` adds a cancellation watermark
  before assistant publication; `p0_auth_custody.rs` tightens immutable
  cancellation event checks; `p0_auth_desktop.rs` changes the unscheduled
  worker stop to the native exact-workspace supervisor while retaining the
  distinct scheduled-worker lane; `reviewed_app_desktop.rs` checks scoped
  cancellation before publication; `native_background_process.rs` gains
  native chat-scope sweep and fail-closed custody uncertainty.
- **Inference and normal Send:** `generation_queue.rs` gains bounded
  stream/reconnect and priority state; `host_clock.rs` denies mixed clock/tool
  requests; `workspace_desktop.rs` adds native-picked CPU bounded search;
  `lib.rs` registers the new original host source/stream/performance modules
  and commands. Source comparison found additions to module/command
  registration, not removal of the bridge's original command set.
- **Review/UI:** `project_review_decisions_desktop.rs` retains original
  digest identity through an ownership fix; `main.jsx`, native intent and
  UI contract tests change presentation and request routing.
  `native_action_proposal.rs` and `browser_model_desktop.rs` retain the
  reviewed model/Browser boundaries in changed descendants.
- **Build/fixtures:** source-only REPO-01 grouped-gate changes,
  `src-tauri/lu-core/src/lib.rs`, the reviewed-tools test, live
  `source-tree.lock.json`, and remaining UI tests were amended after
  the bridge without deleting the original source identity.

**No automatic semantic equivalence is claimed for the 20 changed blobs.**
They have source-level disposition and are protected by current exact-head
tests; original OS acceptance is necessary before promotion.

## Exact-head acceptance blocker found during concurrent CORE-01E review

On independent CORE-01E exact head `97dd1e89...`, run
[#43342512185](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43342512185)
passed source, Windows/Linux compiler, Windows/Linux P0-WIN-CHAT native, and
hosted small-Qwen/SmolVLM model selection. Both **full Core native** jobs
advanced beyond Core Rust tests but **failed during desktop Rust tests**
because Tauri's `generate_context!()` could not find `../dist`
(the frontend build was ordered *after* the desktop Cargo test in the
existing workflow). Windows original job `131621801138`, Linux original
job `131621801163` both record `frontendDist ... path doesn't exist`.
This is an original CI fixture/build-order defect, **not an accepted
installed-app result**. It belongs to the separate CORE-01E repair/acceptance
stream; no source or workflow changes to the protected CORE-01E branch
are made by CORE-01F.

After any repaired exact-head run, independent Linux delegated-cgroup,
Windows Job/SCM restart and process custody receipts, hosted
NSIS/UIA + Debian/AT-SPI install/restart/uninstall, owner physical
Radeon/Qwen3.8 and actual reviewed Browser package remain explicitly
open until their own original oracles pass.

## CORE-01F disposition

**01F-02 source bridge review: COMPLETE AS A SOURCE AUDIT.**
The merge must still wait for:
1. CORE-01E one identical accepted source head, including full native,
   independent OS custody and installed-platform evidence.
2. CORE-01F-03 fresh main-head reconciliation. The snapshot main
   `16ead199037215fdaf1b049b1d06edadac09b81c` contains four
   ledger-only commits outside the R00/Core source history: preserve the
   canonical `docs/LU_DEVELOPMENT_LEDGER.md` and audit any new main delta.
3. Owner authorization for controlled bottom-up integration of R00,
   reviewed P0 bridge, #223, #224, #226, then #228. Do not infer approval
   from green source CI, mergeable=true, or a GitHub UI status.
4. Separate post-merge physical release readiness and browser provenance.

The bridge was reviewed *in place as ancestry*, not replaced, rebased,
reapplied or merged. CORE-01E remains unchanged. This CORE-01F audit is a
draft source-only artifact.

## Reproducible source checks

```sh
node --test scripts/core01f-bridge-review.test.mjs
node scripts/core01f-bridge-review.mjs
node --test scripts/core01f-merge-preflight.test.mjs
```

Requires the immutable R00, bridge and CORE-01E Git objects in the local
read-only source checkout. The verifier rejects absent or changed blobs,
missing paths, false acceptance flags, excluded provider/IDE paths,
deleted source, non-ancestral refs, and incomplete Git history. It reads
only; it cannot merge, replay worker authority, or modify user files.

## Final source-trust and UI command audit

The CORE-01F source-only regression now also checks that reviewed
worker launch and conversation revocation are wired, scheduled and
unscheduled chat-scoped cancellation remain distinct, original
cancel/audit evidence cannot claim automatic retries or independent
OS-custody success, original ANSF sources and model task proposals
cannot grant execution, interrupted HF transfers require new review,
and the Browser runtime manifest remains honestly `not-packaged`.
These are static source contracts, **not** native runtime substitutes.

The original P0 bridge desktop `main.jsx` exposed 176 distinct
`invoke("lu_*")` command names. A source comparison against CORE-01E
found all 176 retained and precisely two new command names:
`lu_stream_reconnect` and `lu_perf01_recent`. The original
`src-tauri/lu-core/src/lib.rs` module list retained all 223 and
added `prefix_kv_cache` and `warm_model_residency`. No removed
commands or modules were found in these source comparisons.

These checks close the frozen bridge's source-level trust review,
but no running native Job, installed OS dialog, or model inference
is established by a string or blob test.
