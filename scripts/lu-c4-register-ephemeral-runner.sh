#!/usr/bin/env bash
# One-use registration only. Run as a dedicated NONROOT user on a disposable VM
# after its systemd Delegate=yes + protected ancestor preflight passes.
# Never execute as root, on a personal workstation, or from GitHub Actions.
set -euo pipefail

REPOSITORY_URL='https://github.com/vjmichael/Lu-AI-GUI'
WORKFLOW_URL='https://github.com/vjmichael/Lu-AI-GUI/actions/workflows/lu-core-c4-delegated-kernel-original-sha.yml'

die() { printf 'LU_C4_RUNNER_REGISTRATION_BLOCKED: %s\n' "$*" >&2; exit 1; }
usage() {
  printf 'Generate label: %s --new-label\n' "$0"
  printf 'Register once:  %s --label lu-c4-<24hex>\n' "$0"
  exit 2
}

if [[ "${1:-}" == '--new-label' && $# -eq 1 ]]; then
  python3 -c 'import secrets; print("lu-c4-"+secrets.token_hex(12))'
  exit 0
fi
[[ $# -eq 2 && "$1" == '--label' ]] || usage
one_time_label="$2"
[[ "$one_time_label" =~ ^lu-c4-[0-9a-f]{24}$ ]] || die 'invalid one-time label'
[[ "$(id -u)" != 0 ]] || die 'runner must not be root'
[[ "${LU_C4_DISPOSABLE_VM_CONFIRM:-}" == 'YES' ]] ||
  die 'use an isolated, disposable VM; explicitly set LU_C4_DISPOSABLE_VM_CONFIRM=YES'
[[ -x ./config.sh && -x ./run.sh ]] ||
  die 'run from a GitHub-verified Linux x64 runner installation directory'
[[ ! -e .runner && ! -e .credentials ]] || die 'this directory is already registered'
[[ "$(stat -c %u .)" == "$(id -u)" ]] || die 'runner directory owned by another account'
# The runner stores short-lived registration credentials in this directory.
# Deny both group and world access; never rely on the last octal digit only.
python3 -c 'import os,sys;sys.exit(0 if (os.stat(".").st_mode & 0o077)==0 else 1)' ||
  die 'runner directory must have owner-only access (chmod 700)'
[[ "${BASH_SOURCE[0]}" != "$0" || -t 0 ]] || die 'interactive local terminal required'

printf '\nRepository: %s\n' "$REPOSITORY_URL"
printf 'Ephemeral labels: self-hosted, linux, x64, lu-cgroup-v2-delegated, %s\n' "$one_time_label"
printf 'First queue the *manual* kernel workflow with input one_time_label=%s:\n%s\n\n' \
  "$one_time_label" "$WORKFLOW_URL"
printf '%s\n' 'GitHub: Settings > Actions > Runners > New self-hosted runner > Linux > x64'
printf '%s\n' 'Use the one-hour registration token from the GitHub settings screen.'
printf '%s\n' 'Do NOT copy this token into the repository, issue, workflow, chat or shell history.'
read -r -s -p 'Paste one-time GitHub runner REGISTRATION token: ' registration_token
printf '\n'
[[ -n "$registration_token" && "${#registration_token}" -lt 512 ]] ||
  die 'missing or invalid-length registration token'
trap 'unset registration_token' EXIT

# GitHub automatically deregisters after exactly one job.
# The runner gets no repository token outside the job and no sudo grant.
runner_name="lu-c4-${one_time_label#lu-c4-}"
./config.sh \
  --unattended \
  --url "$REPOSITORY_URL" \
  --token "$registration_token" \
  --name "$runner_name" \
  --labels "lu-cgroup-v2-delegated,$one_time_label" \
  --work _work \
  --ephemeral
unset registration_token
trap - EXIT

printf '\nLU_C4_EPHEMERAL_RUNNER_REGISTERED_LOCALLY=1\n'
printf 'Name: %s\nOne-use label: %s\n' "$runner_name" "$one_time_label"
printf '%s\n' 'Registration alone is NOT kernel acceptance.'
printf '%s\n' 'The administrator must now start the dedicated systemd Delegate=yes service.'
printf '%s\n' 'Verify it is Online in GitHub Settings > Actions > Runners.'
printf '%s\n' 'After its one job, the ephemeral runner de-registers automatically.'
printf '%s\n' 'Destroy or reset the disposable VM and inspect the exact original job artifacts.'
