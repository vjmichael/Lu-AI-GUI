#!/usr/bin/python3
"""SEC-06C trusted per-operation ROOT supervisor (fixed finite workers only).

The system manager runs this root-owned first-party supervisor as the ONLY
privileged process in a Delegate=yes root-owned service. It creates one
root-owned kernel cgroup below its own service, then starts precisely one
non-root, fixed Git-read OR media/metadata first-party worker inside the leaf.
The root manager stays OUTSIDE the leaf while watching its genuine cgroup.events.
No reviewed worker or model process can write leaf/ancestor cgroup.procs.
Missing kernel data, process death, ancestor escape, or cleanup errors block.
No arbitrary executable, arguments, filesystem destination, network or shell.
"""
import argparse
import ctypes
import hashlib
import os
import pathlib
import pwd
import re
import select
import stat
import subprocess
import sys
import time

GIT_WORKER=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-git-worker")
PROBE_WORKER=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-probe-worker")
NATIVE_WORKER=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-native-worker")
PYTHON=pathlib.Path("/usr/bin/python3").resolve(strict=True)
APPROVAL=pathlib.Path("/run/lu-sec06c")
GIT_RECEIPTS=pathlib.Path("/var/lib/lu-sec06c/receipts")
PROBE_RECEIPTS=pathlib.Path("/var/lib/lu-sec06c/probe-receipts")
NATIVE_RECEIPTS=pathlib.Path("/var/lib/lu-sec06c/native-receipts")
CGROUP=pathlib.Path("/sys/fs/cgroup")
ID=re.compile(r"^[A-Za-z0-9_-]{1,128}$")
HEX32=re.compile(r"^[a-f0-9]{32}$")
HEX64=re.compile(r"^[a-f0-9]{64}$")
OPS={
 "git": {"head","status","branch","count"},
 "probe": {"ffprobe-wav","ffprobe-mp4","pdfinfo-pdf","exiftool-image"},
 "native": {"scan-file","archive-preview","archive-extract",
            "fixed-version","fixed-artifact","app-version"},
}
MS_RDONLY=1
MS_REMOUNT=32
MS_BIND=4096
MS_REC=16384
MS_PRIVATE=1<<18
CLONE_NEWNS=0x00020000
PR_SET_NO_NEW_PRIVS=38
MAX_OUTPUT=16384

def refuse(reason):
    raise RuntimeError("SEC06C_PINNED_KERNEL_SUPERVISOR_DENIED: "+reason)

def checked_root_file(path):
    path=pathlib.Path(path)
    if not path.is_absolute() or path.is_symlink():refuse("installed program symlink")
    info=path.stat()
    if not stat.S_ISREG(info.st_mode) or info.st_uid!=0 or info.st_mode&0o022:
        refuse("installed program must be root-owned immutable")
    for item in (path.parent,path.parent.parent):
        info=item.stat()
        if info.st_uid!=0 or info.st_mode&0o022:
            refuse("installed parent directory mutable")
    return path

def checked_root_dir(path):
    path.mkdir(parents=True,exist_ok=True,mode=0o755)
    info=path.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid!=0 or info.st_mode&0o022:
        refuse("root approval or receipt directory mutable")

def first_party_unit(nonce,domain):
    if os.geteuid()!=0:refuse("only trusted systemd service may supervise kernel")
    own=pathlib.Path("/proc/self/cgroup").read_text(encoding="ascii").splitlines()
    if len(own)!=1 or not own[0].startswith("0::/"):
        refuse("non-native unified cgroup-v2 supervisor")
    group=own[0][3:]
    unit=f"lu-sec06c-{domain}-{nonce}.service"
    if not group.endswith("/"+unit):
        refuse("root supervisor not in its exact systemd-issued service")
    mount=[x for x in pathlib.Path("/proc/self/mountinfo").read_text().splitlines()
       if " - cgroup2 " in x and x.split(" - ",1)[0].split()[4]==str(CGROUP)]
    if len(mount)!=1 or "rw" not in mount[0].split(" - ",1)[0].split()[5].split(","):
        refuse("delegated kernel cgroup mount is not authentic and writable by ROOT")
    parent=(CGROUP/group.lstrip("/")).resolve(strict=True)
    if parent==CGROUP or not parent.is_relative_to(CGROUP):
        refuse("untrusted kernel service boundary")
    if parent.stat().st_uid!=0 or parent.parent.stat().st_uid!=0:
        refuse("original systemd service or ancestor is not root-owned")
    try:
        if os.getxattr(parent,"user.delegate")!=b"1":
            refuse("systemd root-owned Delegate=yes not genuine")
    except OSError as exc:
        refuse("systemd kernel delegation absent: "+type(exc).__name__)
    if not (parent/"cgroup.kill").is_file():
        refuse("real root-owned cgroup.kill absent")
    return parent,unit

