#!/usr/bin/python3
"""SEC-06C dedicated fixed native Lu scanner/archive/utility broker.

This separate reviewed sudo policy never launches arbitrary user programs,
never accepts browser URLs or model commands and never runs a desktop worker
as root. It provisions only the installed root-owned systemd kernel manager
with one of the six explicitly typed, independently checked native modes.
"""
import argparse
import hashlib
import os
import pathlib
import pwd
import re
import stat
import subprocess
import sys

MANAGER=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-kernel-manager")
NATIVE=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-native-worker")
PYTHON=pathlib.Path("/usr/bin/python3").resolve(strict=True)
SYSTEMD=pathlib.Path("/usr/bin/systemd-run")
RUN_DIR=pathlib.Path("/run/lu-sec06c")
RECEIPTS=pathlib.Path("/var/lib/lu-sec06c/native-receipts")
KINDS={"scan-file","archive-preview","archive-extract",
       "fixed-version","fixed-artifact","app-version"}
REVIEW_ID=re.compile(r"^[a-zA-Z0-9_-]{1,128}$")
HEX32=re.compile(r"^[a-f0-9]{32}$")
HEX64=re.compile(r"^[a-f0-9]{64}$")
def deny(reason):raise RuntimeError("SEC06C_NATIVE_BROKER_DENIED: "+reason)
def root_file(path):
    p=pathlib.Path(path)
    if not p.is_absolute() or p.is_symlink():deny("untrusted installed file")
    st=p.stat()
    if not stat.S_ISREG(st.st_mode) or st.st_uid!=0 or st.st_mode&0o022:
        deny("root installed binary is mutable")
    for d in (p.parent,p.parent.parent):
        st=d.stat()
        if st.st_uid!=0 or st.st_mode&0o022:deny("root installed parent mutable")
def root_dir(path):
    path.mkdir(mode=0o755,parents=True,exist_ok=True)
    st=path.lstat()
    if not stat.S_ISDIR(st.st_mode) or st.st_uid!=0 or st.st_mode&0o022:
        deny("root admission/receipt dir changed")
def digest(exe,args):
    return hashlib.sha256(("\0".join([exe]+args)+"\0").encode("utf-8")).hexdigest()
def safe_name(s):
    return 0<len(s)<=128 and not s.startswith(".") and ".." not in s and all(
        c.isascii() and (c.isalnum() or c in "._-") for c in s)
def canonical(path,directory=False):
    p=pathlib.Path(path)
    return p.is_absolute() and not p.is_symlink() and p.resolve(strict=True)==p and (
        p.is_dir() if directory else p.is_file())
def preview_output(path):
    p=pathlib.Path(path)
    return p.is_absolute() and canonical(str(p.parent),True) and safe_name(p.name) and not p.exists()
def finite(kind,args):
    if kind=="scan-file":
        return len(args)==2 and args[0]=="--lu-file-scan-worker" and canonical(args[1])
    if kind=="archive-preview":
        return len(args)==6 and args[0]=="--lu-archive-preview-worker" and (
            canonical(args[1],True) and safe_name(args[2])
            and preview_output(args[3]) and HEX32.fullmatch(args[4])
            and args[5].isdigit() and int(args[5])>1)
    if kind=="archive-extract":
        return len(args)==9 and args[0]=="--lu-archive-extract-worker" and (
            canonical(args[1],True) and safe_name(args[2])
            and canonical(args[3],True) and safe_name(args[4])
            and HEX64.fullmatch(args[5])
            and HEX32.fullmatch(args[6]) and HEX32.fullmatch(args[7])
            and args[8].isdigit() and int(args[8])>1)
    if kind=="fixed-version":return args==["--lu-e1-version-one-shot"]
    if kind=="fixed-artifact":return (
        len(args)==2 and args[0]=="--lu-e1-artifact-one-shot"
        and args[1] in ("docx","xlsx"))
    if kind=="app-version":return args in (["--version"],["-version"])
    return False
def binary(exe,sha):
    if not canonical(exe) or not HEX64.fullmatch(sha):
        deny("original reviewed executable path and digest missing")
    st=pathlib.Path(exe).stat()
    if st.st_uid!=0 or st.st_mode&0o022:
        deny("reviewed app not in trusted root-owned installation")
    h=hashlib.sha256()
    with open(exe,"rb") as f:
        for buf in iter(lambda:f.read(65536),b""):h.update(buf)
    if h.hexdigest()!=sha:deny("reviewed executable changed since host approval")
