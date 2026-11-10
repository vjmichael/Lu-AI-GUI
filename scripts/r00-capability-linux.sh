#!/usr/bin/env bash
# R00: one exact-head, real-model K/I/A/F and verified reviewed-worker group.
# CI only. Never grants worker authority from a model's benchmark response.
set -euo pipefail
mkdir -p r00-capability-evidence
root="$GITHUB_WORKSPACE/r00-capability-evidence"
test "$(git rev-parse HEAD)" = "$EXPECTED_SHA"
node scripts/verify-source-tree.mjs
export LU_E1_P07_LLAMA_PORT=18972
export LU_E1_P07_GIT="$(command -v git)"
export LU_E1_P07_FFPROBE="$(command -v ffprobe)"
export LU_MEMORY_LOOP03C_GIT="$LU_E1_P07_GIT"
export LU_MEMORY_LOOP03C_FFPROBE="$LU_E1_P07_FFPROBE"
export LU_MEMORY_LOOP03C_NATIVE_FIXTURE_PATH="$root/memory-native.json"
export LU_MEMORY_LOOP03C_POSITIVE_FIXTURE_PATH="$root/memory-positive.json"
export LU_MEMORY_LOOP03C_REVIEWED_FIXTURE_PATH="$root/memory-reviewed.json"

# Genuine native ANSF/SQLite/continuity and Git/FFprobe worker evidence.
cargo test --locked --manifest-path lu-desktop/src-tauri/Cargo.toml --lib \
  memory_loop03c_actual_original_ansf_host_capsule_corpus -- --nocapture \
  2>&1 | tee "$root/native-ansf.log"
cargo test --locked --manifest-path lu-desktop/src-tauri/Cargo.toml --lib \
  memory_loop03c_positive_direct_git_ansf_journal_and_correction_corpus -- --nocapture \
  2>&1 | tee "$root/native-positive.log"
cargo test --locked --manifest-path lu-desktop/src-tauri/Cargo.toml --lib \
  memory_loop03c_reviewed_git_ffprobe_changed_source_and_exclusive_restart \
  -- --ignored --nocapture 2>&1 | tee "$root/native-reviewed.log"
for item in "$LU_MEMORY_LOOP03C_NATIVE_FIXTURE_PATH" "$LU_MEMORY_LOOP03C_POSITIVE_FIXTURE_PATH" "$LU_MEMORY_LOOP03C_REVIEWED_FIXTURE_PATH"; do
  test -s "$item"
done
python3 benchmarks/e1/memory_loop03c_real_qwen_comprehension.py \
  --fixture "$LU_MEMORY_LOOP03C_NATIVE_FIXTURE_PATH" \
  --positive-fixture "$LU_MEMORY_LOOP03C_POSITIVE_FIXTURE_PATH" \
  --reviewed-fixture "$LU_MEMORY_LOOP03C_REVIEWED_FIXTURE_PATH" \
  --output "$root/fixture-validation.json" --validate-fixture-only

# Pinned binary and GGUF: no artificial completions.
git clone --depth 1 --branch b12143 https://github.com/ggml-org/llama.cpp.git "$RUNNER_TEMP/r00-llama-src"
test "$(git -C "$RUNNER_TEMP/r00-llama-src" rev-parse HEAD)" = 2bc46baa9207d098d03741ac394e7faaf1a955b6
cmake -S "$RUNNER_TEMP/r00-llama-src" -B "$RUNNER_TEMP/r00-llama-build" \
  -DCMAKE_BUILD_TYPE=Release -DGGML_NATIVE=OFF -DGGML_VULKAN=OFF \
  -DLLAMA_CURL=OFF -DLLAMA_BUILD_SERVER=ON -DLLAMA_BUILD_TESTS=OFF
cmake --build "$RUNNER_TEMP/r00-llama-build" --target llama-server --parallel 3
engine="$RUNNER_TEMP/r00-llama-build/bin/llama-server"
model="$RUNNER_TEMP/r00-qwen.gguf"
curl -fL --retry 4 --retry-delay 5 --connect-timeout 30 \
  https://huggingface.co/QuantFactory/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B.Q4_K_M.gguf -o "$model"
printf '%s  %s\n' 7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d "$model" | sha256sum -c -
sha256sum "$engine" "$model" > "$root/engine-model-sha256.txt"

"$engine" -m "$model" --host 127.0.0.1 --port "$LU_E1_P07_LLAMA_PORT" -c 8192 -ngl 0 > "$root/live-gguf-server.log" 2>&1 &
server_pid="$!"
cleanup(){ kill "$server_pid" 2>/dev/null || true; wait "$server_pid" 2>/dev/null || true; }
trap cleanup EXIT
ready=0
for i in $(seq 1 180); do
  kill -0 "$server_pid" || { echo "Model server exited before ready" >&2; exit 1; }
  if curl -fsS --max-time 2 "http://127.0.0.1:$LU_E1_P07_LLAMA_PORT/health" > "$root/health.json"; then ready=1; break; fi
  sleep 2
