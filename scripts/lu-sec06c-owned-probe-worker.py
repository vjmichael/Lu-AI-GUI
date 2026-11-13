#!/usr/bin/python3
"""SEC-06C reviewed media/metadata worker; not a shell or general executor.

Privileged fixed launcher supplies a finite kind + reviewed executable and
resource SHA-256. Worker runs only as original nonroot UID inside protected
systemd unit. Resources are opened without following symlinks, hashed by the
worker, then passed by pinned FD to the reviewed finite CLI.
"""
import argparse
import hashlib
import os
import pathlib
import select
import stat
import subprocess
import sys
import tempfile
import time

RUN_DIR=pathlib.Path("/run/lu-sec06c")
FIELDS="format=duration:stream=index,codec_type,codec_name,width,height,sample_rate,channels"
OPS={
  "ffprobe-wav": ("ffprobe",("wav",),256*1024*1024,16384),
  "ffprobe-mp4": ("ffprobe",("mp4",),256*1024*1024,16384),
  "pdfinfo-pdf": ("pdfinfo",("pdf",),32*1024*1024,8192),
  "exiftool-image": ("exiftool",("png","jpg","jpeg"),32*1024*1024,8192),
}
def deny(why): raise RuntimeError("SEC06C_PROBE_WORKER_DENIED: "+why)
def hexid(s,n):return len(s)==n and all(c in "0123456789abcdef" for c in s)
def review_binary(p,sha,kind):
    exe=pathlib.Path(p)
    if not exe.is_absolute() or exe.is_symlink() or not hexid(sha,64):
        deny("invalid reviewed executable identity")
    real=exe.resolve(strict=True)
    if real!=exe or real.name!=OPS[kind][0] or not real.is_file():
        deny("reviewed executable is not exact finite probe")
    if real.stat().st_uid!=0 or real.stat().st_mode & 0o022:
        deny("reviewed executable writable outside root")
    digest=hashlib.sha256()
    with real.open("rb") as source:
        for chunk in iter(lambda:source.read(65536),b""):digest.update(chunk)
    if digest.hexdigest()!=sha:deny("reviewed executable digest changed")
    return real
def kernel_guard(nonce):
    if os.geteuid()==0:deny("root worker forbidden")
    rows=pathlib.Path("/proc/self/cgroup").read_text().splitlines()
    if len(rows)!=1 or not rows[0].startswith("0::/"):
        deny("unified kernel cgroup missing")
    group=rows[0][3:]
    if group!="/" and not group.endswith("/lu-sec06c-probe-"+nonce+".service"):
        deny("not original root-owned operation unit")
    mount=pathlib.Path("/sys/fs/cgroup")
    p=(mount/group.lstrip("/")).resolve(strict=True)
    if p.stat().st_uid==os.geteuid() or os.access(p/"cgroup.procs",os.W_OK):
        deny("worker controls operation cgroup")
    if group!="/" and os.access(p.parent/"cgroup.procs",os.W_OK):
        deny("worker can migrate into ancestor")
    mounts=[x for x in pathlib.Path("/proc/self/mountinfo").read_text().splitlines()
        if " - cgroup2 " in x and x.split(" - ",1)[0].split()[4]=="/sys/fs/cgroup"]
    if len(mounts)!=1 or "ro" not in mounts[0].split(" - ",1)[0].split()[5].split(","):
        deny("systemd ProtectControlGroups=yes missing")
def approved_first_launch(nonce):
    if RUN_DIR.is_symlink() or RUN_DIR.stat().st_uid!=0 or RUN_DIR.stat().st_mode & 0o022:
        deny("root first-worker approval directory writable")
    target=RUN_DIR/(nonce+".go")
    deadline=time.monotonic()+20
    while time.monotonic()<deadline:
        readable,_,_=select.select([sys.stdin.buffer],[],[],0)
        if readable and not sys.stdin.buffer.read(1):
            deny("original Lu host died before first-worker approval")
        try:
            st=target.lstat()
        except FileNotFoundError:
            time.sleep(.025);continue
        if not stat.S_ISREG(st.st_mode) or st.st_uid!=0 or st.st_mode&0o022 or st.st_nlink!=1:
            deny("one-use OS approval mutable")
        if target.read_text(encoding="ascii")!="LU_SEC06C_GO_V1\n"+nonce+"\n":
            deny("one-use OS approval identity mismatch")
        return
    deny("system manager approval timed out")
