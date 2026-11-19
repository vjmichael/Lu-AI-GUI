#!/usr/bin/env bash
set -euo pipefail
[[ "${GITHUB_ACTIONS:-}" == true && -n "${DISPLAY:-}" && -n "${RUNNER_TEMP:-}" && -n "${WO8_FIXTURE_ROOT:-}" ]]
source "$(dirname -- "${BASH_SOURCE[0]}")/x11_openbox_readiness.sh"
log_dir=$(native_fixture_log_dir "$WO8_FIXTURE_ROOT" "$RUNNER_TEMP")
openbox >"$log_dir/window-manager-output.txt" 2>&1 &
wm_pid=$!
trap 'kill "$wm_pid" 2>/dev/null || true; wait "$wm_pid" 2>/dev/null || true' EXIT
wait_for_openbox_ewmh "$wm_pid"
env GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  /usr/bin/python3 ide/app/fixtures/dev02_technical_installed_linux.py
