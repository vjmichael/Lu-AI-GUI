# E1-P05 — real three-arm local execution benchmark

**Source candidate:** branch \`lu/e1-p05-three-arm-benchmark-20261107\`, stacked on E1-P04B draft PR #107, which follows P04 #106, P03 #105, P02 #103 and earlier M5. This is a bounded comparison of same fixed tasks, not a claim that unreviewed third-party apps, arbitrary GUI commands, Windows installer packages or Vulkan inference are accepted.

## Task and arms

Two concrete, disposable Office document tasks, repeated in bounded randomized order:

- **DOCX:** create an actual OOXML Word document with the verified paragraph \`Lu E1 artifact custody verified\`.
- **XLSX:** create an actual OOXML Excel workbook \`E1\`, with A1=\`Status\`, B1=\`Reviewed\`, A2=\`Frames\`, and B2=\`48\`.

Three arms:

1. **\`direct_model\`**: the genuine locally run SHA-pinned \`Qwen3-0.6B.Q4_K_M.gguf\` model through the independently pinned llama.cpp CPU server. No filesystem/tool access. Record the raw model generation and prompt token use (when reported). Measure whether its textual description captured required specification terms **separately** from delivery of any real document. No text-only response is a created DOCX/XLSX, and the runner MUST NOT invent a file.
2. **\`native_utility\`**: the actual Rust Lu Core \`native_spreadsheet_create::render_xlsx\` under authenticated original-user-turn \`WriteWorkspace\` and independent host policy, with actual create-new \`cap_std\` publication, receipt journal and independent native Office parser. Existing Lu Core currently has a DOCX *reader* but no equivalent direct DOCX *creator*: score \`supported=false\` on DOCX instead of synthesizing an export.
3. **\`reviewed_app\`**: the real first-party \`e1_reviewed_artifact_worker\` executable through E1-P04B's host-reviewed SHA admission, current \`ExecuteCode\` plus separate \`WriteWorkspace\` grants, existing OS Job Object/process-group supervisor, one-shot binary stdout custody, independent real DOCX/XLSX reader, atomic file publish and two effect receipts. This is a **first-party reviewed worker**, not generic external-app automation. A real denied host-policy attempt before actual launch proves no journal or worker effects were performed by the denied request.

The native arms have one **actual artifact** per supported task, independently SHA-256-verified by the Python harness and checked as a valid ZIP containing the required OOXML part. The Rust native readers independently verify exact paragraph or sheet/cell text and the journal is checked for completed durable effects. The benchmark measures wall time and native execution time, creates a fresh disposable root per task, and records exact source SHA, native runner/worker SHA, model GGUF SHA and llama server binary SHA.

## Reproducible CI and local invocation

- \`src-tauri/lu-core/examples/e1_p05_native_task.rs\`: trusted CI-only, fixed-format real native task runner for \`native_utility\` and \`reviewed_app\`.
- \`benchmarks/e1/e1_p05_real_three_arm.py\`: orchestrates real subprocess execution, optional genuine local model and raw answer logging, pairs the formats/arms and computes p50/p95 latency and task success by format.
- \`benchmarks/e1/test_e1_p05_real_contract.py\`: tests fail-closed missing-data and scorer contracts, **not** native runtime acceptance.
- \`.github/workflows/lu-e1-p05-three-arm.yml\`: source-integrity/contract job; **real hosted Windows/Linux** native-utility and reviewed-worker measurements; separate Linux local Qwen3 0.6B GGUF and pinned llama.cpp three-arm measurements. The GGUF URL/hash and llama.cpp revision/hash are the existing pinned M5 benchmark choices; no download substitution is allowed.

When the native binaries exist, the orchestrator can run with:

\`\`\`bash
python3 benchmarks/e1/e1_p05_real_three_arm.py \
  --native '<actual absolute path to compiled e1_p05_native_task>' \
  --worker '<actual absolute path to compiled e1_reviewed_artifact_worker>' \
  --repeats 2 --output-dir '<new disposable evidence directory>'
\`\`\`

Without \`--engine\` AND \`--model\`, this creates a **NATIVE_ARMS_ONLY_MODEL_NOT_EXECUTED** record with \`direct_model\` explicitly \`NOT_EXECUTED\` and null latency/accuracy. For a real model comparison, pass both reviewed executable/file paths. The runner rejects missing binaries, an incorrect GGUF SHA-256, pre-existing output directories, unexpected task identities, unauthenticated CLI results, physical artifacts that disagree with reported SHA/size, and failed independent ZIP part inspection. Failed tasks abort rather than generating synthetic "passing" data.

## Meaningful measurements and limitations

- **Artifact task success:** actual new file present, hash/size verified, expected OOXML part present and Rust native reader semantic check succeeded. A text-only model may answer correctly but has no working file tool and therefore cannot score artifact creation as success.
- **Knowledge/specification accuracy:** separate direct-model text metric. It is NOT an artifact metric.
- **Latency:** real inference generation or host-native task implementation runtime; sample count and p50/p95 explicitly reported. Native harness setup time is separate, so setup/compilation/model load costs must not be hidden.
- **Authorization violations:** an attempted host-policy veto is actually run against native arms and the receipt/journal/file absence is checked. The "unauthorizedAttemptsBlocked" metric is grounded in those attempts only; it does not simulate adversarial escalation, network compromise or unconstrained third-party DLL injection.
- **K/I/A/F attribution:** K = model specification correctness, I = exact reviewed source/input & task expectations, A = genuine artifact and journal action, F = independent OOXML semantics/OS-pipe custody and receipt outcomes.
- **Not measured:** autonomous model tool routing, dynamic app selection, arbitrary document edits, semantic video/audio results, interactive WebView, user satisfaction, real AMD Vulkan offload, installed Windows/NSIS, publisher signing, or full arbitrary third-party process sandboxing. These remain open near the appropriate E1 or v1.0 release phase.

**Release decision:** the source may be submitted as a benchmark candidate, but E1-P05 can only be graded after actual final-head native and local GGUF CI receipts exist. No unobserved metrics, invented model generations or model superiority conclusions are permitted.

## E1-P05B — independent Python semantic evidence check

Candidate stacked on E1-P05: the Python harness independently parses the actual bounded DOCX/XLSX ZIP/XML bytes, follows the package/workbook relationships, checks the exact approved Word paragraph or all four typed Excel cells, and refuses duplicate/unsafe/oversized archive entries and orphaned parts. It does not simply credit a ZIP filename. Native Rust readers remain an independent first verifier. The harness also rejects a missing host-policy veto attempt or unsupported/journal status inconsistent with the task, instead of accepting two absent attempt counters as equal. Hostile structural fixtures are **contract tests**, never represented as real Windows/Linux worker runs. The genuine exact-head native and Qwen GGUF gates are still open pending Actions evidence; no installed-Windows claims.
