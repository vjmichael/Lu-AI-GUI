#!/usr/bin/env bash
# Group 6A installed Linux operator acceptance: no fabricated UI interaction.
# Observe the actual Debian-installed IDE and physically switch projects.
set -euo pipefail
exe="${1:-}"
evidence="${2:-}"
if [[ -z "$exe" || -z "$evidence" || -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" || -n "${GITHUB_ACTIONS:-}" ]]; then
  echo 'G06A_PHYSICAL_LINUX_DENIED:interactive_installed_session_required' >&2; exit 2
fi
exe="$(realpath -e -- "$exe")"
[[ "$(basename -- "$exe")" == lu-ide-desktop ]] || { echo 'G06A_PHYSICAL_LINUX_DENIED:wrong_binary' >&2; exit 2; }
[[ -f "$exe" && -x "$exe" && ! -L "$exe" ]] || { echo 'G06A_PHYSICAL_LINUX_DENIED:not_regular_executable' >&2; exit 2; }
sha="$(sha256sum -- "$exe" | cut -d' ' -f1)"
echo "Installed executable SHA256=$sha"
echo 'Open registered project A; verify branch inventory, refs/health, linked-worktree names and file blame.'
echo 'Switch to registered project B. Verify project A branch/history data disappears.'
read -r -p 'After actual operator verification, type G06A_SWITCH_VERIFIED: ' reply
[[ "$reply" == G06A_SWITCH_VERIFIED ]] || { echo 'G06A_PHYSICAL_LINUX_DENIED:switch_not_witnessed' >&2; exit 2; }
echo 'Close the installed IDE and start a new process using the SAME installed executable.'
echo 'Verify project persistence, new refresh and no stale data from project B in project A.'
read -r -p 'After actual installed restart verification, type G06A_RESTART_VERIFIED: ' reply
[[ "$reply" == G06A_RESTART_VERIFIED ]] || { echo 'G06A_PHYSICAL_LINUX_DENIED:restart_not_witnessed' >&2; exit 2; }
[[ "$(sha256sum -- "$exe" | cut -d' ' -f1)" == "$sha" ]] || { echo 'G06A_PHYSICAL_LINUX_DENIED:binary_changed' >&2; exit 2; }
mkdir -p -- "$evidence"
receipt="$evidence/g06a-physical-installed-linux.txt"
[[ ! -e "$receipt" ]] || { echo 'G06A_PHYSICAL_LINUX_DENIED:receipt_exists' >&2; exit 2; }
printf 'scope=linux_installed_operator_witness\nexe_sha256=%s\nproject_switch_observed=operator_acknowledged\nrestart_observed=operator_acknowledged\noriginal_git_objects_independently_hashed=NO\nrelease_approved=NO\n' "$sha" > "$receipt"
echo "G06A_PHYSICAL_LINUX_OPERATOR_OBSERVATION_RECORDED=$receipt"
