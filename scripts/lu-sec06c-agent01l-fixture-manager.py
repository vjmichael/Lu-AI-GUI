#!/usr/bin/python3
"""AGENT-01L test-only privileged adapter to the existing SEC-06C systemd manager.

Runs ONLY in lu-sec06c-fixture-<nonce>.service, a root-owned Delegate=yes
systemd unit, and imports the installed immutable SEC-06C cgroup controls.
It never grants model/desktop tool execution or accepts a command to run.
The original Python destructive test and Rust Git/restart test are unchanged.
"""
import argparse
import ctypes
import importlib.util
from importlib.machinery import SourceFileLoader
import os
import pathlib
import pwd
import re
import select
import stat
import subprocess
import sys
import time

MODULE=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-kernel-manager")
RECEIPTS=pathlib.Path("/var/lib/lu-sec06c/fixture-receipts")
CGROUP=pathlib.Path("/sys/fs/cgroup")
ID32=re.compile(r"^[a-f0-9]{32}$")
LEADER_CODE="""import subprocess
child=subprocess.Popen(['/bin/sleep','90'],start_new_session=True,
  stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
print(child.pid,flush=True)
child.wait()
"""
CHAT="c-"+"1"*32
TURN="g-"+"2"*32
CALL="agent01l-cgroup-escaped-gh"


def fail(message):
    raise RuntimeError("AGENT01L_SEC06C_ROOT_FIXTURE_DENIED: "+message)



HOST_MANIFEST=pathlib.Path("/usr/libexec/lu/lu-agent01l-fixture-host.sha256")


def kernel_starttime(pid):
    raw=pathlib.Path(f"/proc/{pid}/stat").read_text(encoding="ascii")
    if ") " not in raw:
        fail("untrusted original kernel process stat")
    fields=raw.rsplit(")",1)[1].split()
    if len(fields)<20 or not re.fullmatch(r"[0-9]{1,30}",fields[19]):
        fail("original kernel PID starttime absent")
    return fields[19]


def checked_admission(nonce,pid,uid,starttime):
    # Fixed one-use ROOT voucher was written only after the trusted launcher
    # hashed the live original binary against the installed ROOT manifest.
    # This does NOT relax systemd's restricted service capability bounding set.
    manifest=os.open(HOST_MANIFEST,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC)
    try:
        info=os.fstat(manifest)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid!=0
                or info.st_mode&0o022 or info.st_nlink!=1):
            fail("original root-owned binary manifest invalid")
        pinned=os.read(manifest,66)
        if (len(pinned)!=65 or pinned[-1:]!=bytes([10])
                or not re.fullmatch(b"[0-9a-f]{64}",pinned[:64])):
            fail("original root-owned binary digest malformed")
    finally:
        os.close(manifest)
    target=RECEIPTS/(nonce+".admission")
    fd=os.open(target,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC)
    try:
        info=os.fstat(fd)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid!=0
                or info.st_mode&0o077 or info.st_nlink!=1):
            fail("one-use root-owned admission identity invalid")
        body=os.read(fd,256)
        if os.read(fd,1):
            fail("root-owned admission record exceeded bounded size")
    finally:
        os.close(fd)
    expected=("AGENT01L_ROOT_VERIFIED_HOST_V1\n"
              f"NONCE={nonce}\nHOST_PID={pid}\nUID={uid}\n"
              f"STARTTIME={starttime}\nEXEC_SHA256={pinned[:64].decode('ascii')}\n")
    if body!=expected.encode("ascii"):
        fail("root-verified executable or original PID starttime mismatch")


def original_host(pid,uid,nonce):
    if pid<=1 or uid<=0:
        fail("invalid reviewed original PID/UID")
    status=pathlib.Path(f"/proc/{pid}/status").read_text(encoding="ascii")
    rows=[x.split() for x in status.splitlines() if x.startswith("Uid:")]
    if len(rows)!=1 or len(rows[0])!=5 or int(rows[0][1])!=uid:
        fail("original test host not owned by invoking nonroot UID")
    fd=os.pidfd_open(pid)
    try:
        checked_admission(nonce,pid,uid,kernel_starttime(pid))
        return fd
    except BaseException:
        os.close(fd)
        raise