done
test "$ready" -eq 1
# A small real model may fail comprehension. That must FAIL acceptance, but
# its mistake must not prevent independent K/I/A/F and three-arm measurement.
# Every model output is still genuine and journaled; nothing is repaired or
# fabricated into a tool call. Each model-dependent test gets its own receipt.
model_semantic_failures=0
if python3 benchmarks/e1/agent01g_qwen_plan_probe.py \
  --port "$LU_E1_P07_LLAMA_PORT" --output "$root/genuine-qwen-plan-probe.json" \
  2>&1 | tee "$root/plan-probe.log"; then
  echo 'MODEL_PLAN_PROBE_EXECUTED=1' >> "$GITHUB_STEP_SUMMARY"
else
  model_semantic_failures=$((model_semantic_failures+1))
  echo 'MODEL_PLAN_PROBE_FAILED=1' >> "$GITHUB_STEP_SUMMARY"
fi
for case in \
  agent01g_genuine_gguf_requires_fresh_ansf_review_native_git_oracle_and_restart \
  agent01i_genuine_qwen_two_fresh_approvals_independent_git_and_restart \
  genuine_gguf_independently_selects_git_and_media_tools_and_survives_journal_restart; do
  if cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml \
    --test e1_reviewed_model_tools "$case" -- --ignored --nocapture \
    2>&1 | tee "$root/$case.log"; then
    if ! grep -Fq "test $case ... ok" "$root/$case.log"; then
      model_semantic_failures=$((model_semantic_failures+1))
      echo "MODEL_NATIVE_TEST_MISSING=$case" >> "$GITHUB_STEP_SUMMARY"
    fi
  else
    model_semantic_failures=$((model_semantic_failures+1))
    echo "MODEL_NATIVE_TEST_FAILED=$case" >> "$GITHUB_STEP_SUMMARY"
  fi
done
cleanup
trap - EXIT

# Memory interface A/B keeps source/seed/model fixed. A model failure
# remains in raw output; benchmark completeness is not model competence.
for protocol in raw json_schema; do
  rc=0
  python3 benchmarks/e1/memory_loop03c_real_qwen_comprehension.py \
    --engine "$engine" --model "$model" \
    --fixture "$LU_MEMORY_LOOP03C_NATIVE_FIXTURE_PATH" \
    --positive-fixture "$LU_MEMORY_LOOP03C_POSITIVE_FIXTURE_PATH" \
    --reviewed-fixture "$LU_MEMORY_LOOP03C_REVIEWED_FIXTURE_PATH" \
    --response-protocol "$protocol" --repetitions 1 --port 18983 \
    --output "$root/memory-$protocol.json" \
    2>&1 | tee "$root/memory-$protocol.log" || rc=$?
  test "$rc" -eq 0 -o "$rc" -eq 2
done
python3 benchmarks/e1/r00_genuine_kiaf_comparison.py \
  --raw "$root/memory-raw.json" --schema "$root/memory-json_schema.json" \
  --output "$root/kiaf-interface-comparison.json" 2>&1 | tee "$root/kiaf-scores.log"
cat "$root/kiaf-scores.log" >> "$GITHUB_STEP_SUMMARY"

# Same exact task: a model without filesystem authority versus two real
# first-party utilities. Every output checked by independent OOXML oracle.
cargo build --locked --manifest-path src-tauri/lu-core/Cargo.toml \
  --example e1_p05_native_task --example e1_reviewed_artifact_worker
python3 benchmarks/e1/e1_p05_real_three_arm.py \
  --native "$GITHUB_WORKSPACE/src-tauri/lu-core/target/debug/examples/e1_p05_native_task" \
  --worker "$GITHUB_WORKSPACE/src-tauri/lu-core/target/debug/examples/e1_reviewed_artifact_worker" \
  --engine "$engine" --model "$model" --repeats 1 \
  --source-commit "$EXPECTED_SHA" --output-dir "$root/three-arm"
python3 benchmarks/e1/verify_e1_p05_raw.py \
  "$root/three-arm/e1_p05_raw.json" --source-commit "$EXPECTED_SHA" \
  --mode three-arm --repeats 1
cat "$root/three-arm/e1_p05_summary.md" >> "$GITHUB_STEP_SUMMARY"
echo 'This is real Qwen3-0.6B CPU, not target Qwen 27B or installed Windows Vulkan/rollback acceptance.' >> "$GITHUB_STEP_SUMMARY"

# A model-comprehension regression is never downgraded to success simply
# because all raw measurements and independent worker outcomes were captured.
printf 'MODEL_NATIVE_FAILURES=%d\n' "$model_semantic_failures" | tee "$root/model-semantic-summary.txt"
if test "$model_semantic_failures" -ne 0; then
  echo "Genuine Qwen plan/tool comprehension remains OPEN ($model_semantic_failures independent native failures)." >> "$GITHUB_STEP_SUMMARY"
  exit 48
fi
