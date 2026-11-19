#!/usr/bin/python3
"""Independent administrator-only renderer witness for disposable Linux testing.

Never packaged or called by Lu. Only read the original admitted service,
original nonroot Chromium and Node images, and the exact renderer(s).
"""
import os
import pathlib
import re
import select
import stat
import sys
import time

CGROUP=pathlib.Path('/sys/fs/cgroup/system.slice')
BUNDLE=pathlib.Path('/usr/lib/lu/browser')
PASS='SEC06C_CHROMIUM_REAL_USERNS_SECCOMP_PASS=1 renderer_in_original_kernel_leaf=1 renderer_userns_distinct=1 seccomp_bpf=1 exact_chromium_image=1'
def require(ok):
    if not ok:raise RuntimeError('original renderer identity not verified')
def membership(pid):return pathlib.Path(f'/proc/{pid}/cgroup').read_text().strip()
def same_image(pid,path):
    a=pathlib.Path(f'/proc/{pid}/exe').stat();b=path.stat()
    return (a.st_dev,a.st_ino)==(b.st_dev,b.st_ino)
def status(pid):
    return dict(line.split(':',1) for line in pathlib.Path(f'/proc/{pid}/status').read_text().splitlines() if ':' in line)
def live(fd):
    poll=select.poll();poll.register(fd,select.POLLIN);return not poll.poll(0)
def root_directory(path):
    info=path.lstat();require(stat.S_ISDIR(info.st_mode) and info.st_uid==0 and not info.st_mode&0o022)
def main():
    require(os.geteuid()==0 and len(sys.argv)==4)
    nonce,chrome,node=sys.argv[1:]
    require(re.fullmatch('[a-f0-9]{32}',nonce) is not None and chrome.isdecimal() and node.isdecimal())
    chrome,node=int(chrome),int(node)
    uid=int(os.environ.get('SUDO_UID','0'));require(uid>0 and min(chrome,node)>1 and chrome!=node)
    unit=CGROUP/f'lu-sec06c-browser-{nonce}.service';leaf=unit/f'lu-sec06c-leaf-{nonce}'
    for p in (unit,leaf,BUNDLE):root_directory(p)
    group='0::/'+str(leaf.relative_to('/sys/fs/cgroup'))
    for pid,image in ((chrome,BUNDLE/'chrome'),(node,BUNDLE/'node')):
        require(membership(pid)==group and same_image(pid,image))
        require(status(pid)['Uid'].split()[0]==str(uid))
    chrome_fd=os.pidfd_open(chrome);node_fd=os.pidfd_open(node)
    try:
        chrome_userns=os.readlink(f'/proc/{chrome}/ns/user');deadline=time.monotonic()+10
        candidates=0; titles=0
        while time.monotonic()<deadline:
            require(live(chrome_fd) and live(node_fd) and membership(chrome)==group and membership(node)==group)
            candidates=titles=0
            for raw in (leaf/"cgroup.procs").read_text().split():
                require(raw.isdecimal());pid=int(raw)
                if pid in (chrome,node) or pid<=1:continue
                try:
                    fd=os.pidfd_open(pid)
                    try:
                        if not same_image(pid,BUNDLE/'chrome'):continue
                        candidates+=1
                        cmd=pathlib.Path(f'/proc/{pid}/cmdline').read_bytes()
                        # Linux Chromium may present a rewritten space-delimited
                        # process title instead of preserving argv NUL boundaries.
                        tokens=re.split(rb'[\x00 \t]+',cmd)
                        if b"--type=renderer" not in tokens:continue
                        titles+=1
                        require(b"--no-sandbox" not in tokens and b"--disable-setuid-sandbox" not in tokens)
                        status_lines=status(pid);new_userns=os.readlink(f'/proc/{pid}/ns/user')
                        if (live(fd) and membership(pid)==group and same_image(pid,BUNDLE/'chrome')
                            and status_lines.get("Seccomp","").strip()=='2'
                            and status_lines.get('NoNewPrivs','').strip()=='1'
                            and chrome_userns!=new_userns):
                            require(live(chrome_fd) and live(node_fd))
                            print(PASS,flush=True);return 0
                    finally:os.close(fd)
                except (FileNotFoundError,ProcessLookupError):continue
            time.sleep(.05)
        print(f'RENDERER_WITNESS_NOT_VERIFIED chrome_image_candidates={candidates} renderer_titles={titles}',file=sys.stderr)
        return 93
    finally:os.close(chrome_fd);os.close(node_fd)
if __name__=='__main__':
    try:sys.exit(main())
    except (OSError,ValueError,RuntimeError,KeyError):
        print('RENDERER_WITNESS_NOT_VERIFIED identity_or_kernel_observation',file=sys.stderr);sys.exit(93)
