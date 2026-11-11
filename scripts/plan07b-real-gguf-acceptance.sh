#!/usr/bin/env bash
# Genuine PLAN-07B Qwen3: original native ANSF, real streamed abort and exact receipt.
# This is NOT Lu's production FIFO cancellation or the installed human UI.
set -euo pipefail
: "$RUNNER_TEMP"
: "$GITHUB_WORKSPACE"
: "$PLAN07B_REAL_HEAD"
: "$PLAN07B_REAL_MODEL_SHA"
: "$PLAN07B_REAL_PORT"
test "$(git rev-parse HEAD)" = "$PLAN07B_REAL_HEAD"
node scripts/verify-source-tree.mjs
mkdir -p plan07b-real-evidence
echo "SOURCE_SHA=$PLAN07B_REAL_HEAD" > plan07b-real-evidence/source.txt
python3 -m py_compile benchmarks/e1/plan07b_real_gguf_task_updates.py \
    scripts/plan07b-independent-original-ansf-oracle.py
export LU_PLAN07B_REAL_PROFILE="$RUNNER_TEMP/plan07b-original-ansf"
export LU_PLAN07B_REAL_FIXTURE="$GITHUB_WORKSPACE/plan07b-real-evidence/original-native-prompt.json"
export LU_PLAN07B_REAL_REPLY="$GITHUB_WORKSPACE/plan07b-real-evidence/real-model-reply.txt"
export LU_PLAN07B_REAL_CANCEL="$GITHUB_WORKSPACE/plan07b-real-evidence/real-stream-cancel.json"
export LU_PLAN07B_REAL_ORACLE="$GITHUB_WORKSPACE/plan07b-real-evidence/native-ansf-oracle.json"
export LU_PLAN07B_REAL_DISPOSABLE=YES
test ! -e "$LU_PLAN07B_REAL_PROFILE"
test ! -e "$LU_PLAN07B_REAL_FIXTURE"
pushd lu-desktop
npm ci --no-audit --no-fund
npm run build
npm run check
popd
cargo test --locked --manifest-path lu-desktop/src-tauri/Cargo.toml \
  --lib project_update_model_desktop::tests::export_real_qwen_original_ansf_prompt_fixture \
  -- --ignored --exact --nocapture 2>&1 | tee plan07b-real-evidence/original-ansf-fixture.log
grep -Fq 'PLAN07B_REAL_GGUF_ORIGINAL_HOST_FIXTURE_READY=1' plan07b-real-evidence/original-ansf-fixture.log
test -s "$LU_PLAN07B_REAL_FIXTURE"

# Existing Lu PLAN04 genuine GGUF acceptance pins the same source/model.
git clone --depth 1 --branch b12143 https://github.com/ggml-org/llama.cpp.git "$RUNNER_TEMP/plan07b-llama"
actual="$(git -C "$RUNNER_TEMP/plan07b-llama" rev-parse HEAD)"
test "$actual" = 2bc46baa9207d098d03741ac394e7faaf1a955b6
cmake -S "$RUNNER_TEMP/plan07b-llama" -B "$RUNNER_TEMP/plan07b-build" \
  -DCMAKE_BUILD_TYPE=Release -DGGML_NATIVE=OFF -DGGML_VULKAN=OFF \
  -DLLAMA_CURL=OFF -DLLAMA_BUILD_SERVER=ON -DLLAMA_BUILD_TESTS=OFF
cmake --build "$RUNNER_TEMP/plan07b-build" --target llama-server --parallel 3 \
  2>&1 | tee plan07b-real-evidence/actual-engine-build.log
server="$RUNNER_TEMP/plan07b-build/bin/llama-server"
sha256sum "$server" | tee plan07b-real-evidence/original-engine-sha256.txt
echo "LLAMA_CPP_SOURCE_SHA=$actual" >> plan07b-real-evidence/source.txt
model="$RUNNER_TEMP/plan07b-real-Qwen3-0.6B.gguf"
curl -fL --retry 4 --retry-delay 5 --connect-timeout 30 \
  https://huggingface.co/QuantFactory/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B.Q4_K_M.gguf \
  -o "$model"
