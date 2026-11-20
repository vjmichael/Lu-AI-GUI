#!/usr/bin/python3
"""Original fixed-path legacy marker migration test; disposable offline VM only.

Run prepare as root in a dedicated quiescent guest, then recover after actual
reboot of the same overlay. This test never alters production marker state.
"""
import hashlib
import json
import os
import pathlib
import runpy
import stat
import subprocess
import sys

KERNEL=pathlib.Path('/usr/libexec/lu/lu-sec06c-owned-kernel-manager')
SHARED=pathlib.Path('/run/lu-sec06c')
DENIAL=pathlib.Path('/var/lib/lu-sec06c/legacy-denials')
RECEIPT=pathlib.Path('/var/lib/lu-sec06c/legacy-retention-fixture.json')

def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def marker_state(p):
    meta=p.lstat()
    return {'mode':meta.st_mode,'uid':meta.st_uid,'ino':meta.st_ino,'value':os.readlink(p) if p.is_symlink() else digest(p)}
def sync(p):
    fd=os.open(p,os.O_RDONLY|os.O_DIRECTORY)
    try:os.fsync(fd)
    finally:os.close(fd)
def no_positive(nonces):
    for subdir in ('receipts','probe-receipts','native-receipts','browser-receipts'):
        for nonce in nonces:
            assert not os.path.lexists(RECEIPT.parent/subdir/(nonce+'.receipt'))
def main():
    assert os.geteuid()==0 and pathlib.Path('/proc/1/comm').read_text().strip()=='systemd'
    assert sorted(p.name for p in pathlib.Path('/sys/class/net').iterdir())==['lo']
    boot=pathlib.Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    k=runpy.run_path(str(KERNEL),run_name='lu_sec06f_retention_fixture')
    if sys.argv[1]=='prepare':
        assert not RECEIPT.exists()
        # Only disposable-guest-created markers. No live unit is selected or removed.
        active=subprocess.check_output(['systemctl','list-units','--no-legend','--state=active,activating,deactivating','lu-sec06c-*.service'],text=True)
        assert not active.strip()
        SHARED.mkdir(mode=0o755,parents=True,exist_ok=True)
        DENIAL.mkdir(mode=0o755,parents=True,exist_ok=True)
        nonces=[os.urandom(16).hex() for _ in range(3)]
        markers=[SHARED/(nonce+'.go') for nonce in nonces]
        markers[0].write_bytes(b'LU_SEC06C_GO_V1\n'+nonces[0].encode()+b'\n')
        markers[1].write_bytes(b'')
        markers[2].symlink_to('/nonexistent-legacy-marker-fixture')
        partial=DENIAL/(nonces[1]+'.denied');partial.write_bytes(b'')
        unrelated=SHARED/'unrelated-preserve-fixture';unrelated.write_bytes(b'unchanged')
        before=[marker_state(p) for p in markers]
        action=['/usr/bin/python3','/mnt/deploy/lu-sec06f-retain-legacy-admissions.py']
        for _ in range(2):
            out=subprocess.check_output(action,text=True)
            assert 'SEC06F_LEGACY_DENIALS_RETAINED=3' in out
            assert [marker_state(p) for p in markers]==before
            assert unrelated.read_bytes()==b'unchanged' and partial.read_bytes()==b''
        no_positive(nonces)
        hashes=[digest(DENIAL/(n+'.denied')) for n in nonces]
        for n in nonces:
            try:k['require_unspent_legacy_nonce'](n)
            except RuntimeError:pass
            else:raise AssertionError('legacy admission permitted')
        fresh=os.urandom(16).hex();k['require_unspent_legacy_nonce'](fresh)
        assert not (DENIAL/(fresh+'.denied')).exists()
        with RECEIPT.open('x') as f:
            json.dump({'boot':boot,'nonces':nonces,'hashes':hashes},f);f.flush();os.fsync(f.fileno())
        sync(RECEIPT.parent)
        print('SEC06F_LEGACY_PRESERVED_NONDESTRUCTIVE_PASS=1',flush=True)
    elif sys.argv[1]=='recover':
        witness=json.loads(RECEIPT.read_text());assert witness['boot']!=boot
        for n,h in zip(witness['nonces'],witness['hashes']):
            assert not os.path.lexists(SHARED/(n+'.go'))
            path=DENIAL/(n+'.denied');meta=path.lstat()
            assert stat.S_ISREG(meta.st_mode) and meta.st_uid==0 and not meta.st_mode&0o022 and meta.st_nlink==1
            assert digest(path)==h
            try:k['require_unspent_legacy_nonce'](n)
            except RuntimeError:pass
            else:raise AssertionError('legacy replay permitted after reboot')
        no_positive(witness['nonces'])
        print('SEC06F_LEGACY_REBOOT_REPLAY_DENIED_PASS=1',flush=True)
    else:raise RuntimeError('expected prepare or recover')

if __name__=='__main__':main()
