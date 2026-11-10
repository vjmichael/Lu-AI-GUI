#!/usr/bin/env bash
# WO8-D native Linux installed .deb graphical process acceptance.
# This is an installed-app smoke, NOT a signed or protected worker acceptance.
set -euo pipefail
deny() { echo "WO8_NATIVE_LINUX_DENIED:$1" >&2; exit 1; }
[[ "${GITHUB_ACTIONS:-}" == true && -n "${RUNNER_TEMP:-}" ]] || deny disposable_native_runner_only
[[ $# -eq 2 ]] || deny usage_deb_path_and_exact_source_sha
deb="$(realpath -- "$1")"
head_sha="$2"
[[ "$head_sha" =~ ^[a-f0-9]{40}$ ]] || deny exact_commit_sha_missing
actual_sha="$(git rev-parse HEAD)"
[[ "$actual_sha" == "$head_sha" ]] || deny exact_head_drift
[[ "$deb" == "$PWD/ide/app/src-tauri/target/release/bundle/deb/"* ]] || deny package_outside_build_output
[[ -f "$deb" && ! -L "$deb" ]] || deny missing_or_linked_package
[[ "$(dpkg-deb --field "$deb" Architecture)" == amd64 ]] || deny unexpected_package_arch
package="$(dpkg-deb --field "$deb" Package)"
[[ "$package" == lu-ide-desktop || "$package" == lu-ide ]] || deny unexpected_package_identity
[[ "$(dpkg-query --show --showformat='${Status}' "$package" 2>/dev/null || true)" != "install ok installed" ]] || deny preexisting_package
echo "WO8_LINUX_DEB_SHA256=$(sha256sum "$deb" | cut -d' ' -f1)"
installed=0
group02_catalog_prepared=0
cleanup() {
  if [[ "$installed" -eq 1 ]]; then
    if ! sudo dpkg --remove "$package"; then
      echo 'WO8_NATIVE_LINUX_DENIED:debian_uninstall_failed' >&2
      exit 1
    fi
    if dpkg-query --show --showformat='${Status}' "$package" 2>/dev/null | grep -q 'install ok installed'; then
      echo 'WO8_NATIVE_LINUX_DENIED:package_remains_installed' >&2
      exit 1
    fi
    if [[ "$group02_catalog_prepared" -eq 1 ]]; then
      # This is actual data left by the installed GTK application, not a
      # test-created stand-in. Check it only after real native dpkg removal.
      python3 ide/app/fixtures/group02_user_data_preservation.py after "$fixture" ||
        { echo 'IDE_G02_LINUX_DENIED:user_catalog_or_source_not_preserved' >&2; exit 1; }
      [[ ! -e /usr/bin/lu-ide-desktop ]] ||
        { echo 'IDE_G02_LINUX_DENIED:program_remains_after_uninstall' >&2; exit 1; }
    fi
  fi
}
trap cleanup EXIT
sudo dpkg --install "$deb" || deny native_deb_install_failed
installed=1
exec_path=/usr/bin/lu-ide-desktop
[[ -x "$exec_path" ]] || deny installed_desktop_executable_missing
# The unsigned default install must not stage native debug/tool execution.
[[ ! -e /usr/lib/lu-ide-desktop/resources/debugger/bin/lldb-dap ]] || deny default_install_bundled_debugger
# A dedicated one-child parent makes native GTK folder selection
# deterministic without supplying the project to Tauri or the backend.
# The human-equivalent native GTK file-list row must still be selected.
parent="$RUNNER_TEMP/lu-ide-wo8-gtk-parent-${GITHUB_RUN_ID:-fixture}"
fixture="$parent/lu-ide-wo8-linux-${GITHUB_RUN_ID:-fixture}"
[[ ! -e "$parent" && ! -L "$parent" ]] || deny preexisting_picker_parent
mkdir -m 700 -- "$parent"
mkdir -m 700 -- "$fixture"
env WO8_INSTALLED_EXE="$exec_path" WO8_FIXTURE_ROOT="$fixture" \
  dbus-run-session -- xvfb-run -a -s '-screen 0 1280x800x24' \
  bash ide/app/fixtures/wo8_linux_window_acceptance.sh || deny real_graphical_launch_failed
echo "WO8_LINUX_INSTALLED_DEB_GRAPHICAL_LAUNCH=PASS"
# Separate real native Linux AT-SPI keyboard/editor/OS picker session.
printf 'fn main() { println!("native"); }\n' >"${fixture}/main.rs"
# Real GTK dialog keyboard activation requires an actual EWMH X11 WM.
# The native Linux26 run proved that bare Xvfb lacks _NET_ACTIVE_WINDOW.
# All invoking installed workflows already stage the reviewed OS Openbox.
command -v openbox >/dev/null || deny native_openbox_unavailable
command -v xprop >/dev/null || deny native_xprop_unavailable
env WO8_INSTALLED_EXE="$exec_path" WO8_FIXTURE_ROOT="$fixture" \
  dbus-run-session -- xvfb-run -a -s '-screen 0 1280x800x24' \
  bash -c '
    set -euo pipefail
    openbox >/dev/null 2>&1 &
    wm_pid=$!
    trap "kill $wm_pid 2>/dev/null || true; wait $wm_pid 2>/dev/null || true" EXIT
    ready=0
    for n in {1..20}; do
      if xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q "window id #"; then
        ready=1
        break
      fi
      sleep 0.2
    done
    if [[ "$ready" -ne 1 ]]; then
      echo "WO8_NATIVE_LINUX_DENIED:ewmh_window_manager_not_verified" >&2
      exit 1
    fi
    echo "WO8_NATIVE_LINUX_X11_EWMH_WM=verified"
    # Documented GTK 3 preference: actual native chooser starts in the
    # application process CWD, which is ROOT.parent. Keep the OS picker,
    # keyboard and independent installed host/UI exact-project checks.
    command -v gsettings >/dev/null ||
      { echo "IDE_G02_LINUX_DENIED:gsettings_unavailable" >&2; exit 2; }
    gsettings set org.gtk.Settings.FileChooser startup-mode cwd ||
      { echo "IDE_G02_LINUX_DENIED:native_gtk_setting_failed" >&2; exit 2; }
    [[ "$(gsettings get org.gtk.Settings.FileChooser startup-mode)" == "'cwd'" ]] ||
      { echo "IDE_G02_LINUX_DENIED:native_gtk_setting_unverified" >&2; exit 2; }
    echo "IDE_G02_NATIVE_GTK_STARTUP_CWD=verified"
    /usr/bin/python3 ide/app/fixtures/wo8_linux_editor_atspi.py
  ' || deny linux_native_editor_acceptance_failed
echo "WO8_LINUX_INSTALLED_EDITOR_AND_CONFLICT=PASS"
# Keep this check INSIDE the actual installed-package lifetime: the EXIT trap
# removes the executable as soon as this fixture returns to its caller.
env WO8_INSTALLED_EXE="$exec_path" WO8_FIXTURE_ROOT="$fixture" \
  dbus-run-session -- xvfb-run -a -s '-screen 0 1280x800x24' \
  bash -c '
    set -euo pipefail
    openbox >/dev/null 2>&1 &
    wm_pid=$!
    trap "kill $wm_pid 2>/dev/null || true; wait $wm_pid 2>/dev/null || true" EXIT
    for n in {1..20}; do
      xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q "window id #" && break
      sleep 0.2
    done
    /usr/bin/python3 ide/app/fixtures/dev035_jobs_installed_atspi.py
  ' || deny dev035_real_installed_jobs_denied
echo "WO8_LINUX_PROTECTED_RUN_DEBUG=NOT_ACCEPTED"
# Record the *real* OS-installed app's catalog and edited source immediately
# before the EXIT trap uninstalls this exact Debian package.
python3 ide/app/fixtures/group02_user_data_preservation.py before "$fixture" ||
  deny group02_preuninstall_native_catalog_unverifiable
group02_catalog_prepared=1