def original_profile(raw,uid):
    p=pathlib.Path(raw)
    if not p.is_absolute() or p.is_symlink() or p.resolve(strict=True)!=p:
        fail("untrusted original test profile")
    if not p.name.startswith("lu-agent01l-cgroup-"):
        fail("not the original original test-owned temporary folder")
    # The fixture's original tempfile is a private nonroot directory,
    # never a model-selected user document path.
    m=p.lstat()
    if not stat.S_ISDIR(m.st_mode) or m.st_uid!=uid or m.st_mode&0o077:
        fail("test profile is not private to its original nonroot owner")
    return p


def read_child_number(pipe,limit=8):
    end=time.monotonic()+limit
    content=b""
    poll=select.poll()
    poll.register(pipe.fileno(),select.POLLIN|select.POLLHUP|select.POLLERR)
    while time.monotonic()<end and len(content)<=32:
        if not poll.poll(100):
            continue
        data=os.read(pipe.fileno(),33-len(content))
        if not data:
            break
        content+=data
        if b"\n" in content:
            break
    if b"\n" not in content:
        fail("original fixed worker did not identify its nested descendant")
    line,rest=content.split(b"\n",1)
    if rest or not (1<=len(line)<=10 and line.isdigit()):
        fail("malformed original fixed descendant identifier")
    return int(line)


def cgroup_of(pid):
    rows=pathlib.Path(f"/proc/{pid}/cgroup").read_text(encoding="ascii").splitlines()
    if len(rows)!=1 or not rows[0].startswith("0::/"):
        fail("missing original unified cgroup membership")
    return rows[0][3:]


def parent_of(pid):
    line=pathlib.Path(f"/proc/{pid}/stat").read_text(encoding="ascii")
    return int(line.rsplit(")",1)[1].split()[1])



def publish_original_handshake(profile,nonce,uid,result):
    # Original Python test creates a PRIVATE 0700 directory owned by the
    # nonroot runner. The systemd service intentionally has no CAP_DAC_OVERRIDE.
    # Temporarily use ONLY that exact runner FSUID for the six-line READY
    # handshake; keep EUID root and restore ROOT FSUID before waiting for
    # SIGKILL / invoking independent kernel cgroup.kill.
    if os.geteuid()!=0:
        fail("only the root-owned fixture may publish original handshake")
    if profile.is_symlink() or not profile.is_dir():
        fail("original runner's private profile changed")
    info=profile.lstat()
    if info.st_uid!=uid or info.st_mode&0o077:
        fail("original runner profile ownership changed")
    libc=ctypes.CDLL(None,use_errno=True)
    libc.setfsuid.argtypes=[ctypes.c_int]
    libc.setfsuid.restype=ctypes.c_int
    prior=libc.setfsuid(uid)
    try:
        if prior!=0 or libc.setfsuid(-1)!=uid:
            fail("original scoped FSUID admission refused")
        evidence=profile/"host-crash-evidence.txt"
        staged=profile/(".lu-agent01l-"+nonce+".stage")
        fd=os.open(staged,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o644)
        with os.fdopen(fd,"w",encoding="ascii") as out:
            out.write(result)
            out.flush()
            os.fsync(out.fileno())
        if evidence.exists() or evidence.is_symlink():
            fail("original test evidence file already occupied")
        os.rename(staged,evidence)
        folder=os.open(profile,os.O_RDONLY|os.O_DIRECTORY)
        try:
            os.fsync(folder)
        finally:
            os.close(folder)
    finally:
        prior=libc.setfsuid(0)
        if prior!=uid or libc.setfsuid(-1)!=0:
            fail("protected root FSUID not restored before kernel cleanup")


