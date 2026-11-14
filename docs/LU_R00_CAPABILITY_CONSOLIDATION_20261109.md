# R00 capability consolidation — real model, ANSF, K/I/A/F, UX follow-up and hosted release lifecycle

**Scope:** Draft Lu Core PR #200. This is a single, manually admitted task group; it does not authorize merge, unattended model continuation, or physical GPU/release promotion.

## What was changed

1. **Real-model execution path:** The exact-head grouped Linux test exercises the existing genuine Qwen3-0.6B CPU GGUF model-to-ANSF plan, independent fresh user approval, reviewed Git/FFprobe worker, independent source oracle, distinct second approval, crash/restart and no-replay tests. Those production/native Rust paths existed before this group; this change consolidates their acceptance without inventing a new model executor.
2. **Memory and K/I/A/F:** Re-runs genuine original ANSF/SQLite, belief/CCM, native reviewed-worker fixture generation; then compares the *same* pinned Qwen3-0.6B model, original host evidence, seeds and cases with plain completion versus llama.cpp JSON-schema-constrained output. The schema constrains field presence/types **only**, not the correct host status, authority or freshness assertions. A separate auditor verifies model and raw-completion hashes, 1:1 paired cases, K/I/A/F check rates, model-format failures, unsafe permission claims and feedback overclaims. Model competency may fail while the diagnostic successfully measures it. The E1-P05 native-artifact three-arm pilot independently distinguishes a text-only model from real utility and reviewed-worker artifacts.
3. **UX-CHAT02:** The Versions/source view now provides **Prepare human follow-up from correction**, opening an editable, explicit new linked-human-reply composer pinned to the current corrected display version and SHA. It never mutates an original ANSF turn, silently contacts the model, grants an app permission or automatically reposts uncertain replies. The native existing prepared-turn, immutable parent, fresh human turn and crash-safe reconciliation remain the authoritative publication boundary.
4. **Hosted Windows lifecycle:** A source-pinned real llama.cpp CPU and Windows Lu EXE are packaged into an isolated unsigned NSIS test installer. Its disposable installation/uninstallation test now creates and independently hashes an isolated AppData sentinel, requiring that user data survive removal. **Two distinct-version rollback, real installed GGUF responses, OS picker interaction, physical AMD Vulkan, 7B+/Qwen27B, signing/legal and upgrade to v1.0 remain OPEN.** The builder must not publish a deceptive installer if compilation, actual installation or hash/data checks fail.

## Exact-head task-group CI

The pre-existing REPO-01 source job remains the only inexpensive automatic PR check. The heavy `native-r00-capability` Windows/Linux matrix runs only when the draft PR #200 description receives a **new** `LU_R00_CAPABILITY_GATE_SHA=<exact-head>` marker. No heavy jobs are triggered by ordinary synchronize/push, source-lock change or duplicate marker. Artifacts carry the exact commit SHA, native logs, genuine model raw reports, and hosted installer identity.

## Evidence grades and remaining decisions

- **Native test PASS** does not imply that the model understood a task; model competence is judged separately from measurement completeness.
- **CPU GGUF Qwen3-0.6B PASS** does not imply a 7B+ or Qwen3.8 27B pass, AMD Radeon Vulkan, multimodal/MMProj or a physical installed-app pass.
- **Hosted NSIS PASS** does not imply verified two-version upgrade/rollback or user-profile migration after forced termination. Preserve these as release gates.
- **Edited-message follow-up PASS** creates only a new authenticated human note with explicit provenance. A subsequent model turn or reviewed tool requires fresh user authorization.
- **AGENT-01L full-tree custody** remains limited to the already accepted positive tests; hostile ancestor-cgroup migration and whole-systemd-scope termination remain separate.

Do not mark these gates passed before actual exact-head compiler/runtime logs and artifacts have been inspected.