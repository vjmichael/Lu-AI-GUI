#!/usr/bin/env bash
# Actual unsigned Debian/GTK/AT-SPI/X11 modern DEV-01 installed regression.
# This accepts NO signed release, independent operator approval, or OS custody.
set -euo pipefail
deny() { printf 'DEV01_MODERN_DEB_DENIED:%s\n' "$1" >&2; exit 1; }
[[ "${GITHUB_ACTIONS:-}" == true && -n "${RUNNER_TEMP:-}" ]] || deny runner_required
[[ $# -eq 2 && "$2" =~ ^[a-f0-9]{40}$ ]] || deny exact_source_sha_required
[[ "$(git rev-parse HEAD)" == "$2" ]] || deny changed_source_sha
deb="$(realpath -- "$1")"
[[ "$deb" == "$PWD/ide/app/src-tauri/target/release/bundle/deb/"* ]] || deny foreign_package
[[ -f "$deb" && ! -L "$deb" ]] || deny untrusted_deb
package="$(dpkg-deb --field "$deb" Package)"
[[ "$package" == lu-ide-desktop || "$package" == lu-ide ]] || deny package_identity
[[ "$(dpkg-query --show --showformat='${Status}' "$package" 2>/dev/null || true)" != "install ok installed" ]] || deny preexisting_install
installed=0
cleanup() {
  if [[ $installed -eq 1 ]]; then
    sudo dpkg --remove "$package" || return 1
    [[ "$(dpkg-query --show --showformat='${Status}' "$package" 2>/dev/null || true)" != "install ok installed" ]] || return 1
  fi
}
trap cleanup EXIT
sudo dpkg --install "$deb" || deny real_deb_install_failed
installed=1
[[ -x /usr/bin/lu-ide-desktop ]] || deny actual_installed_executable_missing
fixture="$RUNNER_TEMP/lu-ide-dev01-modern-${GITHUB_RUN_ID:-native}"
[[ ! -e "$fixture" ]] || deny reused_fixture
mkdir -m 700 "$fixture"
python3 - "$fixture" <<'PY'
from pathlib import Path
import sys
root=Path(sys.argv[1])
(root/"main.rs").write_text('fn main() { println!("modern dev01"); }\n')
(root/"second.rs").write_text('fn second() {}\n')
(root/"move-me.rs").write_text('fn move_me() {}\n')
for d in range(24):
    folder=root/f"src-{d:02d}"
    folder.mkdir(mode=0o700)
    for n in range(36):
        (folder/f"file-{n:03d}.rs").write_text(f"// source {d} {n}\n")
assert len(list(root.rglob("*.rs")))==867
PY
env WO8_INSTALLED_EXE=/usr/bin/lu-ide-desktop WO8_FIXTURE_ROOT="$fixture" \
  dbus-run-session -- xvfb-run -a -s '-screen 0 1280x800x24' \
  /usr/bin/python3 ide/app/fixtures/dev01_modern_linux_installed_atspi.py \
  || deny native_four_feature_installed_os_regression_failed
echo 'DEV01_MODERN_LINUX_DEBIAN_ACTUAL_INSTALLED_UI=PASS'
echo 'DEV01_MODERN_LINUX_PHYSICAL_RELEASE_OR_HUMAN_APPROVAL=NOT_TESTED'
