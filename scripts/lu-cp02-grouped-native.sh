#!/usr/bin/env bash
# CP-02C: one consolidated native regression cohort. Real compile/test
# binaries on each OS; never treat filtered-out tests as success.
set -euo pipefail
export CARGO_TERM_COLOR=never
mkdir -p cp02-evidence
core=src-tauri/lu-core/Cargo.toml
desktop=lu-desktop/src-tauri/Cargo.toml
failures=0
run_tests() {
  local name="$1"; shift
  local log="cp02-evidence/$name.log"
  if cargo test --locked "$@" -- --nocapture 2>&1 | tee "$log" &&
     grep -Eq '^test result: ok\. [1-9][0-9]* passed; 0 failed;' "$log"; then
    printf 'PASS %s\n' "$name" | tee -a cp02-evidence/summary.txt
  else
    printf 'FAIL %s\n' "$name" | tee -a cp02-evidence/summary.txt
    failures=$((failures+1))
  fi
}
# Separate compile gates avoid repeatedly retrying an already failed build.
if cargo test --locked --manifest-path "$core" --lib --no-run 2>&1 | tee cp02-evidence/core-compile.log; then
  run_tests core-reader --manifest-path "$core" --lib native_reader
  run_tests core-linked-manifest --manifest-path "$core" --lib ansf_linked_attachments::tests
  run_tests core-legacy-image-manifest --manifest-path "$core" --lib ansf_image_attachments::tests
  run_tests core-live-chat --manifest-path "$core" --lib ansf_live_chat::tests
  run_tests core-multimodal-transport --manifest-path "$core" --lib multimodal_http::tests
  run_tests core-supervisor --manifest-path "$core" --lib inference_supervisor
  run_tests core-persistent-backend --manifest-path "$core" --lib persistent_llama_backend::tests
  run_tests core-passive-localization --manifest-path "$core" --lib passive_localization::tests
  for suite in diagnostic_contract diagnostic_localization passive_diagnostics; do
    run_tests "core-$suite" --manifest-path "$core" --test "$suite"
  done
else
  printf 'FAIL core-compile\n' | tee -a cp02-evidence/summary.txt
  failures=$((failures+1))
fi

if cargo test --locked --manifest-path "$desktop" --lib --no-run 2>&1 | tee cp02-evidence/desktop-compile.log; then
  for suite in linked_attachment_file::tests linked_attachment_intake::tests cp02_reader_attachment cp02_linked_original causal_diagnostic diagnostic_archive::tests perf01_telemetry::tests attachment_store::tests generation_queue::tests stream_delivery::tests workspace_desktop::tests image_submission_nonce_is_chat_bound_and_rejects_bad_shape; do
    run_tests "desktop-${suite//::/-}" --manifest-path "$desktop" --lib "$suite"
  done
else
  printf 'FAIL desktop-compile\n' | tee -a cp02-evidence/summary.txt
  failures=$((failures+1))
fi
printf '%s\n' 'REAL_GGUF_MMPROJ_SEMANTICS_AND_LIVE_OS_DIALOG_CLIPBOARD_WEBVIEW_NOT_TESTED' | tee cp02-evidence/native-scope.txt
test "$failures" -eq 0
