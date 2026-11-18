# AGENT-01G — genuine local Qwen GGUF diagnostic evidence

**Recorded:** November 8, 2026 (Phoenix). **Evidence class:** *Local CPU diagnostic only, not Lu host acceptance.*

These three JSON files are copied **byte-for-byte** from the previously run isolated local Qwen2.5 0.5B Q2_K test using an extracted Debian 13 Lu preview engine. The `manifest.json` pins each file's original byte count and SHA-256; `scripts/verify-agent01g-local-evidence.mjs` verifies them without requiring the model binaries.

| Receipt | Actual observation | What it does NOT establish |
|---|---|---|
| [Engine smoke](lu_engine_smoke_receipt.json) | Real CPU llama-server loaded user-owned GGUF and returned `4` for `2 + 2`. Server exit recorded. | No Lu authorization, task broker, independent journal or production plan. |
| [Diagnostic Git success](lu_native_gguf_git_diagnostic_receipt.json) | Actual model-generated function call `read_git_head`, actual read-only Git child and separate on-disk `.git/HEAD` reference comparison, matching SHA and child exit. | Outside the Lu host's source-bound `AGENT-01D/01C` authorization and receipt chain. The model's extra explanatory sentence was **not** independently verified. |
| [Rejected initial attempt](lu_native_gguf_git_diagnostic_first_attempt.json) | Model emitted tool-looking prose with fabricated parameters, so the harness refused tool execution. | Not successful tool selection. |

**Model:** `qwen2.5-0.5b-instruct-q2_k.gguf` (415,182,688 bytes; SHA-256 `9ee36184e616dfc76df4f5dd66f908dbde6979524ae36e6cefb67f532f798cb8`).

**Runtime package:** `Lu-Desktop_0.8.5~lu20261030.7_amd64.deb` (SHA-256 `a2bceab0d3da708674888c8fb77782f2bd26c09530c0cf626ae10ecd2e453408`), CPU `llama-server` built from pinned upstream `7fe450e19305b828c199d602c23a8337aaa1f03b`. Neither model nor runtime binary is uploaded here.

**More capable model:** The current GitHub `lu-e1-p07-genuine-gguf.yml` independently downloads Qwen3-0.6B Q4_K_M from Hugging Face and verifies its own SHA. That model is **not** stored in GitHub as a source blob or release asset. Its tool-selection reliability cannot be inferred from these Qwen2.5 receipts; use consistent benchmark prompts and the actual scheduled production Lu worker to compare.

**Required AGENT-01G acceptance:** exact-head real Lu desktop + loaded GGUF, original ANSF goal, immutable human-approved plan, fresh per-step native authorization, model-selected reviewed tool, real protected worker/cancellation and independent `AGENT-01C/01D` receipt/restart. See [AGENT-01G engineering record](../../../docs/LU_AGENT01G_REVIEWED_STEP_EXECUTION_20261108.md). **Open until observed.**

The original diagnostic JSON retains the isolated container path and loopback port only as provenance, not a portable runtime URL.
