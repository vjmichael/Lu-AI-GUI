#!/usr/bin/env bash
# IDE 6B-05: disposable CI Debian install/start/restart/remove + independent Git oracle.
# A headless Actions runner cannot supply real human OS approval.
set -euo pipefail
fail() { echo "G06B05_DEB_INSTALLED_DENIED:$1" >&2; exit 2; }
[[ "${GITHUB_ACTIONS:-}" == true && -n "${RUNNER_TEMP:-}" ]] || fail disposable_actions_runner_required
[[ $# -eq 2 ]] || fail deb_and_source_sha_required
deb="$(realpath -e -- "$1")"; source_sha="$2"
[[ "$source_sha" =~ ^[0-9a-f]{40}$ && "$(git rev-parse HEAD)" == "$source_sha" ]] || fail exact_source_mismatch
[[ "$deb" == *.deb && ! -L "$deb" ]] || fail native_deb_package_required
pkg="$(dpkg-deb --field "$deb" Package)"
[[ -n "$pkg" && "$(dpkg-deb --field "$deb" Architecture)" == amd64 ]] || fail package_identity
[[ "$(dpkg-query --show --showformat='${Status}' "$pkg" 2>/dev/null || :)" != "install ok installed" ]] || fail unexpected_preexisting_install
sha="$(sha256sum "$deb" | cut -d' ' -f1)"
fixture="$RUNNER_TEMP/g06b05-deb-fixture-$$"
manifest="$RUNNER_TEMP/g06b05-deb-baseline-$$.json"
python3 ide/app/fixtures/g06b05_git_oracle.py fixture --directory "$fixture" --mode create
python3 ide/app/fixtures/g06b05_git_oracle.py snapshot --repo "$fixture" --manifest "$manifest" --mode create --expect-index initial
installed=0
cleanup() {
  if [[ "$installed" -eq 1 ]]; then
    sudo dpkg --remove "$pkg" || echo 'G06B05_DEB_UNINSTALL_FAILED' >&2
  fi
}
trap cleanup EXIT
sudo dpkg --install "$deb" || fail dpkg_install
installed=1
exe=/usr/bin/lu-ide-desktop
[[ -x "$exe" && ! -L "$exe" ]] || fail installed_executable_absent
exe_sha="$(sha256sum "$exe" | cut -d' ' -f1)"
# Actual installed WebKitGTK window and two separate installed process launches.
# Never pass a reviewed project or manufacture a native authorization event.
export G06B05_EXE="$exe"
xvfb-run -a dbus-run-session -- bash -euo pipefail -c '
  export HOME="$(mktemp -d)"; export XDG_CONFIG_HOME="$HOME/.config"; export XDG_DATA_HOME="$HOME/.local/share"
  openbox >/dev/null 2>&1 & wm=$!
  trap '\''kill "$wm" 2>/dev/null || :; wait "$wm" 2>/dev/null || :'\'' EXIT
  for cycle in 1 2; do
    "$G06B05_EXE" >/dev/null 2>&1 & child=$!
    sleep 8
    kill -0 "$child" || { echo "G06B05_DEB_DENIED:installed_app_exited_cycle_$cycle" >&2; exit 2; }
    actual="$(readlink -e "/proc/$child/exe")"
    [[ "$actual" == "$G06B05_EXE" ]] || { echo "G06B05_DEB_DENIED:not_installed_process" >&2; exit 2; }
    xdotool search --name "Lu IDE" >/dev/null || { echo "G06B05_DEB_DENIED:no_native_window" >&2; exit 2; }
    kill "$child"; wait "$child" 2>/dev/null || :
  done
'
python3 ide/app/fixtures/g06b05_git_oracle.py check --manifest "$manifest" --outcome unchanged
[[ "$(sha256sum "$exe" | cut -d' ' -f1)" == "$exe_sha" ]] || fail installed_executable_changed
sudo dpkg --remove "$pkg" || fail deb_uninstall
installed=0
[[ ! -e "$exe" ]] || fail executable_remains_after_uninstall
python3 ide/app/fixtures/g06b05_git_oracle.py check --manifest "$manifest" --outcome unchanged
printf 'G06B05_DEB_SOURCE_SHA=%s\nG06B05_DEB_SHA256=%s\nG06B05_INSTALLED_EXE_SHA256=%s\nG06B05_REAL_INSTALLED_START_RESTART_REMOVE=PASS\nG06B05_PHYSICAL_OS_DIALOG=NOT_EXECUTED\n' "$source_sha" "$sha" "$exe_sha"