printf '%s  %s\n' "$PLAN07B_REAL_MODEL_SHA" "$model" | sha256sum -c -
sha256sum "$model" | tee plan07b-real-evidence/original-model-sha256.txt
echo "MODEL_SHA256=$PLAN07B_REAL_MODEL_SHA" >> plan07b-real-evidence/source.txt

"$server" -m "$model" --host 127.0.0.1 \
  --port "$PLAN07B_REAL_PORT" -c 8192 -ngl 0 -np 1 \
  > plan07b-real-evidence/llama-server.log 2>&1 &
pid="$!"
trap 'kill "$pid" 2>/dev/null || true' EXIT
ready=0
for _ in $(seq 1 180); do
  kill -0 "$pid" 2>/dev/null || { echo 'Original engine exited before ready'; exit 1; }
  if curl -fsS --max-time 2 "http://127.0.0.1:$PLAN07B_REAL_PORT/health" \
      > plan07b-real-evidence/health.json; then ready=1; break; fi
  sleep 2
done
test "$ready" = 1
python3 benchmarks/e1/plan07b_real_gguf_task_updates.py \
  --fixture "$LU_PLAN07B_REAL_FIXTURE" \
  --reply "$LU_PLAN07B_REAL_REPLY" \
  --cancel "$LU_PLAN07B_REAL_CANCEL" \
  --response-receipt plan07b-real-evidence/original-inference.json \
  --engine "$server" --model "$model" --port "$PLAN07B_REAL_PORT" \
  2>&1 | tee plan07b-real-evidence/genuine-model-inference.log
test -s "$LU_PLAN07B_REAL_REPLY"
test -s "$LU_PLAN07B_REAL_CANCEL"
cargo test --locked --manifest-path lu-desktop/src-tauri/Cargo.toml \
  --lib project_update_model_desktop::tests::verify_real_qwen_reply_cancellation_and_original_ansf_no_task_acceptance \
  -- --ignored --exact --nocapture 2>&1 | tee plan07b-real-evidence/native-genuine-reply-audit.log
grep -Fq 'PLAN07B_REAL_GGUF_ORIGINAL_ANSF_CANCEL_AND_NO_COMPLETION_PASS=1' \
  plan07b-real-evidence/native-genuine-reply-audit.log
grep -Fq 'test result: ok. 1 passed; 0 failed;' \
  plan07b-real-evidence/native-genuine-reply-audit.log
python3 scripts/plan07b-independent-original-ansf-oracle.py \
  --profile "$LU_PLAN07B_REAL_PROFILE" \
  --fixture "$LU_PLAN07B_REAL_FIXTURE" \
  --reply "$LU_PLAN07B_REAL_REPLY" \
  --cancel "$LU_PLAN07B_REAL_CANCEL" \
  --inference plan07b-real-evidence/original-inference.json \
  --host-oracle "$LU_PLAN07B_REAL_ORACLE" \
  --output plan07b-real-evidence/independent-final-oracle.json \
  2>&1 | tee plan07b-real-evidence/independent-sqlite-witness.log
grep -Fq 'PLAN07B_INDEPENDENT_REAL_GGUF_NO_AUTO_ACCEPTANCE_PASS=1' \
  plan07b-real-evidence/independent-sqlite-witness.log
echo 'PLAN07B_REAL_PINNED_GGUF_ANSF_NO_AUTO_ACCEPTANCE_PASS=1' >> "$GITHUB_STEP_SUMMARY"
echo 'Original Qwen3 reply, immutable ANSF model proof and interrupted external stream passed. Lu production FIFO cancellation and physical installed UI remain separate.' >> "$GITHUB_STEP_SUMMARY"
