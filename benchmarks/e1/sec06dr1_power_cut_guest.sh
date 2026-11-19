#!/bin/bash
set -euo pipefail
cd /mnt/deploy; sha256sum -c SHA256SUMS
[[ $(cat /proc/1/comm) == systemd ]]
[[ $(findmnt -n -o FSTYPE /sys/fs/cgroup) == cgroup2 ]]
p=/home/luprobe/sec06dr1
if [[ $1 == prepare ]]; then
  useradd -m -s /bin/bash luprobe
  install -d -o luprobe -g luprobe -m 0700 "$p"
  install -d -o root -g root -m 0755 /usr/libexec/lu
  for pair in 'lu-sec06c-owned-native-launcher.py lu-sec06c-launch-native' 'lu-sec06c-owned-native-worker.py lu-sec06c-owned-native-worker' 'lu-sec06c-owned-kernel-manager.py lu-sec06c-owned-kernel-manager' 'scanner lu-file-scan-worker-acceptance' 'core-tests core-tests'; do set -- $pair; install -o root -g root -m 0755 "$1" "/usr/libexec/lu/$2"; done
  printf 'luprobe ALL=(root) NOPASSWD: /usr/libexec/lu/lu-sec06c-launch-native *\n' > /etc/sudoers.d/lu-probe
  chmod 0440 /etc/sudoers.d/lu-probe; visudo -cf /etc/sudoers.d/lu-probe
  runuser -u luprobe -- env LU_SEC06D_PROFILE="$p" LU_SEC06D_PHASE=prepare /usr/libexec/lu/core-tests --ignored --nocapture sec06d_r1_vm_power_cut &
  python3 /mnt/deploy/observer.py prepare
  wait
else
  python3 /mnt/deploy/observer.py recover
  python3 /mnt/deploy/observer.py delay-prepare
  runuser -u luprobe -- env LU_SEC06D_PROFILE="$p/late" LU_SEC06D_PHASE=missing /usr/libexec/lu/core-tests --ignored --nocapture sec06d_r1_vm_power_cut
  python3 /mnt/deploy/observer.py delay-restore
  runuser -u luprobe -- env LU_SEC06D_PROFILE="$p/late" LU_SEC06D_PHASE=late /usr/libexec/lu/core-tests --ignored --nocapture sec06d_r1_vm_power_cut
  runuser -u luprobe -- env LU_SEC06D_PROFILE="$p" LU_SEC06D_PHASE=recover /usr/libexec/lu/core-tests --ignored --nocapture sec06d_r1_vm_power_cut
fi