def pinned_input(path,kind,sha):
    p=pathlib.Path(path)
    if not p.is_absolute() or p.is_symlink() or p.resolve(strict=True)!=p:
        deny("resource path changed from original host selection")
    if p.suffix.lower().lstrip(".") not in OPS[kind][1] or not hexid(sha,64):
        deny("resource type or original content digest invalid")
    fd=os.open(p,os.O_RDONLY|os.O_CLOEXEC|os.O_NOFOLLOW)
    try:
        info=os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or not 0<info.st_size<=OPS[kind][2]:
            deny("original media/metadata resource outside bounded size")
        h=hashlib.sha256()
        while True:
            data=os.read(fd,65536)
            if not data:break
            h.update(data)
        if h.hexdigest()!=sha:deny("original reviewed resource changed")
        os.lseek(fd,0,os.SEEK_SET)
        return fd
    except Exception:
        os.close(fd);raise
def argv(kind,exe,fd):
    f="/proc/self/fd/"+str(fd)
    if kind.startswith("ffprobe-"):
        fmt="wav" if kind=="ffprobe-wav" else "mov"
        return [str(exe),"-v","error","-protocol_whitelist","file",
            "-f",fmt,"-show_entries",FIELDS,"-of","json","-i",f]
    if kind=="pdfinfo-pdf":return [str(exe),"-isodates",f]
    if kind=="exiftool-image":
        return [str(exe),"-j","-n","-ImageWidth","-ImageHeight",
            "-FileType","-FileSize#","--",f]
    deny("not finite operation")
def run(args):
    if not hexid(args.nonce,32) or args.kind not in OPS:deny("invalid original kind/nonce")
    kernel_guard(args.nonce)
    exe=review_binary(args.binary,args.binary_sha,args.kind)
    fd=pinned_input(args.file,args.kind,args.resource_sha)
    try:
        approved_first_launch(args.nonce)
        readable,_,_=select.select([sys.stdin.buffer],[],[],0)
        if readable and not sys.stdin.buffer.read(1):
            deny("host cancelled original resource before approved execution")
        with tempfile.TemporaryFile() as output:
            child=subprocess.Popen(argv(args.kind,exe,fd),
                stdin=subprocess.DEVNULL,stdout=output,stderr=subprocess.DEVNULL,
                pass_fds=(fd,),env={"PATH":"/usr/bin:/bin","LC_ALL":"C",
                                     "HOME":"/nonexistent","GIT_CONFIG_NOSYSTEM":"1"})
            try:
                deadline=time.monotonic()+30
                while child.poll() is None:
                    if time.monotonic()>=deadline:
                        child.kill();child.wait();deny("finite probe deadline exceeded")
                    waiting,_,_=select.select([sys.stdin.buffer],[],[],0.035)
                    if waiting and not sys.stdin.buffer.read(1):
                        child.kill();child.wait();deny("original host lifetime ended")
                    if output.tell()>OPS[args.kind][3]:
                        child.kill();child.wait();deny("bounded probe output exceeded")
                if child.returncode!=0:deny("finite reviewed probe returned error")
                if output.tell()>OPS[args.kind][3]:
                    deny("finite reviewed probe output too large")
                output.seek(0)
                result=output.read(OPS[args.kind][3]+1)
                if len(result)>OPS[args.kind][3]:deny("probe output exceeded bound")
                sys.stdout.buffer.write(result);sys.stdout.buffer.flush()
                return 0
            finally:
                if child.poll() is None:
                    child.kill();child.wait()
    finally:os.close(fd)
def main():
    ap=argparse.ArgumentParser(allow_abbrev=False)
    for x in ("nonce","kind","binary","binary_sha","file","resource_sha"):
        ap.add_argument("--"+x.replace("_","-"),required=True)
    try:return run(ap.parse_args())
    except Exception as exc:
        print("SEC06C_OS_PROBE_WORKER_BLOCKED: "+str(exc),file=sys.stderr)
        return 93
if __name__=="__main__":sys.exit(main())