def digest_file(path,expected):
    p=pathlib.Path(path)
    if not p.is_absolute() or p.is_symlink() or not HEX64.fullmatch(expected):
        refuse("binary digest/path invalid")
    actual=p.resolve(strict=True)
    if p!=actual or not actual.is_file() or actual.stat().st_uid!=0 or actual.stat().st_mode&0o022:
        refuse("unreviewed or mutable executable")
    h=hashlib.sha256()
    with actual.open("rb") as f:
        for chunk in iter(lambda:f.read(65536),b""):h.update(chunk)
    if h.hexdigest()!=expected:refuse("reviewed executable SHA changed")
    return actual

def input_resource(path,expected,op):
    p=pathlib.Path(path)
    if not p.is_absolute() or p.is_symlink() or not HEX64.fullmatch(expected):
        refuse("resource path or SHA not canonical")
    if p.resolve(strict=True)!=p or not p.is_file():
        refuse("resource has escaped reviewed path")
    ext=p.suffix.lower().lstrip(".")
    allowed={"ffprobe-wav":("wav",),"ffprobe-mp4":("mp4",),
             "pdfinfo-pdf":("pdf",),"exiftool-image":("png","jpg","jpeg")}
    size=p.stat().st_size
    ceiling=256*1024*1024 if op.startswith("ffprobe-") else 32*1024*1024
    if ext not in allowed[op] or not 0<size<=ceiling:
        refuse("resource type or size forbidden")
    # Root never opens user file bytes. The nonroot worker reopens O_NOFOLLOW
    # and verifies the original host resource digest before any probe exec.
    return p

def preexec_leaf(leaf,uid,gid,readonly=True):
    """Called only in forked child, still root; no untrusted exec yet."""
    # A diagnostic is ONLY the failing kernel syscall phase/errno. It cannot
    # authorize execution, attest cleanup or expose arbitrary model bytes.
    stage="cgroup-migrate"
    try:
        # Move into root-owned kernel leaf before dropping any privilege.
        with (leaf/"cgroup.procs").open("w",encoding="ascii") as f:
            f.write(str(os.getpid()))
        libc=ctypes.CDLL(None,use_errno=True)
        if readonly:
            stage="unshare-private-mount"
            if libc.unshare(CLONE_NEWNS)!=0:
                raise OSError(ctypes.get_errno(),"mount namespace isolation")
            stage="private-mount-propagation"
            if libc.mount(None,b"/",None,MS_REC|MS_PRIVATE,None)!=0:
                raise OSError(ctypes.get_errno(),"private mount propagation")
            stage="readonly-kernel-cgroup-mount"
            if libc.mount(None,b"/sys/fs/cgroup",None,
                          MS_REMOUNT|MS_BIND|MS_RDONLY,None)!=0:
                raise OSError(ctypes.get_errno(),"worker cgroup mount readonly")
        # The hostile-migration probe intentionally retains a genuine rw
        # cgroup mount. It still drops root and never receives cgroup rights.
        stage="prctl-no-new-privileges"
        if libc.prctl(PR_SET_NO_NEW_PRIVS,1,0,0,0)!=0:
            raise OSError(ctypes.get_errno(),"no new privileges")
        stage="clear-root-supplementary-groups"
        os.setgroups([])
        stage="drop-group-identity"
        os.setgid(gid)
        stage="drop-user-identity"
        os.setuid(uid)
    except OSError as exc:
        number=exc.errno if isinstance(exc.errno,int) else -1
        # The original native failure is setuid EPERM after setgid. Observe
        # only fixed kernel capability masks and numeric original IDs; this
        # is diagnostic, not permission or proof. Never dump environment,
        # paths, model content or user data into process logs.
        masks={}
        try:
            for row in pathlib.Path("/proc/self/status").read_text().splitlines():
                for key in ("CapEff","CapPrm","CapBnd"):
                    if row.startswith(key+":"):
                        masks[key]=row.split(":",1)[1].strip()
        except OSError:pass
        mask=" ".join(key+"="+masks.get(key,"unavailable")
            for key in ("CapEff","CapPrm","CapBnd"))
        msg=(f"SEC06C_KERNEL_PREEXEC_DENIED stage={stage} errno={number} "
             f"euid={os.geteuid()} egid={os.getegid()} {mask}"+chr(10))
        os.write(2,msg.encode("ascii"))
        raise

