#!/usr/bin/env bash
# P04 signed Debian two-version native acceptance: disposable CI only.
# No key, package or signer is generated here. Never an automatic release.
set -euo pipefail
deny() { echo "P04_SIGNED_DEB_DENIED:$1" >&2; exit 1; }
[[ "$#" -eq 9 ]] || deny nine_reviewed_inputs_required
old_deb="$(realpath -- "$1")"
old_sig="$(realpath -- "$2")"
new_deb="$(realpath -- "$3")"
new_sig="$(realpath -- "$4")"
approved_key="$(realpath -- "$5")"
old_sha="$6"
new_sha="$7"
signer_fingerprint="$8"
mode="$9"
[[ "$GITHUB_ACTIONS" == true && -n "$RUNNER_TEMP" && -n "$DISPLAY" ]] || deny isolated_graphical_os_runner_only
[[ "$mode" == upgrade || "$mode" == injected_recovery ]] || deny unexpected_transition_mode
[[ "$old_sha" =~ ^[a-fA-F0-9]{64}$ && "$new_sha" =~ ^[a-fA-F0-9]{64}$ ]] || deny missing_sha256
[[ "$old_sha" != "$new_sha" ]] || deny identical_version_artifacts
[[ "$signer_fingerprint" =~ ^[a-fA-F0-9]{40}$ ]] || deny unreviewed_signer
for file in "$old_deb" "$old_sig" "$new_deb" "$new_sig" "$approved_key"; do
  [[ -f "$file" && ! -L "$file" && "$file" == "$RUNNER_TEMP/"* ]] || deny artifact_outside_reviewed_ci_sandbox
done
[[ "$(sha256sum "$old_deb" | cut -d' ' -f1)" == "$old_sha" ]] || deny old_package_digest_drift
[[ "$(sha256sum "$new_deb" | cut -d' ' -f1)" == "$new_sha" ]] || deny new_package_digest_drift
gnupg="$(mktemp -d -p "$RUNNER_TEMP" lu-ide-p04-gpg-XXXXXX)"
chmod 700 "$gnupg"
export GNUPGHOME="$gnupg"
gpg --batch --quiet --import "$approved_key" || deny approved_signer_import_failed
observed="$(gpg --batch --with-colons --fingerprint | awk -F: '$1=="fpr"{print $10;exit}' | tr '[:lower:]' '[:upper:]')"
expected="$(printf '%s' "$signer_fingerprint" | tr '[:lower:]' '[:upper:]')"
[[ "$observed" == "$expected" ]] || deny approved_signer_fingerprint_drift
gpg --batch --quiet --verify "$old_sig" "$old_deb" || deny old_signature_invalid
gpg --batch --quiet --verify "$new_sig" "$new_deb" || deny new_signature_invalid
pkg="$(dpkg-deb --field "$old_deb" Package)"
[[ "$pkg" == lu-ide || "$pkg" == lu-ide-desktop ]] || deny unexpected_package
[[ "$(dpkg-deb --field "$new_deb" Package)" == "$pkg" ]] || deny changed_package_identity
[[ "$(dpkg-deb --field "$old_deb" Architecture)" == amd64 ]] || deny old_wrong_cpu
[[ "$(dpkg-deb --field "$new_deb" Architecture)" == amd64 ]] || deny new_wrong_cpu
old_version="$(dpkg-deb --field "$old_deb" Version)"
new_version="$(dpkg-deb --field "$new_deb" Version)"
dpkg --compare-versions "$old_version" lt "$new_version" || deny not_a_real_version_upgrade
if dpkg-query -W -f=${Status} "$pkg" 2>/dev/null | grep -q "install ok installed"; then
  deny existing_installed_ide
fi
fixture="$RUNNER_TEMP/lu-ide-p04-native-$GITHUB_RUN_ID"
[[ ! -e "$fixture" ]] || deny preexisting_fixture
mkdir -m 700 "$fixture"
printf 'fn main() { println!("signed"); }\n' > "$fixture/main.rs"
catalog="$HOME/.local/share/org.lu.ide/projects.json"
[[ ! -e "$catalog" ]] || deny preexisting_app_catalog
installed=0
cleanup() {
  code=$?
  trap - EXIT
  if [[ "$installed" -eq 1 ]]; then
    sudo dpkg --remove "$pkg" || code=1
  fi
  rm -rf -- "$gnupg"
  exit "$code"
}
trap cleanup EXIT
sudo dpkg --install "$old_deb" || deny old_signed_install_failed
installed=1
[[ -x /usr/bin/lu-ide-desktop ]] || deny old_installed_app_missing
env WO8_INSTALLED_EXE=/usr/bin/lu-ide-desktop WO8_FIXTURE_ROOT="$fixture" \
  /usr/bin/python3 ide/app/fixtures/wo8_linux_editor_atspi.py || deny native_old_gui_project_registration_failed
[[ -f "$catalog" && ! -L "$catalog" ]] || deny native_user_catalog_not_created
catalog_before="$(sha256sum "$catalog" | cut -d' ' -f1)"
source_before="$(sha256sum "$fixture/main.rs" | cut -d' ' -f1)"
sudo dpkg --install "$new_deb" || deny new_signed_install_failed
[[ "$(dpkg-query -W -f=${Version} "$pkg")" == "$new_version" ]] || deny new_real_version_not_installed
if [[ "$mode" == injected_recovery ]]; then
  # Recovery after a deliberate, bounded post-install fault. This does NOT
  # satisfy the real broken-install rollback gate.
  sudo dpkg --install "$old_deb" || deny previous_signed_package_not_restored
  [[ "$(dpkg-query -W -f=${Version} "$pkg")" == "$old_version" ]] || deny old_version_not_restored
  echo 'P04_INJECTED_SIGNED_DEBIAN_POSTINSTALL_RECOVERY=PASS'
  echo 'P04_REAL_FAILED_INSTALLER_ROLLBACK=NOT_PROVEN'
fi
env WO8_INSTALLED_EXE=/usr/bin/lu-ide-desktop WO8_FIXTURE_ROOT="$fixture" \
    WO8_P04_REOPEN_ONLY=true /usr/bin/python3 ide/app/fixtures/wo8_linux_editor_atspi.py \
  || deny installed_version_project_reopen_failed
[[ "$(sha256sum "$catalog" | cut -d' ' -f1)" == "$catalog_before" ]] || deny user_catalog_changed
[[ "$(sha256sum "$fixture/main.rs" | cut -d' ' -f1)" == "$source_before" ]] || deny original_project_file_changed
sudo dpkg --remove "$pkg" || deny signed_uninstall_failed
installed=0
[[ ! -x /usr/bin/lu-ide-desktop ]] || deny signed_app_left_after_uninstall
[[ "$(sha256sum "$catalog" | cut -d' ' -f1)" == "$catalog_before" ]] || deny uninstall_lost_catalog
echo 'P04_NATIVE_SIGNED_DEBIAN_TWO_VERSION_PROJECT_PERSISTENCE=PASS'
echo 'P04_LINUX_RELEASE_ACCEPTED=NO'
