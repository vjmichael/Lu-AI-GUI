#!/usr/bin/env python3
"""SEC-02 destructive native Linux delegated-cgroup negative verifier.

No sudo; only a disposable first-party test host/guardian and explicit pidfds.
No fake receipts: missing cgroup delegation is NOT_AVAILABLE, not a PASS.
"""
import argparse
import os
import pathlib
import select
import signal
import subprocess
import sys
import tempfile
import time

def running(pid):
    try:
        value=pathlib.Path(f"/proc/{pid}/stat").read_text().rsplit(")",1)[1].split()[0]
        return value not in ("Z","X","x")
    except FileNotFoundError:
        return False

def terminated(fd):
    p=select.poll()
    p.register(fd,select.POLLIN)
    return bool(p.poll(0))

def wait_file(path,timeout,process=None):
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
        if path.exists():
            return path.read_text(encoding="ascii")
        if process is not None and process.poll() is not None:
            raise AssertionError(f"test host exited before {path.name}: {process.returncode}")
        time.sleep(.02)
    raise AssertionError(f"test host timed out without {path.name}")

def bounded_empty(cgroup,deadline):
    while time.monotonic()<deadline:
        if cgroup.joinpath("cgroup.events").exists():
            rows=cgroup.joinpath("cgroup.events").read_text().splitlines()
            if "populated 0" in rows:
                return True
        time.sleep(.03)
    return False

def check_evidence(raw,mode,host):
    lines=raw.splitlines()
    if len(lines)!=8 or lines[0]!="READY":
        raise AssertionError("not a complete original native fixture identity")
    fields={}
    for line in lines[1:]:
        k,sep,v=line.partition("=")
        if not sep or k in fields:
            raise AssertionError("malformed identity row")
        fields[k]=v
    if set(fields)!={"MODE","HOST","GUARDIAN","LEADER","ESCAPED","CGROUP","RECEIPT"}:
        raise AssertionError("unknown fixture fields")
    if fields["MODE"]!=mode or int(fields["HOST"])!=host.pid:
        raise AssertionError("wrong test host/mode")
    for key in ("GUARDIAN","LEADER","ESCAPED"):
        fields[key]=int(fields[key])
        if fields[key]<=1 or not running(fields[key]):
            raise AssertionError(f"missing live original {key}")
    if len({host.pid,fields["GUARDIAN"],fields["LEADER"],fields["ESCAPED"]})!=4:
        raise AssertionError("reused process identity")
    cgroup=pathlib.Path(fields["CGROUP"])
    if not cgroup.is_dir() or not cgroup.name.startswith("lu-review-"):
        raise AssertionError("unowned or missing private cgroup")
    if not cgroup.as_posix().startswith("/sys/fs/cgroup/"):
        raise AssertionError("cgroup escaped unified mount")
    original=pathlib.Path(f"/proc/{fields['LEADER']}/cgroup").read_text().strip()
    escaped=pathlib.Path(f"/proc/{fields['ESCAPED']}/cgroup").read_text().strip()
    if not original.endswith("/"+cgroup.name) or (
        escaped!=original+"/p0-sec02-nested"
    ):
        raise AssertionError("leader or setsid descendant not in nested trusted cgroup")
    guard_group=pathlib.Path(f"/proc/{fields['GUARDIAN']}/cgroup").read_text().strip()
    if guard_group==original or guard_group.startswith(original+"/"):
        raise AssertionError("guardian must be outside killable cgroup")
    if not cgroup.joinpath("cgroup.kill").exists():
        raise AssertionError("not a real kernel cgroup.kill")
    receipt=pathlib.Path(fields["RECEIPT"])
    if receipt.exists():
        raise AssertionError("stale proof before first-party cancellation")
    fields["cgroup"]=cgroup
    fields["receipt"]=receipt
    return fields