def kernel_empty(stream):
    try:
        stream.seek(0)
        records=stream.read().splitlines()
        return [x for x in records if x.startswith("populated ")]==["populated 0"]
    except OSError:return False

def wait_kernel_zero(events,timeout):
    until=time.monotonic()+timeout
    while time.monotonic()<until:
        if kernel_empty(events):return True
        time.sleep(.025)
    return False

def original_root_file(path,body):
    flags=os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW
    fd=os.open(path,flags,0o644)
    with os.fdopen(fd,"w",encoding="ascii") as file:
        file.write(body);file.flush();os.fsync(file.fileno())
    folder=os.open(path.parent,os.O_RDONLY|os.O_DIRECTORY)
    try:os.fsync(folder)
    finally:os.close(folder)

def finite_native(kind,arguments):
    def name(x):
        return 0<len(x)<=128 and not x.startswith(".") and ".." not in x and all(
            c.isascii() and (c.isalnum() or c in "._-") for c in x)
    def absolute(raw,dir=False):
        p=pathlib.Path(raw)
        return p.is_absolute() and not p.is_symlink() and p.resolve(strict=True)==p and (
            p.is_dir() if dir else p.is_file())
    def new_target(raw):
        p=pathlib.Path(raw)
        return p.is_absolute() and absolute(str(p.parent),True) and name(p.name) and not p.exists()
    if kind=="scan-file":
        return len(arguments)==2 and arguments[0]=="--lu-file-scan-worker" and absolute(arguments[1])
    if kind=="archive-preview":
        return len(arguments)==6 and arguments[0]=="--lu-archive-preview-worker" and (
            absolute(arguments[1],True) and name(arguments[2])
            and new_target(arguments[3]) and HEX64.fullmatch(arguments[4]+arguments[4])
            and arguments[5].isdigit() and int(arguments[5])>1)
    if kind=="archive-extract":
        return len(arguments)==9 and arguments[0]=="--lu-archive-extract-worker" and (
            absolute(arguments[1],True) and name(arguments[2])
            and absolute(arguments[3],True) and name(arguments[4])
            and HEX64.fullmatch(arguments[5]) and
            HEX64.fullmatch(arguments[6]+arguments[6]) and
            HEX64.fullmatch(arguments[7]+arguments[7])
            and arguments[8].isdigit() and int(arguments[8])>1)
    if kind=="fixed-version":return arguments==["--lu-e1-version-one-shot"]
    if kind=="fixed-artifact":return (len(arguments)==2 and
        arguments[0]=="--lu-e1-artifact-one-shot" and arguments[1] in ("docx","xlsx"))
    if kind=="app-version":return arguments in (["--version"],["-version"])
    return False

