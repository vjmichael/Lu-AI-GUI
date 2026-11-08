#!/usr/bin/env bash
# Run an actual installed-app AT-SPI witness under a verified native X11 WM.
# Must be invoked INSIDE dbus-run-session + xvfb-run, not as a mock/browser test.
set -euo pipefail
deny() { echo "WO8_NATIVE_LINUX_DENIED:$1" >&2; exit 1; }
[[ $# -ge 1 && -n "${DISPLAY:-}" && -n "${DBUS_SESSION_BUS_ADDRESS:-}" ]] || deny missing_native_x11_or_dbus_session
command -v openbox >/dev/null || deny window_manager_absent
command -v xprop >/dev/null || deny native_x11_wm_probe_absent
command -v xdotool >/dev/null || deny native_x11_input_absent

# Each xvfb-run starts a *new X server*. The prior graphical smoke's WM has
# exited, so launch and witness a new WM before interacting with the GTK picker.
openbox --sm-disable >/dev/null 2>&1 &
wm_pid=$!
cleanup() {
  kill "$wm_pid" 2>/dev/null || true
  wait "$wm_pid" 2>/dev/null || true
}
trap cleanup EXIT

ready=0
for ((attempt=0; attempt<50; attempt++)); do
  kill -0 "$wm_pid" 2>/dev/null || deny native_window_manager_terminated
  if xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null |
      grep -Eq '_NET_SUPPORTING_WM_CHECK\(WINDOW\): window id # 0x[[:xdigit:]]+'; then
    ready=1
    break
  fi
  sleep 0.2
done
[[ "$ready" -eq 1 ]] || deny native_window_manager_not_ready
echo 'WO8_NATIVE_LINUX_EWMH_WINDOW_MANAGER=PASS'

# Preserve failure from the *real* OS automation. No IPC/JS/database shortcut.
"$@"
kill -0 "$wm_pid" 2>/dev/null || deny native_window_manager_died_during_editor
