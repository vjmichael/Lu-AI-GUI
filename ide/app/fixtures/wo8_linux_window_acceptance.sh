#!/usr/bin/env bash
# Executed in a real Xvfb/DBus session by the installed-package witness.
set -euo pipefail
[[ -x "${WO8_INSTALLED_EXE:-}" && -n "${WO8_FIXTURE_ROOT:-}" ]] || exit 10
env WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  GDK_BACKEND=x11 "${WO8_INSTALLED_EXE}" >"${WO8_FIXTURE_ROOT}/app-output.txt" 2>&1 &
desktop_pid=$!
cleanup() {
  kill "$desktop_pid" 2>/dev/null || true
  wait "$desktop_pid" 2>/dev/null || true
}
trap cleanup EXIT
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
xdotool windowactivate --sync "$window" || { echo 'WO8_NATIVE_LINUX_DENIED:window_not_focusable' >&2; exit 13; }
xdotool windowfocus --sync "$window" || { echo 'WO8_NATIVE_LINUX_DENIED:window_keyboard_focus_denied' >&2; exit 16; }
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
