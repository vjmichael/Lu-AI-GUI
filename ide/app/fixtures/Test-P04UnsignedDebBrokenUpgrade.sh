#!/usr/bin/env bash
# P04 real dpkg post-install failure and prior-version recovery.
# UNSIGNED disposable CI ONLY, never a production release.
set -euo pipefail
deny() { echo "P04_DPKG_ROLLBACK_DENIED:$1" >&2; exit 2; }
[[ "$#" == 1 && "${GITHUB_ACTIONS:-}" == true && -n "${RUNNER_TEMP:-}" ]] ||
  deny "disposable_runner_required"
package="$(realpath -- "$1")"
[[ -f "$package" && ! -L "$package" && "$package" == "$GITHUB_WORKSPACE/"* ]] ||
  deny "only_exact_native_ci_built_package"
pkg="$(dpkg-deb --field "$package" Package)"
[[ "$pkg" == lu-ide || "$pkg" == lu-ide-desktop ]] || deny "unexpected_debian_package"
new_version="$(dpkg-deb --field "$package" Version)"
[[ "$new_version" =~ ^[a-zA-Z0-9.+:~_-]+$ ]] || deny "invalid_debian_version"
old_version="$new_version~p04previous"
dpkg --compare-versions "$old_version" lt "$new_version" || deny "previous_version_not_ordered"
if dpkg-query -W -f='${Status}' "$pkg" 2>/dev/null | grep -q 'install ok installed'; then
  deny "preexisting_package"
fi
root="$(mktemp -d -p "$RUNNER_TEMP" lu-ide-p04-os-upgrade-XXXXXX)"
oldtree="$root/old"
broken="$root/broken"
oldpkg="$root/old.deb"
brokenpkg="$root/broken.deb"
state="$HOME/.local/share/org.lu.ide/projects.json"
[[ ! -e "$state" && ! -L "$state" ]] || deny "preexisting_user_catalog"
installed=0
state_created=0
cleanup() {
  code=$?
  trap - EXIT
  set +e
  if [[ "$installed" == 1 ]]; then sudo dpkg --remove "$pkg" >/dev/null 2>&1 || code=2; fi
  if [[ "$state_created" == 1 ]]; then
    rm -f -- "$state"
    rmdir -- "$(dirname "$state")" >/dev/null 2>&1 || true
  fi
  rm -rf -- "$root"
  exit "$code"
}
trap cleanup EXIT
dpkg-deb --raw-extract "$package" "$oldtree"
dpkg-deb --raw-extract "$package" "$broken"
[[ -f "$oldtree/DEBIAN/control" && -f "$broken/DEBIAN/control" ]] ||
  deny "actual_debian_control_not_found"
sed -i "s/^Version: .*/Version: $old_version/" "$oldtree/DEBIAN/control"
[[ "$(grep -c "^Version: $old_version$" "$oldtree/DEBIAN/control")" == 1 ]] ||
  deny "old_package_version_not_rewritten"
cat > "$broken/DEBIAN/postinst" <<'POSTINST'
#!/bin/sh
# Intentionally fails at actual dpkg configuration AFTER payload unpack.
# This altered package is not submitted for release/signing.
exit 77
POSTINST
chmod 755 "$broken/DEBIAN/postinst"
dpkg-deb --build "$oldtree" "$oldpkg" >/dev/null
dpkg-deb --build "$broken" "$brokenpkg" >/dev/null
[[ "$(dpkg-deb --field "$oldpkg" Version)" == "$old_version" ]] || deny "prior_version_invalid"
[[ "$(dpkg-deb --field "$brokenpkg" Version)" == "$new_version" ]] || deny "new_version_invalid"
sudo dpkg --install "$oldpkg" >/dev/null || deny "original_native_install_failed"
installed=1
exe=/usr/bin/lu-ide-desktop
[[ -f "$exe" && ! -L "$exe" ]] || deny "actual_installed_binary_missing"
app_hash="$(sha256sum "$exe" | cut -d' ' -f1)"
[[ "$(dpkg-query -W -f='${Version}' "$pkg")" == "$old_version" ]] ||
  deny "previous_version_not_installed"
mkdir -p "$(dirname "$state")"
printf '%s\n' '{"projects":[{"id":"p04-failed-upgrade-user-state"}]}' > "$state"
state_created=1
state_hash="$(sha256sum "$state" | cut -d' ' -f1)"
set +e
sudo dpkg --install "$brokenpkg" > "$root/failed-upgrade.log" 2>&1
failure=$?
set -e
[[ "$failure" -ne 0 ]] || deny "installer_injected_failure_not_real"
[[ "$(dpkg-query -W -f='${Status}' "$pkg" || true)" != 'install ok installed' ]] ||
  deny "dpkg_reported_success_after_real_failure"
echo 'P04_NATIVE_DPKG_FAILED_IN_POSTINST=PASS'
sudo dpkg --install "$oldpkg" >/dev/null || deny "previous_native_package_restore_failed"
[[ "$(dpkg-query -W -f='${Version}' "$pkg")" == "$old_version" ]] ||
  deny "previous_version_not_restored"
[[ "$(dpkg-query -W -f='${Status}' "$pkg")" == 'install ok installed' ]] ||
  deny "previous_version_not_configured"
[[ "$(sha256sum "$exe" | cut -d' ' -f1)" == "$app_hash" ]] ||
  deny "old_program_binary_not_restored"
[[ "$(sha256sum "$state" | cut -d' ' -f1)" == "$state_hash" ]] ||
  deny "user_catalog_mutated"
sudo dpkg --remove "$pkg" >/dev/null || deny "real_package_uninstall_failed"
installed=0
[[ ! -e "$exe" ]] || deny "package_binary_left_after_uninstall"
[[ "$(sha256sum "$state" | cut -d' ' -f1)" == "$state_hash" ]] ||
  deny "uninstall_deleted_user_state"
echo 'P04_NATIVE_REAL_DPKG_FAILED_UPGRADE_RECOVERY=PASS'
echo 'P04_USER_STATE_SURVIVED_REAL_DPKG_FAILURE=PASS'
echo 'P04_PRODUCTION_RELEASE_AND_SIGNING=NOT_ACCEPTED'
