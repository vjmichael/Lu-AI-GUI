#!/usr/bin/python3
"""Fixed Lu desktop reviewed worker mediator; always unprivileged.

Only scanner, safe archive preview/extract, fixed E1 version/artifact, or
host-reviewed app version are permitted. This is NOT a shell/exec API:
a root-owned outer broker binds each argument by hash and exact human turn;
the mediator validates the finite operation again after UID drop.
"""
import argparse
import hashlib
import os
import pathlib
import re
import select
import stat
import subprocess
import sys
import time

HEX64=re.compile(r"^[a-f0-9]{64}$")
KINDS={"scan-file","archive-preview","archive-extract",
       "fixed-version","fixed-artifact","app-version"}
LIMITS={
 "scan-file":128*1024,
 "archive-preview":0,
 "archive-extract":0,
 "fixed-version":32*1024,
 "fixed-artifact":16*1024*1024,
 "app-version":16*1024
}
def reject(reason):raise RuntimeError("SEC06C_NATIVE_MEDIATOR_DENIED: "+reason)
def safe_name(s):
    return (0<len(s)<=128 and not s.startswith(".") and ".." not in s
      and all(c.isascii() and (c.isalnum() or c in "._-") for c in s))
def canonical(p,folder=False):
    path=pathlib.Path(p)
    return (path.is_absolute() and not path.is_symlink()
      and path.resolve(strict=True)==path and
      (path.is_dir() if folder else path.is_file()))
def target_new(path):
    p=pathlib.Path(path)
    return (p.is_absolute() and canonical(str(p.parent),True)
        and safe_name(p.name) and not p.exists() and not p.is_symlink())
def finite(kind,args):
    if kind=="scan-file":return (len(args)==2
       and args[0]=="--lu-file-scan-worker" and canonical(args[1]))
    if kind=="archive-preview":return (len(args)==6
       and args[0]=="--lu-archive-preview-worker"
       and canonical(args[1],True) and safe_name(args[2])
       and target_new(args[3]) and HEX64.fullmatch(args[4]+args[4])
       and args[5].isdigit() and int(args[5])>1)
    if kind=="archive-extract":return (len(args)==9
       and args[0]=="--lu-archive-extract-worker"
       and canonical(args[1],True) and safe_name(args[2])
       and canonical(args[3],True) and safe_name(args[4])
       and HEX64.fullmatch(args[5]) and HEX64.fullmatch(args[6]+args[6])
       and HEX64.fullmatch(args[7]+args[7])
       and args[8].isdigit() and int(args[8])>1)
    if kind=="fixed-version":return args==["--lu-e1-version-one-shot"]
    if kind=="fixed-artifact":return (len(args)==2 and
       args[0]=="--lu-e1-artifact-one-shot" and args[1] in ("docx","xlsx"))
    if kind=="app-version":return args in (["--version"],["-version"])
    return False
def digest(exe,argv):
    return hashlib.sha256(("\0".join([exe]+argv)+"\0").encode("utf-8")).hexdigest()
def attested_binary(exe,expected):
    if not HEX64.fullmatch(expected) or not canonical(exe):
        reject("reviewed binary path changed")
    image=pathlib.Path(exe)
    st=image.stat()
    if st.st_uid!=0 or st.st_mode&0o022:
        reject("reviewed executable writable outside root")
    h=hashlib.sha256()
    with image.open("rb") as f:
        for block in iter(lambda:f.read(65536),b""):h.update(block)
    if h.hexdigest()!=expected:reject("reviewed executable digest changed")
def root_go(nonce):
    root=pathlib.Path("/run/lu-sec06c")
    st=root.lstat()
    if not stat.S_ISDIR(st.st_mode) or st.st_uid!=0 or st.st_mode&0o022:
        reject("root-owned one-use GO dir unsafe")
    marker=root/(nonce+".go")
    until=time.monotonic()+20
    while time.monotonic()<until:
        try:info=marker.lstat()
        except FileNotFoundError:
            time.sleep(.02);continue
        if (not stat.S_ISREG(info.st_mode) or info.st_uid!=0
            or info.st_mode&0o022 or info.st_nlink!=1):
            reject("root GO identity altered")
        if marker.read_text(encoding="ascii")!="LU_SEC06C_GO_V1\n"+nonce+"\n":
            reject("original root GO nonce mismatch")
        return
    reject("original trusted supervisor issued no one-use GO")
def run(opts):
    if os.geteuid()==0:reject("root reviewed worker forbidden")
    if opts.kind not in KINDS or not re.fullmatch("[a-f0-9]{32}",opts.nonce):
        reject("invalid finite operation")
    if len(opts.arg)>12 or any(len(x)>4096 or "\0" in x
        or any(ord(c)<32 for c in x) for x in opts.arg):
        reject("finite worker args invalid")
    if not finite(opts.kind,opts.arg):
        reject("exact finite reviewed argument contract denied")
    if digest(opts.binary,opts.arg)!=opts.args_sha:
        reject("host-to-root finite arguments digest changed")
    attested_binary(opts.binary,opts.binary_sha)
    # The trusted OS manager remains OUTSIDE the leaf; this nonroot
    # intermediary is the archive worker's REAL immediate parent inside it.
    # PDEATHSIG binds the archive operation to this mediator, while the
    # root manager independently reaps the entire pinned cgroup on exit.
    args=list(opts.arg)
    if opts.kind.startswith("archive-"):
        args[-1]=str(os.getpid())
    root_go(opts.nonce)
    child=subprocess.Popen([opts.binary]+args,
        stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,
        env={"PATH":"/usr/bin:/bin","LC_ALL":"C","HOME":"/nonexistent"},
        close_fds=True,start_new_session=True)
    limit=LIMITS[opts.kind]
    forwarded=0
    deadline=time.monotonic()+(600 if opts.kind.startswith("archive-") else 90)
    try:
        while True:
            if time.monotonic()>deadline:
                reject("bounded original reviewed worker exceeded deadline")
            readable,_,_=select.select([child.stdout,sys.stdin.buffer],[],[],.03)
            if sys.stdin.buffer in readable and not sys.stdin.buffer.read(1):
                reject("original Lu host cancelled or died")
            if child.stdout in readable:
                chunk=os.read(child.stdout.fileno(),65536)
                if chunk:
                    forwarded+=len(chunk)
                    if forwarded>limit:
                        reject("bounded native stdout exceeded finite contract")
                    sys.stdout.buffer.write(chunk)
                    sys.stdout.buffer.flush()
                elif child.poll() is not None:
                    break
            if child.poll() is not None and not readable:break
        if child.wait(timeout=3)!=0:
            reject("original reviewed native worker failed")
        return 0
    finally:
        if child.poll() is None:
            child.kill();child.wait(timeout=5)
def main():
    ap=argparse.ArgumentParser(allow_abbrev=False)
    for name in ("nonce","kind","binary","binary_sha","args_sha"):
        ap.add_argument("--"+name.replace("_","-"),required=True)
    ap.add_argument("--arg",action="append",default=[])
    try:return run(ap.parse_args())
    except Exception as exc:
        print("SEC06C_OWNED_NATIVE_BLOCKED: "+str(exc),file=sys.stderr)
        return 93
if __name__=="__main__":sys.exit(main())
