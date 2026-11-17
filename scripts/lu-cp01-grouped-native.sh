#!/usr/bin/env bash
# CP-01E/F: run the actual locked Rust test binaries, not source-only checks.
set -euo pipefail
export CARGO_TERM_COLOR=never
mkdir -p cp01-evidence
run_tests() {
  local name="$1"
  shift
  cargo test --locked "$@" -- --nocapture 2>&1 | tee "cp01-evidence/$name.log"
  # A typo or filtered-out suite must NOT produce a successful gate.
  grep -Eq '^test result: ok\. [1-9][0-9]* passed; 0 failed;' "cp01-evidence/$name.log"
}
core=src-tauri/lu-core/Cargo.toml
desktop=lu-desktop/src-tauri/Cargo.toml
for suite in diagnostic_contract diagnostic_localization passive_diagnostics; do
  run_tests "core-$suite" --manifest-path "$core" --test "$suite"
done
for suite in ansf_image_attachments::tests ansf_live_chat::tests multimodal_http::tests inference_supervisor persistent_llama_backend::tests passive_localization::tests; do
  run_tests "core-${suite//::/-}" --manifest-path "$core" --lib "$suite"
done
for suite in causal_diagnostic diagnostic_archive::tests perf01_telemetry::tests attachment_store::tests generation_queue::tests stream_delivery::tests workspace_desktop::tests image_submission_nonce_is_chat_bound_and_rejects_bad_shape; do
  run_tests "desktop-${suite//::/-}" --manifest-path "$desktop" --lib "$suite"
done
# Positive genuine MMProj test is intentionally NOT run with dummy resources.
# The installed physical gate requires separately reviewed real engine/model/projector
# inputs; see docs/LU_CP01_MULTIMODAL_DELIVERY_ACCEPTANCE.md.
printf '%s\n' 'HOSTED_NATIVE_TESTS_PASSED' 'REAL_MMPROJ_AND_INSTALLED_PHYSICAL_NOT_TESTED' | tee cp01-evidence/native-scope.txt
