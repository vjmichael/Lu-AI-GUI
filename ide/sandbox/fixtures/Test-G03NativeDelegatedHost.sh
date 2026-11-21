#!/usr/bin/env bash
# Execution only inside the exact transient lu-ide-delegated systemd unit.
set -euo pipefail
test "$#" -eq 4 || { echo IDE_G03_LINUX_HOST_ARGS_DENIED >&2; exit 2; }
prep="$1"; witness="$2"; hostile="$3"; source="$4"
for file in "$prep" "$witness" "$hostile" "$source"; do
  test -f "$file" || { echo IDE_G03_LINUX_HOST_PIN_MISSING >&2; exit 2; }
done
test "$(id -un)" = runner || { echo IDE_G03_LINUX_DELEGATED_NOT_RUNNER >&2; exit 2; }
"$prep"
"$witness"
echo IDE_G03_LINUX_REAL_CGROUP_CHILD_GRANDCHILD_KILL=PASS
/usr/bin/bwrap --unshare-all --die-with-parent --new-session \
  --ro-bind /usr /usr \
  --ro-bind-try /bin /bin --ro-bind-try /lib /lib \
  --ro-bind-try /lib64 /lib64 \
  --ro-bind "$source" /source --ro-bind "$hostile" /verify.py \
  --tmpfs /out --proc /proc --dev /dev \
  -- /usr/bin/python3 /verify.py
test "$(cat "$source")" = immutable-g03-source || { echo IDE_G03_LINUX_SOURCE_CHANGED >&2; exit 2; }
echo IDE_G03_LINUX_REAL_DELEGATED_CGROUP_AND_BWRAP=PASS
echo IDE_G03_LINUX_INSTALLED_PRODUCTION_WORKER=NOT_AUTHORIZED