def run(opts):
    if os.geteuid()!=0 or "SUDO_UID" not in os.environ:
        deny("requires exact original sudo policy")
    uid=int(os.environ["SUDO_UID"])
    if uid<=0 or pwd.getpwuid(uid).pw_uid!=uid:
        deny("unprivileged first-party host identity missing")
    if (not HEX32.fullmatch(opts.nonce) or opts.kind not in KINDS
      or not HEX64.fullmatch(opts.args_sha)
      or any(not REVIEW_ID.fullmatch(i) for i in
             (opts.chat,opts.turn,opts.call))):
        deny("invalid original source/nonce/kind")
    if (len(opts.arg)>12 or any(len(a)>4096 or "\0" in a or
        any(ord(c)<32 for c in a) for a in opts.arg)):
        deny("bounded original argv invalid")
    for p in (pathlib.Path(__file__).resolve(),MANAGER,NATIVE,PYTHON,SYSTEMD):
        root_file(p)
    binary(opts.binary,opts.binary_sha)
    if not finite(opts.kind,opts.arg) or digest(opts.binary,opts.arg)!=opts.args_sha:
        deny("not one of six finite reviewed native argument contracts")
    root_dir(RUN_DIR);root_dir(RECEIPTS)
    receipt=RECEIPTS/(opts.nonce+".receipt")
    if receipt.exists() or (RUN_DIR/(opts.nonce+".go")).exists():
        deny("original one-use native nonce replay")
    unit=f"lu-sec06c-native-{opts.nonce}.service"
    argv=[
        str(SYSTEMD),"--system","--quiet","--wait","--pipe",
        "--no-ask-password","--service-type=exec",f"--unit={unit}",
        "--property=User=root","--property=Delegate=yes",
        "--property=KillMode=control-group","--property=PrivateNetwork=yes",
        "--property=NoNewPrivileges=yes",
        "--property=CapabilityBoundingSet=CAP_SYS_ADMIN CAP_SETUID CAP_SETGID CAP_KILL",
        "--property=RestrictSUIDSGID=yes",
        "--property=TasksMax=32","--property=MemoryMax=536870912",
        "--property=RuntimeMaxSec=900",
        str(PYTHON),str(MANAGER),"--domain","native","--nonce",opts.nonce,
        "--chat",opts.chat,"--turn",opts.turn,"--call",opts.call,
        "--uid",str(uid),"--op",opts.kind,
        "--binary",opts.binary,"--binary-sha",opts.binary_sha,
        "--args-sha",opts.args_sha,
    ]
    for arg in opts.arg:argv.append("--arg="+arg)
    result=subprocess.run(argv,cwd="/",check=False,timeout=930)
    if not receipt.is_file() or receipt.is_symlink():
        deny("trusted kernel supervisor produced no original native receipt")
    st=receipt.stat()
    if st.st_uid!=0 or st.st_mode&0o022 or st.st_nlink!=1 or st.st_size>1024:
        deny("native original kernel receipt mutable")
    with receipt.open("r",encoding="ascii") as f:raw=f.read(1025)
    prefix="\n".join([
        "LU_SEC06C_OWNED_NATIVE_V1","NONCE="+opts.nonce,
        "CHAT="+opts.chat,"TURN="+opts.turn,"CALL="+opts.call,
        "KIND="+opts.kind,"BINARY_SHA256="+opts.binary_sha,
        "ARGV_SHA256="+opts.args_sha,"UNIT="+unit,
        "ORIGINAL_UID="+str(uid),"KERNEL_EMPTY=1","EFFECT=",
    ])
    if raw==prefix+"COMPLETED\n" and result.returncode==0:return 0
    if raw==prefix+"INTERRUPTED\n" and result.returncode!=0:return 73
    deny("kernel unit/receipt mismatch or original native operation interrupted")
def main():
    ap=argparse.ArgumentParser(allow_abbrev=False)
    for key in ("nonce","chat","turn","call","kind","binary","binary_sha","args_sha"):
        ap.add_argument("--"+key.replace("_","-"),required=True)
    ap.add_argument("--arg",action="append",default=[])
    try:return run(ap.parse_args())
    except Exception as exc:
        print("SEC06C_ROOT_NATIVE_BROKER_BLOCKED: "+str(exc),file=sys.stderr)
        return 91
if __name__=="__main__":sys.exit(main())
