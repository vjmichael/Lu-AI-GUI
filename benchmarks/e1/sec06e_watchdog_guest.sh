#!/bin/bash
set -euo pipefail
cd /mnt/deploy; sha256sum -c SHA256SUMS
[[ $(cat /proc/1/comm) == systemd ]]
[[ $(findmnt -n -o FSTYPE /sys/fs/cgroup) == cgroup2 ]]
useradd -m -s /bin/bash luprobe
install -d -o root -g root -m 0755 /usr/libexec/lu
for pair in 'lu-sec06c-owned-native-launcher.py lu-sec06c-launch-native' 'lu-sec06c-owned-native-worker.py lu-sec06c-owned-native-worker' 'lu-sec06c-owned-kernel-manager.py lu-sec06c-owned-kernel-manager' 'scanner lu-file-scan-worker-acceptance' 'core-tests core-tests'; do set -- $pair; install -o root -g root -m 0755 "$1" "/usr/libexec/lu/$2"; done
printf 'luprobe ALL=(root) NOPASSWD: /usr/libexec/lu/lu-sec06c-launch-native *\n' > /etc/sudoers.d/lu-probe
chmod 0440 /etc/sudoers.d/lu-probe; visudo -cf /etc/sudoers.d/lu-probe
runuser -u luprobe -- /usr/libexec/lu/core-tests --ignored --nocapture cp06er2_root_owned_scheduler_complete_and_stop
python3 /mnt/deploy/observer.py
