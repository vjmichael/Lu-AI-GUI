#!/usr/bin/env bash
# Executed in a real Xvfb/DBus session by the installed-package witness.
set -euo pipefail
[[ -x "${WO8_INSTALLED_EXE:-}" && -n "${WO8_FIXTURE_ROOT:-}" ]] || exit 10
command -v openbox >/dev/null || { echo 'WO8_NATIVE_LINUX_DENIED:window_manager_absent' >&2; exit 17; }
source "$(dirname -- "${BASH_SOURCE[0]}")/x11_openbox_readiness.sh"
log_dir=$(native_fixture_log_dir "$WO8_FIXTURE_ROOT" "${RUNNER_TEMP:-${TMPDIR:-/tmp}}") || { echo 'WO8_NATIVE_LINUX_DENIED:log_directory_outside_fixture_required' >&2; exit 19; }
openbox >"$log_dir/window-manager-output.txt" 2>&1 &
wm_pid=$!
desktop_pid=''
cleanup() {
  if [[ -n "$desktop_pid" ]]; then
    kill "$desktop_pid" 2>/dev/null || true
    wait "$desktop_pid" 2>/dev/null || true
  fi
  kill "$wm_pid" 2>/dev/null || true
  wait "$wm_pid" 2>/dev/null || true
}
trap cleanup EXIT
wait_for_openbox_ewmh "$wm_pid" || { echo 'WO8_NATIVE_LINUX_DENIED:window_manager_not_ready' >&2; exit 18; }
env WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  GDK_BACKEND=x11 "${WO8_INSTALLED_EXE}" >"$log_dir/app-output.txt" 2>&1 &
desktop_pid=$!
window=""
for attempt in $(seq 1 45); do
  if ! kill -0 "$desktop_pid" 2>/dev/null; then
    echo 'WO8_NATIVE_LINUX_DENIED:installed_desktop_process_terminated' >&2
    exit 11
  fi
  window="$(xdotool search --onlyvisible --name 'Lu IDE' 2>/dev/null | head -n 1 || true)"
  [[ -n "$window" ]] && break
  sleep 1
done
[[ -n "$window" ]] || { echo 'WO8_NATIVE_LINUX_DENIED:no_real_window' >&2; exit 12; }
# Xvfb alone is not a window manager; use a real EWMH-capable WM in CI.
# A mapped window must be activated and keyboard focus independently verified.
wait_for_openbox_ewmh "$wm_pid" 8 || { echo 'WO8_NATIVE_LINUX_DENIED:window_manager_readiness_lost' >&2; exit 18; }
timeout 10s xdotool windowactivate --sync "$window" || { echo 'WO8_NATIVE_LINUX_DENIED:window_not_focusable' >&2; exit 13; }
timeout 10s xdotool windowfocus --sync "$window" || { echo 'WO8_NATIVE_LINUX_DENIED:window_keyboard_focus_denied' >&2; exit 16; }
name="$(xdotool getwindowname "$window")"
[[ "$name" == *"Lu IDE"* ]] || { echo 'WO8_NATIVE_LINUX_DENIED:unexpected_window_title' >&2; exit 14; }
# A native window is a smoke, not proof of editor/selection protocol behavior.
for cmd in lldb-dap clangd rust-analyzer; do
  if pgrep -x "$cmd" >/dev/null 2>&1; then
    echo "WO8_NATIVE_LINUX_DENIED:unexpected_autostart_$cmd" >&2
    exit 15
  fi
done
echo "WO8_INSTALLED_LINUX_X11_WINDOW_FOCUSED=PASS"