def main():
    ap=argparse.ArgumentParser(allow_abbrev=False)
    ap.add_argument("--nonce",required=True)
    ap.add_argument("--uid",type=int,required=True)
    ap.add_argument("--host-pid",type=int,required=True)
    ap.add_argument("--profile",required=True)
    args=ap.parse_args()
    if not ID32.fullmatch(args.nonce) or args.uid<=0:
        fail("invalid fixed service nonce/UID")
    if os.geteuid()!=0:
        fail("not the privileged SEC-06C service manager")
    if MODULE.is_symlink() or not MODULE.is_file():
        fail("root-owned SEC-06C manager missing")
    loader=SourceFileLoader("lu_sec06c_fixed",str(MODULE))
    spec=importlib.util.spec_from_loader(loader.name,loader)
    shared=importlib.util.module_from_spec(spec)
    loader.exec_module(shared)
    shared.checked_root_file(MODULE)
    shared.checked_root_file(pathlib.Path(__file__).resolve(strict=True))
    shared.checked_root_file(pathlib.Path("/usr/bin/python3").resolve(strict=True))
    shared.checked_root_file(pathlib.Path("/bin/sleep").resolve(strict=True))
    parent,unit=shared.first_party_unit(args.nonce,"fixture")
    for cap in (6,7):
        if not shared.process_capability_bits("CapEff")&(1<<cap):
            fail("root-owned fixture service missing effective UID/GID drop capability")
    user=pwd.getpwuid(args.uid)
    profile=original_profile(args.profile,args.uid)
    shared.checked_root_dir(RECEIPTS)
    hostfd=original_host(args.host_pid,args.uid,args.nonce)
    receipt=RECEIPTS/(args.nonce+".receipt")
    if receipt.exists() or receipt.is_symlink():
        fail("one-use fixture receipt already exists")
    leaf=parent/("lu-sec06c-leaf-"+args.nonce)
    nested=leaf/"lu-nested-host-test"
    leader=None
    host_seen=False
    verified=False
    try:
        leaf.mkdir(mode=0o755)
        nested.mkdir(mode=0o755)
        if (leaf.stat().st_uid!=0 or nested.stat().st_uid!=0 or
                leaf.stat().st_mode&0o022 or nested.stat().st_mode&0o022):
            fail("worker or same-UID attacker can modify kernel cgroup hierarchy")
        with (leaf/"cgroup.events").open(encoding="ascii") as events:
            if not shared.kernel_empty(events) or not (leaf/"cgroup.kill").is_file():
                fail("root-owned cgroup leaf is not genuinely empty and killable")
            # Reuse the original SEC-06C same-UID hostile ancestor-migration
            # probe, running with a genuine writable cgroup mount inside the
            # root-owned leaf. Permission-denied migration is mandatory.
            attacker=subprocess.Popen(
                ["/usr/bin/python3",str(MODULE),
                 "--attack-leaf","--nonce",args.nonce],
                stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,cwd="/",
                preexec_fn=lambda:shared.preexec_leaf(
                    leaf,args.uid,user.pw_gid,readonly=False),
                close_fds=True)
            if attacker.wait(timeout=10)!=0 or not shared.wait_kernel_zero(events,5):
                fail("SEC06C_ORIGINAL_SAME_UID_ANCESTOR_ATTACK_BLOCKED")
            # Both descendants are created IN the root-owned nested leaf,
            # before dropping root. A child inherits its parent's cgroup.
            # The root supervisor then migrates ONLY the known original
            # leader back to its sibling parent cgroup, never granting a
            # nonroot worker write access to ancestor cgroup.procs.
            leader=subprocess.Popen(
                ["/usr/bin/python3","-I","-S","-u","-c",LEADER_CODE],
                stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,cwd="/",
                env={"PATH":"/usr/bin:/bin","LC_ALL":"C"},
                preexec_fn=lambda:shared.preexec_leaf(nested,args.uid,user.pw_gid),
                close_fds=True)
            child=read_child_number(leader.stdout)
            if child<=1 or parent_of(child)!=leader.pid:
                fail("reported child not original descendant of fixed leader")
            if (cgroup_of(leader.pid)!="/"+str(nested.relative_to(CGROUP)) or
                    cgroup_of(child)!="/"+str(nested.relative_to(CGROUP))):
                fail("original fixed descendants not born in protected nested cgroup")
            # Root-only migration, after the leader has already spawned the
            # session-escaped child and before the host receives READY.
            (leaf/"cgroup.procs").write_text(str(leader.pid),encoding="ascii")
            if (cgroup_of(leader.pid)!="/"+str(leaf.relative_to(CGROUP)) or
                    cgroup_of(child)!="/"+str(nested.relative_to(CGROUP))):
                fail("first-party root migration did not preserve nested descendant")
            if os.getpgid(leader.pid)==os.getpgid(child):
                fail("setsid original descendant never escaped process group")
            if parent_of(child)!=leader.pid:
                fail("original genealogy changed before host evidence")
            if leader.poll() is not None:
                fail("fixed original leader exited prematurely")
            # An unprivileged worker can attempt no ancestor migration:
            # root owns parent leaf and nesting, including cgroup.procs.
            for root in (leaf,nested):
                if root.stat().st_uid!=0 or root.stat().st_mode&0o022:
                    fail("original same-UID cgroup boundary is mutable")
            result="\n".join([
                "READY",f"HOST={args.host_pid}",f"LEADER={leader.pid}",
                f"ESCAPED={child}",f"CGROUP={leaf}",
                f"RECEIPT={receipt}",""])
            # Original six-line proof is only a handoff. The unchanged test
            # independently reopens genuine kernel groups and root receipt.
            publish_original_handshake(profile,args.nonce,args.uid,result)
            hostpoll=select.poll()
            hostpoll.register(hostfd,select.POLLIN)
            end=time.monotonic()+35
            while time.monotonic()<end:
                if hostpoll.poll(100):
                    host_seen=True
                    break
                if leader.poll() is not None:
                    fail("original worker terminated before SIGKILL")
            if not host_seen:
                fail("original Lu host never reached SIGKILL")
            # The supervisor stays OUTSIDE leaf, survives host SIGKILL,
            # and invokes the original recursive kernel cgroup.kill.
            (leaf/"cgroup.kill").write_text("1",encoding="ascii")
            if not shared.wait_kernel_zero(events,8):
                fail("real cgroup.kill did not produce recursive populated zero")
            if not shared.kernel_empty(events):
                fail("native kernel populated=0 source changed")
            nested.rmdir()
            leaf.rmdir()
            verified=True
        # Store in a ROOT-owned location so the same-UID test runner cannot
        # fabricate or replace an independent positive cleanup receipt.
        # This legacy format is ONLY historical test evidence, never a
        # production SEC-06C tool grant or a completion authorization.
        shared.original_root_file(receipt,"\n".join([
            "LU_CGROUP_V2_EMPTY_V1",f"CHAT={CHAT}",f"TURN={TURN}",
            f"CALL={CALL}",f"NONCE={args.nonce}","POPULATED=0",""]))
        return 0
    finally:
        os.close(hostfd)
        if not verified:
            try:(leaf/"cgroup.kill").write_text("1",encoding="ascii")
            except OSError:pass
        if leader is not None:
            try:leader.wait(timeout=4)
            except subprocess.TimeoutExpired:
                leader.kill()
                leader.wait(timeout=4)
        if not verified:
            try:nested.rmdir()
            except OSError:pass
            try:leaf.rmdir()
            except OSError:pass


if __name__=="__main__":
    try:sys.exit(main())
    except Exception as exc:
        print("AGENT01L_SEC06C_NATIVE_FIXTURE_BLOCKED: "+str(exc),
              file=sys.stderr)
        sys.exit(91)
