# Lu Core — Phase 1: model intelligence and tool interface (2026-11-09)

Scope: **P0 implementation Phase 1 only**, draft PR #200. Do not silently advance to Phase 2 (multi-step orchestration), Phase 3 (memory), Phase 4 (desktop) or Phase 5 (release), and do not treat GitHub Actions as the product roadmap.

## Implemented capability boundaries

1. **Model output normalization without tool fabrication:** An actual local-model output may include ONE fully closed and bounded Qwen-style reasoning block followed by a SINGLE complete strict JSON object. Lu retains the unmodified generation in the model-audit journal, strips ONLY the reasoning wrapper for validation, and records its normalization class. Truncated/nested/role-injecting reasoning, mixed fences, duplicate JSON keys and multiple objects fail closed. Tool identity and all argument bytes still come from the model's JSON and are validated by Protobuf codecs, per-turn ConversationAuthority and native host policy. No tool is guessed from reasoning or inserted by a model-specific adapter.
2. **Live tool discovery:** Before composing the prompt and before each next generation, the current host-advertised allow-list is checked against the live installed native registry and exact Protobuf type URLs. Uninstalled/stale entries are not projected to the model and previously advertised tools cannot be used to grow the grant. Lu's existing bounded schema catalog remains the sole discovery surface; no new mini-app or MCP.
3. **Typed model feedback:** Every syntactically rejected PRE-dispatch tool/final envelope gets the same host-minted, size-bounded lu_boundary error. Only up to two pre-effect corrections are possible, never an effect replay. Sensitive rejected text, paths and arbitrary name claims are not quoted back to the model. Existing successful native results retain their independent tool-origin and native result envelope.
4. **Source truth after tool choice:** A reviewed Git/media observation session is explicitly host-marked as requiring a SUCCESSFUL native observation before a final answer. The model is free to select a catalog tool; it cannot claim a current SHA or completed check from a final-only guess. An unsupported final receives an OBSERVATION_REQUIRED correction, with no native action. Ordinary unreviewed chat remains allowed to answer without tools.
5. **Model-neutral engine formatting:** The pre-existing test-only llama.cpp /completion bridge offers both tool and final forms in a generic JSON grammar, with no host-filled tool names or output. Model choice and reasoning quality remain measured, not hardcoded.

## Existing constraints remain

- Model output never grants read/write/process authority or consent.
- No approval from ANSF history or retrieved text, no automatic retries of uncertain effects.
- Linux and Windows OS custody remain owned by the separate supervised-worker layer.
- A tiny Qwen3-0.6B may still choose 'final' or fail to select an appropriate native tool. That is a genuine MODEL capability outcome, not proof the tool adapter fabricated a call. A more capable user's GGUF may behave differently; no general tool-selection success may be claimed without actual model evidence.

## Tests (Phase 1 task group only)

The dedicated exact-head manual phase-one CI job verifies: strict bounded parser and rejection cases, live authorized tool discovery, reviewed final-without-tool refusal, synthetic host feedback, source-only engine grammar, and when actually available, pinned genuine Qwen 0.6B CPU model selection on reviewed Git and media cases. A failing Qwen test remains failing; do not weaken the application boundary to green CI.

**Phase 1 completion means:** the code paths and host safeguards are delivered and their integration is exercised. It never means every possible selected local model will choose tools correctly. Release/installer/physical Vulkan and the generalized multi-step runner are separate downstream phases.
