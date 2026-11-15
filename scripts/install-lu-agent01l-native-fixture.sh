#!/usr/bin/env bash
# ADMIN-ONLY / NATIVE LINUX: install fixed AGENT-01L SEC-06C test adapter.
# Run deliberately on the administrator-controlled self-hosted Linux runner;
# never auto-run on a GitHub-hosted runner or from a model tool request.
set -euo pipefail

if [[ "$#" != 2 ]]; then
  echo 'Usage: sudo bash scripts/install-lu-agent01l-native-fixture.sh <exact-source-SHA> <linux-runner-account>' >&2
  exit 2
fi
expected="$1"
user="$2"
if [[ ! "$expected" =~ ^[0-9a-f]{40}$ || ! "$user" =~ ^[a-z_][a-z0-9_-]*$ ]]; then
  echo 'Invalid exact source SHA or native runner account' >&2; exit 2
fi
if [[ "$(id -u)" != 0 || "$(uname -s)" != Linux ]]; then
  echo 'Requires authorized administrator on the original native Linux machine' >&2; exit 78
fi
runner_uid="$(id -u "$user")"
if [[ "$runner_uid" == 0 ]]; then
  echo 'The original native test worker and CI runner must be nonroot' >&2; exit 78
fi
test -f /sys/fs/cgroup/cgroup.controllers || {
  echo 'Actual Linux cgroup v2 kernel interface not present' >&2; exit 78;
}
test -e /run/systemd/system || {
  echo 'Root-owned native systemd manager not running' >&2; exit 78;
}
for tool in git visudo systemd-run sudo python3; do
  command -v "$tool" >/dev/null || { echo "Missing native $tool" >&2; exit 78; }
done
# Git access under sudo can need an explicit one-command safe.directory for
# a checkout owned by the runner. No checkout script executes as ROOT.
actual="$(git -c safe.directory="$(pwd)" rev-parse HEAD)"
if [[ "$actual" != "$expected" ]]; then
  echo 'Source HEAD drifted; do not install unreviewed helper binaries' >&2
  exit 78
fi
if [[ -L /usr/libexec || -L /usr/libexec/lu ]]; then
  echo 'Root-owned installed executable directory is a symlink' >&2; exit 78
fi
install -d -o root -g root -m 0755 /usr/libexec/lu /var/lib/lu-sec06c/receipts \
  /var/lib/lu-sec06c/fixture-receipts /run/lu-sec06c
if [[ "$(stat -c '%u' /usr/libexec/lu)" != 0 ]]; then
  echo 'Untrusted root-owned executable directory' >&2; exit 78
fi
for entry in \
  'lu-sec06c-owned-kernel-manager.py:lu-sec06c-owned-kernel-manager' \
  'lu-sec06c-owned-git-launcher.py:lu-sec06c-launch-git' \
  'lu-sec06c-owned-git-worker.py:lu-sec06c-owned-git-worker' \
  'lu-sec06c-agent01l-fixture-launcher.py:lu-sec06c-launch-agent01l-fixture' \
  'lu-sec06c-agent01l-fixture-manager.py:lu-sec06c-agent01l-fixture-manager'; do
  src_name="$(printf '%s' "$entry" | cut -d : -f 1)"
  dest_name="$(printf '%s' "$entry" | cut -d : -f 2)"
  source="scripts/$src_name"
  target="/usr/libexec/lu/$dest_name"
  if [[ ! -f "$source" || -L "$source" ]]; then
    echo "Missing exact checked-in non-symlink source: $source" >&2; exit 78
  fi
  expected_blob="$(git -c safe.directory="$(pwd)" rev-parse "HEAD:$source")"
  actual_blob="$(git hash-object -- "$source")"
  if [[ "$actual_blob" != "$expected_blob" ]]; then
    echo "Unreviewed source modification: $source" >&2; exit 78
  fi
  install -o root -g root -m 0755 "$source" "$target"
  cmp -s "$source" "$target" || {
    echo "Immutable first-party installed executable differs: $target" >&2; exit 78;
  }
done
# The sudo launcher accepts ONLY a compiled original crash-parent executable
# pinned by a ROOT-owned digest. The user-owned checkout cannot alter the
# compiled identity after admission or substitute a same-named executable.
host="src-tauri/lu-win-worker-job/target/debug/lu-e1-p08-linux-cgroup-crash-parent"
if [[ ! -f "$host" || -L "$host" || ! -x "$host" ]]; then
  echo 'Original native Rust crash-parent must be built from exact reviewed SHA first' >&2
  exit 78
fi
digest="$(sha256sum "$host" | cut -d ' ' -f 1)"
if [[ ! "$digest" =~ ^[0-9a-f]{64}$ ]]; then
  echo 'Original native fixture binary digest invalid' >&2; exit 78
fi
manifest="/usr/libexec/lu/lu-agent01l-fixture-host.sha256"
if [[ -L "$manifest" ]]; then
  echo 'Untrusted existing original fixture digest symlink' >&2; exit 78
fi
printf '%s\n' "$digest" > "$manifest"
chown root:root "$manifest"
chmod 0644 "$manifest"
# Only two FINITE root-owned entrypoints; neither admits arbitrary command,
# model-selected executable, target PID kill, network, or general sudo shell.
policy="/etc/sudoers.d/lu-sec06c-agent01l-native"
if [[ -L "$policy" ]]; then
  echo 'Untrusted existing sudo policy symlink' >&2; exit 78
fi
printf '%s ALL=(root) NOPASSWD: /usr/libexec/lu/lu-sec06c-launch-git *, /usr/libexec/lu/lu-sec06c-launch-agent01l-fixture *\n' "$user" > "$policy"
chown root:root "$policy"
chmod 0440 "$policy"
visudo -cf "$policy"
echo "AGENT01L_NATIVE_FIXED_BROKERS_INSTALLED_SOURCE=$expected"
echo "AGENT01L_POSITIVE_KERNEL_ACCEPTANCE=OPEN"
echo 'The original hostile-ancestor, SIGKILL, nested setsid, cgroup.kill,'
echo 'kernel populated-zero and native Git/restart tests still must run.'