def one_use_worker_args(args):
    if args.domain=="native":
        if (args.op not in OPS["native"] or not args.binary or not args.binary_sha
            or not args.args_sha or not HEX64.fullmatch(args.args_sha)
            or len(args.arg)>12 or any(len(x)>4096 or "\0" in x or
                any(ord(c)<32 for c in x) for x in args.arg)):
            refuse("reviewed native finite source contract absent")
        reviewed=digest_file(args.binary,args.binary_sha)
        digest=hashlib.sha256(("\0".join([str(reviewed)]+args.arg)+"\0")
                              .encode("utf-8")).hexdigest()
        if digest!=args.args_sha or not finite_native(args.op,args.arg):
            refuse("native reviewed argument digest or finite kind changed")
        return [str(PYTHON),str(NATIVE_WORKER),"--nonce",args.nonce,
                "--kind",args.op,"--binary",str(reviewed),
                "--binary-sha",args.binary_sha,"--args-sha",args.args_sha,
                *["--arg="+arg for arg in args.arg]]
    if args.domain=="git":
        if args.op not in OPS["git"] or not args.git or not args.sha or not args.repo:
            refuse("finite Git review not supplied")
        git=digest_file(args.git,args.sha)
        if git.name!="git":refuse("Git executable changed")
        repo=pathlib.Path(args.repo)
        if not repo.is_absolute() or repo.is_symlink() or repo.resolve(strict=True)!=repo:
            refuse("reviewed Git repository not canonical")
        if not (repo/".git").is_dir():refuse("reviewed Git repository not enrolled")
        return [str(PYTHON),str(GIT_WORKER),"--nonce",args.nonce,
                "--git",str(git),"--sha",args.sha,
                "--repo",str(repo),"--op",args.op]
    if args.op not in OPS["probe"] or not args.binary or not args.binary_sha:
        refuse("finite media/metadata review not supplied")
    exe=digest_file(args.binary,args.binary_sha)
    expected={"ffprobe-wav":"ffprobe","ffprobe-mp4":"ffprobe",
              "pdfinfo-pdf":"pdfinfo","exiftool-image":"exiftool"}[args.op]
    if exe.name!=expected:refuse("unreviewed media executable")
    if not args.file or not args.resource_sha:
        refuse("missing original resource SHA")
    resource=input_resource(args.file,args.resource_sha,args.op)
    return [str(PYTHON),str(PROBE_WORKER),"--nonce",args.nonce,
            "--kind",args.op,"--binary",str(exe),
            "--binary-sha",args.binary_sha,"--file",str(resource),
            "--resource-sha",args.resource_sha]

def negative_original_ancestor_probe(nonce):
    """Nonroot same-UID attacker in the actual worker cgroup, not a mock."""
    if os.geteuid()==0 or not HEX32.fullmatch(nonce):
        refuse("adversarial worker must be real nonroot with fixed nonce")
    own=pathlib.Path("/proc/self/cgroup").read_text().strip()
    suffix="/lu-sec06c-leaf-"+nonce
    if not own.startswith("0::/") or not own.endswith(suffix):
        refuse("attacker not inside exact original worker leaf")
    group=(CGROUP/own[3:].lstrip("/")).resolve(strict=True)
    parent=group.parent
    if parent.stat().st_uid!=0 or group.stat().st_uid!=0:
        refuse("attacker inherited an unsafe writable ancestor")
    mount=[x for x in pathlib.Path("/proc/self/mountinfo").read_text().splitlines()
        if " - cgroup2 " in x and x.split(" - ",1)[0].split()[4]==str(CGROUP)]
    if len(mount)!=1 or "rw" not in mount[0].split(" - ",1)[0].split()[5].split(","):
        refuse("same-UID hostile migration must use REAL rw cgroup mount")
    # The same hostile action as original SEC-05: a setsid descendant tries
    # to move to ancestor cgroup.procs, outside its original worker leaf.
    child=subprocess.Popen(["/bin/sleep","3"],start_new_session=True,
         stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
         stderr=subprocess.DEVNULL)
    try:
        victim=pathlib.Path(f"/proc/{child.pid}/cgroup").read_text().strip()
        if victim!=own:refuse("setsid original descendant not in worker leaf")
        denied=False
        try:
            (parent/"cgroup.procs").write_text(str(child.pid),encoding="ascii")
        except PermissionError:
            denied=True
        if not denied:
            refuse("SEC06C_ANCESTOR_ESCAPE_SUCCEEDED")
        after=pathlib.Path(f"/proc/{child.pid}/cgroup").read_text().strip()
        if after!=own:refuse("SEC06C_ANCESTOR_ESCAPE_SUCCEEDED")
        return 0
    finally:
        child.kill()
        child.wait(timeout=5)

