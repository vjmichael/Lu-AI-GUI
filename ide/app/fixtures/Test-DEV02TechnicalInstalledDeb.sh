#!/usr/bin/env bash
set -euo pipefail
[[ "${GITHUB_ACTIONS:-}" == true && "${RUNNER_ENVIRONMENT:-}" == github-hosted && -n "${RUNNER_TEMP:-}" ]]
[[ $# == 3 && "$2" =~ ^[0-9a-f]{40}$ && "$(git rev-parse HEAD)" == "$2" ]]
deb=$(realpath -- "$1")
[[ -f "$1" && ! -L "$1" && "$deb" == "$PWD/ide/app/src-tauri/target/release/bundle/deb/"* ]]
pkg=$(dpkg-deb --field "$deb" Package)
[[ "$pkg" == lu-ide-desktop || "$pkg" == lu-ide ]]
[[ "$(dpkg-query -W -f='${Status}' "$pkg" 2>/dev/null || true)" != 'install ok installed' ]]
[[ ! -e /usr/bin/lu-ide-desktop ]]
# Cleanup even when dpkg failed after installing some files. Never remove a
# pre-existing package; the hosted runner and absent preflight are mandatory.
installed=0
cleanup() {
  if [[ $installed == 1 ]]; then
    sudo dpkg --purge "$pkg" || return 2
    [[ "$(dpkg-query -W -f='${Status}' "$pkg" 2>/dev/null || true)" != 'install ok installed' && ! -e /usr/bin/lu-ide-desktop ]] || return 2
  fi
}
trap cleanup EXIT
installed=1
pkg_stage=$(mktemp -d "$RUNNER_TEMP/dev02-package-readback.XXXXXX")
dpkg-deb --extract "$deb" "$pkg_stage"
sudo dpkg --install "$deb"
[[ -x /usr/bin/lu-ide-desktop && ! -L /usr/bin/lu-ide-desktop ]]
cmp -- "$pkg_stage/usr/bin/lu-ide-desktop" /usr/bin/lu-ide-desktop
mapfile -t installed_assets < <(dpkg-query -L "$pkg" | grep -E '/technical-lsp(/|$)|/THIRD_PARTY_LICENSES(/|$)')
[[ ${#installed_assets[@]} -gt 10 ]]
python3 ide/packaging/verify_technical_lsp_install.py --reference "$3" --archives "$RUNNER_TEMP/dev02-technical-archives" --original-notices "$RUNNER_TEMP/dev02-technical-notices" --platform linux-x86_64 --source-sha "$2" --deb-package "$pkg"
fixture="$RUNNER_TEMP/lu-ide-dev02-technical-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
mkdir -m 700 "$fixture"
printf 'fn main() {}\n' > "$fixture/main.rs"
env WO8_INSTALLED_EXE=/usr/bin/lu-ide-desktop WO8_FIXTURE_ROOT="$fixture" \
  dbus-run-session -- xvfb-run -a -s '-screen 0 1280x800x24' bash ide/app/fixtures/Run-DEV02TechnicalGtkSession.sh
cleanup
installed=0
# Verify all package-owned provider assets are gone, not only the desktop.
[[ ! -e /usr/bin/lu-ide-desktop ]]
for asset in "${installed_assets[@]}"; do [[ ! -e "$asset" && ! -L "$asset" ]]; done
echo 'DEV02_TECHNICAL_DEBIAN_INSTALLED_UI_RESOURCES_UNINSTALL=PASS'