def execute(mode,fixture):
    with tempfile.TemporaryDirectory(prefix="lu-p0-sec02-") as temp:
        root=pathlib.Path(temp)
        evidence=root/"native-evidence"
        outcome=root/"native-outcome"
        host=subprocess.Popen(
            [str(fixture),mode,str(root),str(evidence),str(outcome)],
            stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,env=dict(os.environ),
        )
        target=None
        fds=[]
        control=None
        migrated=None
        escaped_fd=None
        try:
            raw=wait_file(evidence,15,host)
            fields=check_evidence(raw,mode,host)
            target=fields["cgroup"]
            guard=fields["GUARDIAN"]
            escaped=fields["ESCAPED"]
            guard_fd=os.pidfd_open(guard)
            escaped_fd=os.pidfd_open(escaped)
            fds=[guard_fd,escaped_fd]
            control=subprocess.Popen(
                ["/bin/sleep","25"],stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            if not running(control.pid):
                raise AssertionError("unrelated process never started")
            if mode=="guardian-death":
                signal.pidfd_send_signal(guard_fd,signal.SIGKILL)
            elif mode=="ancestor-migration":
                # Attack only the original, pidfd-identified first-party
                # setsid descendant. Never choose an unrelated PID or cgroup.
                # Equal-UID write into a delegated ANCESTOR exposes whether
                # leaf-only cgroup.kill can mint a misleading empty receipt.
                migration_target=target.parent/"cgroup.procs"
                try:
                    migration_target.write_text(str(escaped),encoding="ascii")
                except OSError:
                    migrated=False
                else:
                    deadline=time.monotonic()+2
                    original=pathlib.Path(
                        f"/proc/{fields['LEADER']}/cgroup").read_text().strip()
                    while time.monotonic()<deadline:
                        group=pathlib.Path(f"/proc/{escaped}/cgroup").read_text().strip()
                        if group!=original+"/p0-sec02-nested":
                            if group!=original.rsplit("/",1)[0]:
                                raise AssertionError(
                                    "adversarial child migrated to unexpected cgroup")
                            migrated=True
                            break
                        time.sleep(.02)
                    else:
                        raise AssertionError("kernel acknowledged migration write "
                                             "without moving exact descendant")
            elif mode=="guardian-stall":
                signal.pidfd_send_signal(guard_fd,signal.SIGSTOP)
                deadline=time.monotonic()+2
                while time.monotonic()<deadline:
                    try:
                        state=pathlib.Path(f"/proc/{guard}/stat").read_text().rsplit(")",1)[1].split()[0]
                    except FileNotFoundError:
                        break
                    if state in ("T","t"):
                        break
                    time.sleep(.015)
                else:
                    raise AssertionError("guardian was not actually stopped")
            trigger_status=("MIGRATION_GRANTED" if migrated
                            else "MIGRATION_DENIED") if mode=="ancestor-migration" \
                           else "HOST_VERIFIER_TRIGGER"
            (root/"p0-sec02-trigger").write_text(trigger_status+"\n")
            result=wait_file(outcome,22,host).strip()
            if mode=="guardian-death":
                if result!="GUARDIAN_LOST_UNVERIFIED":
                    raise AssertionError("premature guardian death classified as success")
            elif mode=="guardian-stall":
                if result!="GUARDIAN_STALLED_UNVERIFIED":
                    raise AssertionError("stalled guardian incorrectly acknowledged cleanup")
            elif mode=="ancestor-migration":
                expected=("MIGRATION_ESCAPE_UNVERIFIED" if migrated
                          else "MIGRATION_DENIED_NATIVE_CGROUP_EMPTY")
                if result!=expected:
                    raise AssertionError(
                        f"SEC05 MIGRATION SECURITY FAILURE: observed={result}; "
                        f"expected={expected}. A migrated survivor MUST NEVER "
                        "produce a positive Linux cgroup empty witness.")
            else:
                if result!="WORKER_STALL_NATIVE_CGROUP_EMPTY":
                    raise AssertionError("stalled worker not terminated by live guardian")
            host.wait(timeout=5)
            if host.returncode!=0:
                raise AssertionError(f"native fixture exited {host.returncode}")
            receipt_expected=(mode=="worker-stall" or
                              (mode=="ancestor-migration" and not migrated))
            if fields["receipt"].exists()!=receipt_expected:
                raise AssertionError("guardian loss or migrated descendant "
                                     "minted a false positive kernel witness")
            if mode=="ancestor-migration":
                if migrated:
                    if terminated(escaped_fd) or not running(escaped):
                        raise AssertionError("migration attack did not retain "
                                             "its original surviving descendant")
                    migrated_group=pathlib.Path(f"/proc/{escaped}/cgroup").read_text().strip()
                    if migrated_group==pathlib.Path(
                        f"/proc/{fields['LEADER']}/cgroup").read_text().strip():
                        raise AssertionError("migrated process rejoined killable subtree")
                else:
                    if not terminated(escaped_fd):
                        deadline=time.monotonic()+2
                        while time.monotonic()<deadline and not terminated(escaped_fd):
                            time.sleep(.025)
                    if not terminated(escaped_fd) or target.exists():
                        raise AssertionError("denied migration was not followed "
                                             "by proper cgroup cleanup")
            elif mode=="worker-stall":
                if not terminated(escaped_fd):
                    deadline=time.monotonic()+2
                    while time.monotonic()<deadline and not terminated(escaped_fd):
                        time.sleep(.025)
                if not terminated(escaped_fd) or target.exists():
                    raise AssertionError("stalled worker descendant survived verified cgroup kill")
                record=fields["receipt"].read_text(encoding="ascii")
                if "LU_CGROUP_V2_EMPTY_V1\n" not in record or "\nPOPULATED=0\n" not in record:
                    raise AssertionError("missing original kernel cgroup receipt")
            else:
                # This is the crucial negative proof: a dead/stopped guardian
                # does NOT guarantee full descendant termination.
                if not running(escaped):
                    raise AssertionError("negative fixture never exposed an orphan risk")
                if not target.exists() or not target.joinpath("p0-sec02-nested").exists():
                    raise AssertionError("orphan cgroup unexpectedly empty/removed")
            if not running(control.pid):
                raise AssertionError("unrelated control process was terminated")
            print(
                f"P0_SEC05_NATIVE_{mode.upper().replace('-','_')}_PASS=1 "
                if mode=="ancestor-migration" else
                f"P0_SEC02_NATIVE_{mode.upper().replace('-','_')}_PASS=1 ",
                f"real_cgroup_v2=1 original_pidfd=1 setsid_descendant=1 "
                f"ancestor_migration={migrated if mode=='ancestor-migration' else 'NOT_TESTED'} "
                f"positive_custody={int(receipt_expected)} unrelated_survived=1",
                flush=True)
        finally:
            # A migrated descendant is outside the original kernel leaf.
            # For a red test, explicitly SIGKILL only its original pidfd;
            # never leave a survivor behind merely because the attack exposed
            # a false positive cgroup receipt.
            if mode=="ancestor-migration" and escaped_fd is not None:
                try:
                    if not terminated(escaped_fd):
                        signal.pidfd_send_signal(escaped_fd,signal.SIGKILL)
                except ProcessLookupError:
                    pass
            # Explicit test-only recovery of the exact validated cgroup:
            # never mistake a guardian-death negative for safe production
            # cleanup. The verifier, not Lu, cleans up the expected survivor.
            if target is not None and target.is_dir():
                try:
                    target.joinpath("cgroup.kill").write_text("1")
                    if bounded_empty(target,time.monotonic()+5):
                        nested=target/"p0-sec02-nested"
                        if nested.exists():
                            nested.rmdir()
                        target.rmdir()
                except (OSError,ValueError):
                    pass
            if host.poll() is None:
                host.kill()
            try:
                host.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
            for fd in fds:
                os.close(fd)
            if control is not None:
                if control.poll() is None:
                    control.terminate()
                control.wait(timeout=5)

def main():
    if sys.platform!="linux":
        raise RuntimeError("native Linux-only kernel acceptance")
    ap=argparse.ArgumentParser()
    ap.add_argument("--fixture",required=True)
    ap.add_argument("--mode",choices=["guardian-death","guardian-stall","worker-stall","ancestor-migration"],required=True)
    args=ap.parse_args()
    fixture=pathlib.Path(args.fixture).resolve(strict=True)
    if not os.access(fixture,os.X_OK):
        raise RuntimeError("test fixture is not executable")
    execute(args.mode,fixture)

if __name__=="__main__":
    main()