def run(args):
    if not HEX32.fullmatch(args.nonce) or args.domain not in OPS:
        refuse("invalid one-use OS domain/nonce")
    if not args.chat.startswith("c-") or not args.turn.startswith("g-"):
        refuse("not an original reviewed human turn")
    if len(args.chat)!=34 or len(args.turn)!=34 or not ID.fullmatch(args.call):
        refuse("original user/call identity invalid")
    if not HEX32.fullmatch(args.chat[2:]) or not HEX32.fullmatch(args.turn[2:]):
        refuse("original chat/turn hashes invalid")
    if args.uid<=0 or pwd.getpwuid(args.uid).pw_uid!=args.uid:
        refuse("first-party worker must run as original nonroot UID")
    checked_root_file(pathlib.Path(__file__).resolve())
    checked_root_file(PYTHON)
    checked_root_file(
        GIT_WORKER if args.domain=="git" else
        PROBE_WORKER if args.domain=="probe" else NATIVE_WORKER)
    parent,unit=first_party_unit(args.nonce,args.domain)
    argv=one_use_worker_args(args)
    checked_root_dir(APPROVAL)
    receipts=(
        GIT_RECEIPTS if args.domain=="git" else
        PROBE_RECEIPTS if args.domain=="probe" else NATIVE_RECEIPTS)
    checked_root_dir(receipts)
    receipt=receipts/(args.nonce+".receipt")
    go=APPROVAL/(args.nonce+".go")
    if receipt.exists() or go.exists():
        refuse("one-use reviewed nonce replay")
    leaf=parent/("lu-sec06c-leaf-"+args.nonce)
    leaf.mkdir(mode=0o755)
    if leaf.stat().st_uid!=0 or leaf.stat().st_mode&0o022:
        refuse("root-owned worker leaf unexpectedly delegated")
    events=(leaf/"cgroup.events").open("r",encoding="ascii")
    if not kernel_empty(events) or not (leaf/"cgroup.kill").is_file():
        refuse("kernel child group not initially empty")
    user=pwd.getpwuid(args.uid)
    # Mandatory real same-UID hostile-ancestor attack BEFORE approving any
    # reviewed tool. Root-owned parent is the boundary; any escape blocks.
    attacker=subprocess.Popen([
        str(PYTHON),str(pathlib.Path(__file__).resolve()),
        "--attack-leaf","--nonce",args.nonce
    ],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
       stderr=None,
       preexec_fn=lambda:preexec_leaf(leaf,args.uid,user.pw_gid,readonly=False))
    if attacker.wait(timeout=10)!=0 or not wait_kernel_zero(events,5):
        try:(leaf/"cgroup.kill").write_text("1",encoding="ascii")
        except OSError:pass
        refuse("SEC06C_ORIGINAL_SAME_UID_ANCESTOR_ATTACK_BLOCKED")
    child=None
    kernel_verified=False
    exit_status=93
    output=b""
    native_output=bytearray()
    native_limit=(
        16*1024*1024 if args.domain=="native" and args.op=="fixed-artifact"
        else 128*1024 if args.domain=="native" and args.op=="scan-file"
        else 32*1024 if args.domain=="native"
        and args.op in ("fixed-version","app-version") else 0)
    try:
        # All privileged namespace/cgroup operations happen before drop.
        child=subprocess.Popen(argv,stdin=sys.stdin.buffer,
             stdout=subprocess.PIPE,stderr=None,
             cwd="/",env={"PATH":"/usr/bin:/bin","LC_ALL":"C",
                          "HOME":user.pw_dir,"GIT_CONFIG_NOSYSTEM":"1"},
             preexec_fn=lambda:preexec_leaf(leaf,args.uid,user.pw_gid),
             close_fds=True)
        if child.pid<=1 or pathlib.Path(f"/proc/{child.pid}/cgroup").read_text().strip()!=(
            "0::/"+str(leaf.relative_to(CGROUP))
        ):
            refuse("first-worker kernel membership not pinned before GO")
        original_pidfd=os.pidfd_open(child.pid)
        try:
            original_root_file(go,"LU_SEC06C_GO_V1\n"+args.nonce+"\n")
            deadline=time.monotonic()+(620 if args.domain=="native" else 70)
            while time.monotonic()<deadline:
                readable,_,_=select.select([sys.stdin.buffer],[],[],0)
                if readable and not sys.stdin.buffer.read(1):
                    exit_status=93
                    break
                if args.domain=="native":
                    # Drain original bounded progress/captured output BEFORE
                    # worker exit, preventing a full pipe from deadlocking
                    # the host while the root kernel supervisor runs.
                    ready,_,_=select.select([child.stdout],[],[],0)
                    if ready:
                        chunk=os.read(child.stdout.fileno(),65536)
                        if chunk:
                            native_output.extend(chunk)
                            if len(native_output)>native_limit:
                                exit_status=93
                                break
                poll=child.poll()
                if poll is not None:
                    exit_status=poll
                    break
                time.sleep(.035)
            # Any timeout, cancellation, unexpected exit or child survivor
            # kills ONLY this original systemd-root-owned exact leaf.
            if child.poll() is None or exit_status!=0:
                (leaf/"cgroup.kill").write_text("1",encoding="ascii")
            if not wait_kernel_zero(events,15):
                (leaf/"cgroup.kill").write_text("1",encoding="ascii")
                exit_status=93
                if not wait_kernel_zero(events,15):
                    refuse("genuine root-owned leaf kernel populated-zero unavailable")
            if child.poll() is None:
                child.wait(timeout=10)
            kernel_verified=True
            if exit_status==0:
                if args.domain=="native":
                    tail=child.stdout.read(native_limit+1-len(native_output))
                    native_output.extend(tail)
                    if len(native_output)>native_limit:
                        refuse("finite native worker exceeded reviewed stdout bound")
                    output=bytes(native_output)
                else:
                    output=child.stdout.read(MAX_OUTPUT+1)
                    if len(output)>MAX_OUTPUT:
                        refuse("bounded worker stdout violated")
            else:output=b""
        finally:
            os.close(original_pidfd)
    finally:
        if not kernel_verified:
            try:(leaf/"cgroup.kill").write_text("1",encoding="ascii")
            except OSError:pass
        if child is not None:
            try:child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                child.kill();child.wait(timeout=5)
        if go.exists():go.unlink()
        events.close()
        if kernel_verified:leaf.rmdir()
    if not kernel_verified:refuse("kernel leaf was never verified empty")
    if args.domain=="native":
        evidence="\n".join([
            "LU_SEC06C_OWNED_NATIVE_V1",
            "NONCE="+args.nonce,"CHAT="+args.chat,"TURN="+args.turn,
            "CALL="+args.call,"KIND="+args.op,
            "BINARY_SHA256="+args.binary_sha,
            "ARGV_SHA256="+args.args_sha,
            "UNIT="+unit,"ORIGINAL_UID="+str(args.uid),
            "KERNEL_EMPTY=1",
            "EFFECT="+("COMPLETED" if exit_status==0 else "INTERRUPTED"),""])
    elif args.domain=="git":
        evidence="\n".join([
            "LU_SEC06C_OWNED_GIT_V1",
            "NONCE="+args.nonce,"CHAT="+args.chat,"TURN="+args.turn,
            "CALL="+args.call,"GIT_SHA256="+args.sha,
            "UNIT="+unit,"ORIGINAL_UID="+str(args.uid),
            "KERNEL_EMPTY=1",
            "EFFECT="+("COMPLETED" if exit_status==0 else "INTERRUPTED"),""])
    else:
        evidence="\n".join([
            "LU_SEC06C_OWNED_PROBE_V1",
            "NONCE="+args.nonce,"CHAT="+args.chat,"TURN="+args.turn,
            "CALL="+args.call,"KIND="+args.op,
            "BINARY_SHA256="+args.binary_sha,
            "RESOURCE_SHA256="+args.resource_sha,
            "UNIT="+unit,"ORIGINAL_UID="+str(args.uid),
            "KERNEL_EMPTY=1",
            "EFFECT="+("COMPLETED" if exit_status==0 else "INTERRUPTED"),""])
    original_root_file(receipt,evidence)
    if exit_status==0:
        sys.stdout.buffer.write(output);sys.stdout.buffer.flush()
    return 0 if exit_status==0 else 73

def main():
    if "--attack-leaf" in sys.argv:
        if (len(sys.argv)!=4 or sys.argv[1]!="--attack-leaf"
            or sys.argv[2]!="--nonce"):
            print("SEC06C_ATTACK_ADMISSION_DENIED",file=sys.stderr)
            return 93
        try:return negative_original_ancestor_probe(sys.argv[3])
        except Exception as e:
            print("SEC06C_ANCESTOR_ATTACK_BLOCKED: "+str(e),file=sys.stderr)
            return 93
    ap=argparse.ArgumentParser(allow_abbrev=False)
    for name in ("nonce","domain","chat","turn","call","uid"):
        ap.add_argument("--"+name,required=True)
    for name in ("git","sha","repo","op","binary","binary_sha",
                 "file","resource_sha","args_sha"):
        ap.add_argument("--"+name.replace("_","-"))
    ap.add_argument("--arg",action="append",default=[])
    args=ap.parse_args()
    try:
        args.uid=int(args.uid)
        return run(args)
    except Exception as e:
        print("SEC06C_ROOT_KERNEL_MANAGER_BLOCKED: "+str(e),file=sys.stderr)
        return 91
if __name__=="__main__":sys.exit(main())
