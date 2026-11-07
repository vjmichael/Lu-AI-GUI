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
  fi
}
trap cleanup EXIT
sudo dpkg --install "$deb" || deny native_deb_install_failed
installed=1
exec_path=/usr/bin/lu-ide-desktop
[[ -x "$exec_path" ]] || deny installed_desktop_executable_missing
# The unsigned default install must not stage native debug/tool execution.
[[ ! -e /usr/lib/lu-ide-desktop/resources/debugger/bin/lldb-dap ]] || deny default_install_bundled_debugger
fixture="$RUNNER_TEMP/lu-ide-wo8-linux-${GITHUB_RUN_ID:-fixture}"
mkdir -m 700 -- "$fixture"
env WO8_INSTALLED_EXE="$exec_path" WO8_FIXTURE_ROOT="$fixture" \
  dbus-run-session -- xvfb-run -a -s '-screen 0 1280x800x24' \
  bash ide/app/fixtures/wo8_linux_window_acceptance.sh || deny real_graphical_launch_failed
echo "WO8_LINUX_INSTALLED_DEB_GRAPHICAL_LAUNCH=PASS"
echo "WO8_LINUX_INSTALLED_EDITOR_AND_PROTECTED_RUN=NOT_ACCEPTED"
