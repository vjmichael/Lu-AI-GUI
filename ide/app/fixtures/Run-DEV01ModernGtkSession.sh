#!/usr/bin/env bash
# Real Linux native GTK installed acceptance session. No JS, IPC, fake dialog
# or user project injection: the caller supplies one fixed installed test driver.
set -euo pipefail
deny() { printf 'DEV01_GTK_SESSION_DENIED:%s\n' "$1" >&2; exit 1; }
[[ "${GITHUB_ACTIONS:-}" == true && -n "${RUNNER_TEMP:-}" ]] || deny native_runner_required
[[ -n "${DISPLAY:-}" && -n "${WO8_FIXTURE_ROOT:-}" && -x "${WO8_INSTALLED_EXE:-}" ]] \
  || deny installed_x11_session_required
command -v openbox >/dev/null || deny native_window_manager_absent
command -v xprop >/dev/null || deny native_ewmh_probe_absent
openbox >"${WO8_FIXTURE_ROOT}/window-manager-output.txt" 2>&1 &
wm_pid=$!
cleanup() {
  kill "$wm_pid" 2>/dev/null || true
  wait "$wm_pid" 2>/dev/null || true
}
trap cleanup EXIT
# Xvfb does not provide an EWMH _NET_ACTIVE_WINDOW manager by itself.
# Do not send dialog keystrokes until a real X11 WM publishes its identity.
wm_ready=0
for _ in $(seq 1 40); do
  kill -0 "$wm_pid" 2>/dev/null || deny native_window_manager_exited
  observation="$(xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null || true)"
  if [[ "$observation" =~ ^_NET_SUPPORTING_WM_CHECK\(WINDOW\):[[:space:]]window[[:space:]]id[[:space:]]#[[:space:]]0x[0-9a-fA-F]+ ]]; then
    wm_ready=1
    break
  fi
  sleep 0.25
done
[[ "$wm_ready" == 1 ]] || deny native_window_manager_not_registered
printf 'DEV01_GTK_X11_EWMH_WINDOW_MANAGER=VERIFIED_NOT_PICKER_ACCEPTANCE\n'
# The installed app is opened by the unchanged real AT-SPI driver, not by
# this wrapper. Missing chooser, foreign focus or absent catalog still fail.
driver="${1:-dev01_modern_linux_installed_atspi.py}"
case "$driver" in
  dev01_modern_linux_installed_atspi.py|b2_linux_installed_draft_atspi.py|b4_linux_installed_split_atspi.py|b5_linux_installed_perf_atspi.py) ;;
  *) deny unreviewed_installed_driver ;;
esac
[[ "$#" -le 1 ]] || deny unexpected_driver_arguments
command -v gsettings >/dev/null || deny native_gtk_setting_unavailable
gsettings set org.gtk.Settings.FileChooser startup-mode cwd || deny native_gtk_setting_failed
[[ "$(gsettings get org.gtk.Settings.FileChooser startup-mode)" == *cwd* ]] || deny native_gtk_setting_unverified
env GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  /usr/bin/python3 "ide/app/fixtures/$driver"
