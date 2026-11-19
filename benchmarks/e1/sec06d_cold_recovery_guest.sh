#!/bin/bash
set -euo pipefail
cd /mnt/deploy; sha256sum -c SHA256SUMS
[[ $(cat /proc/1/comm) == systemd ]]
[[ $(findmnt -n -o FSTYPE /sys/fs/cgroup) == cgroup2 ]]
p=/home/luprobe/sec06d
if [[ $1 == prepare ]]; then
  useradd -m -s /bin/bash luprobe
  install -d -o luprobe -g luprobe -m 0700 "$p"
  install -d -o root -g root -m 0755 /usr/libexec/lu
  for pair in 'lu-sec06c-owned-native-launcher.py lu-sec06c-launch-native' 'lu-sec06c-owned-native-worker.py lu-sec06c-owned-native-worker' 'lu-sec06c-owned-kernel-manager.py lu-sec06c-owned-kernel-manager' 'scanner lu-file-scan-worker-acceptance' 'core-tests core-tests'; do set -- $pair; install -o root -g root -m 0755 "$1" "/usr/libexec/lu/$2"; done
  printf 'luprobe ALL=(root) NOPASSWD: /usr/libexec/lu/lu-sec06c-launch-native *\n' > /etc/sudoers.d/lu-probe
  chmod 0440 /etc/sudoers.d/lu-probe; visudo -cf /etc/sudoers.d/lu-probe
  runuser -u luprobe -- env LU_SEC06D_PROFILE="$p" LU_SEC06D_PHASE=prepare /usr/libexec/lu/core-tests --ignored --nocapture sec06d_vm_cold_recovery
  python3 - <<'PY'
import pathlib,sqlite3,os,pwd,hashlib
p=pathlib.Path('/home/luprobe/sec06d'); r=pathlib.Path('/var/lib/lu-sec06c/native-receipts')
with sqlite3.connect(p/'tasks.sqlite') as c:
 rows=c.execute('SELECT t.task_id,t.state,a.nonce,u.original_uid FROM lu_tasks t JOIN lu_task_linux_os_owned_native_admission a ON a.task_id=t.task_id JOIN lu_task_native_prelaunch_uid u ON u.task_id=t.task_id ORDER BY t.task_id').fetchall()
assert [(x[0],x[1]) for x in rows]==[('scan-'+'7'*32,'PENDING'),('scan-'+'8'*32,'RUNNING')]
assert all(x[3]==pwd.getpwnam('luprobe').pw_uid for x in rows)
assert not (r/(rows[0][2]+'.receipt')).exists()
receipt=r/(rows[1][2]+'.receipt'); assert receipt.stat().st_uid==0 and receipt.stat().st_nlink==1 and 'EFFECT=COMPLETED' in receipt.read_text()
pathlib.Path('/root/sec06d-boot-id').write_text(pathlib.Path('/proc/sys/kernel/random/boot_id').read_text())
pathlib.Path('/root/sec06d-receipt-hash').write_text(hashlib.sha256(receipt.read_bytes()).hexdigest())
print('PREBOOT_INDEPENDENT_ADMISSION_RECEIPT_PASS')
PY
else
  [[ $(cat /root/sec06d-boot-id) != $(cat /proc/sys/kernel/random/boot_id) ]]
  python3 - <<'PY'
import pathlib,hashlib,sqlite3
p=pathlib.Path('/home/luprobe/sec06d'); r=pathlib.Path('/var/lib/lu-sec06c/native-receipts'); f=list(r.glob('*.receipt')); assert len(f)==1
assert hashlib.sha256(f[0].read_bytes()).hexdigest()==pathlib.Path('/root/sec06d-receipt-hash').read_text()
assert not list(pathlib.Path('/sys/fs/cgroup/system.slice').glob('lu-sec06c-native-*.service'))
print('DISTINCT_BOOT_ORIGINAL_RECEIPT_HASH_PASS')
PY
  runuser -u luprobe -- env LU_SEC06D_PROFILE="$p" LU_SEC06D_PHASE=recover /usr/libexec/lu/core-tests --ignored --nocapture sec06d_vm_cold_recovery
fi
