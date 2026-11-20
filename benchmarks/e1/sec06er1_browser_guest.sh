#!/bin/bash
set -euo pipefail
mode=${1:-all}
case "$mode" in all|normal-only|stall-only) ;; *) exit 2;; esac
cd /mnt/deploy; sha256sum -c SHA256SUMS
[[ $(cat /proc/1/comm) == systemd && $(findmnt -n -o FSTYPE /sys/fs/cgroup) == cgroup2 && $(ls /sys/class/net) == lo ]]
mkdir -p /tmp/host-browser-runtime
tar -xzf host-runtime.tar.gz -C /tmp/host-browser-runtime
python3 - <<'PY'
import pathlib,os,shutil,json,hashlib
r=pathlib.Path('/tmp/host-browser-runtime')
for f in json.loads((r/'host-dependency-inventory.json').read_text()):
 p=r/f['path'];assert hashlib.file_digest(p.open('rb'),'sha256').hexdigest()==f['sha256']
for folder in ('usr','etc'):
 for p in sorted((r/folder).rglob('*')):
  d=pathlib.Path('/')/p.relative_to(r)
  if d.exists() or d.is_symlink():continue
  if p.is_symlink(): d.parent.mkdir(parents=True,exist_ok=True); d.symlink_to(os.readlink(p))
  elif p.is_file(): d.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,d)
  elif p.is_dir():d.mkdir(parents=True,exist_ok=True)
PY
ldconfig
install -d -o root -g root -m 0755 /usr/lib/lu/browser /usr/libexec/lu
tar -xzf browser.tar.gz -C /usr/lib/lu/browser --strip-components=1
chown -R root:root /usr/lib/lu/browser; chmod -R go-w /usr/lib/lu/browser
for pair in 'lu-sec06c-owned-browser-launcher.py lu-sec06c-launch-browser' 'lu-sec06c-owned-browser-manager.py lu-sec06c-owned-browser-manager' 'lu-sec06c-owned-browser-worker.py lu-sec06c-owned-browser-worker' 'lu-sec06c-owned-kernel-manager.py lu-sec06c-owned-kernel-manager' 'core-tests core-tests'; do set -- $pair; install -o root -g root -m 0755 "$1" "/usr/libexec/lu/$2"; done
for file in /usr/bin/Xvfb /usr/bin/xauth /usr/bin/xkbcomp /usr/lib/lu/browser/chrome /usr/lib/lu/browser/node /usr/lib/lu/browser/xdotool; do echo "CHECK $file"; test -x "$file"; ldd "$file" > /tmp/lu-ldd; if grep 'not found' /tmp/lu-ldd; then exit 42; fi; done
install -o root -g root -m 0755 sec06er1_browser_renderer_observer.py /usr/libexec/lu/lu-sec06er1-browser-renderer-observer
ln -s /usr/lib/lu/browser/xdotool /usr/local/bin/xdotool
useradd -m -s /bin/bash luprobe
printf 'luprobe ALL=(root) NOPASSWD: /usr/libexec/lu/lu-sec06c-launch-browser *, /usr/libexec/lu/lu-sec06er1-browser-renderer-observer *, /usr/bin/systemctl kill --kill-whom=main --signal=SIGKILL lu-sec06c-browser-*.service, /usr/bin/systemctl kill --kill-whom=main --signal=SIGSTOP lu-sec06c-browser-*.service\n' > /etc/sudoers.d/lu-probe
chmod 0440 /etc/sudoers.d/lu-probe; visudo -cf /etc/sudoers.d/lu-probe
runuser -u luprobe -- xvfb-run -a -s '-screen 0 1280x800x24' /usr/lib/lu/browser/xdotool getdisplaygeometry
runuser -u luprobe -- /usr/lib/lu/browser/chrome --version
runuser -u luprobe -- /usr/lib/lu/browser/node --version
echo SEC06ER1_R1_GUEST_PINNED_RUNTIME_PREFLIGHT_PASS
if [[ $mode == normal-only ]]; then
 runuser -u luprobe -- env LU_SEC06C_BROKER_DEBUG=1 xvfb-run -a -s '-screen 0 1280x800x24' python3 /mnt/deploy/sec06c_browser_real_native.py --bundle /usr/lib/lu/browser --launcher /usr/libexec/lu/lu-sec06c-launch-browser
 exit 0
fi
if [[ $mode == stall-only ]]; then
 runuser -u luprobe -- env LU_SEC06C_BROKER_DEBUG=1 xvfb-run -a -s '-screen 0 1280x800x24' python3 /mnt/deploy/sec06c_browser_guardian_loss.py --stall --bundle /usr/lib/lu/browser --launcher /usr/libexec/lu/lu-sec06c-launch-browser
 exit 0
fi
set +e
failed=0
for fixture in sec06c_browser_real_native.py sec06c_browser_host_crash.py sec06c_browser_guardian_loss.py; do
 runuser -u luprobe -- env LU_SEC06C_BROKER_DEBUG=1 xvfb-run -a -s '-screen 0 1280x800x24' python3 "/mnt/deploy/$fixture" --bundle /usr/lib/lu/browser --launcher /usr/libexec/lu/lu-sec06c-launch-browser
 rc=$?; echo "SEC06ER1_R1_FIXTURE_RESULT $fixture $rc"; if [[ $rc != 0 ]]; then failed=1; fi
done
runuser -u luprobe -- env LU_SEC06C_BROKER_DEBUG=1 xvfb-run -a -s '-screen 0 1280x800x24' python3 /mnt/deploy/sec06c_browser_guardian_loss.py --stall --bundle /usr/lib/lu/browser --launcher /usr/libexec/lu/lu-sec06c-launch-browser
rc=$?; echo "SEC06F_BROWSER_WATCHDOG_FIXTURE_RESULT $rc"; if [[ $rc != 0 ]]; then failed=1; fi
runuser -u luprobe -- env LU_SEC06C_BROWSER_REVIEWED_BUNDLE=/usr/lib/lu/browser LU_SEC06C_BROKER_DEBUG=1 xvfb-run -a -s '-screen 0 1280x800x24' /usr/libexec/lu/core-tests --ignored --nocapture sec06c_real_linux_browser_original_grant_handoff_and_durable_recovery
rc=$?; echo "SEC06ER1_R1_RUST_RESULT $rc"; if [[ $rc != 0 ]]; then failed=1; fi
exit "$failed"
