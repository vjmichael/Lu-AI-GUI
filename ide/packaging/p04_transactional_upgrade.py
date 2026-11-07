#!/usr/bin/env python3
"""P04 filesystem transaction/recovery engine for disposable IDE payloads.

A real same-volume rename + fsync journal, independently crash-injectable.
This is the REQUIRED upgrade engine core, not yet the NSIS/dpkg/SCM adapter:
it operates ONLY inside a marked temp fixture and cannot authorize a
production install, control services or silently replay workers.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile

MARKER="lu-ide-p04-isolated-transaction-v1\n"
SCHEMA="lu.ide.p04.transaction.v1"
VALID_VERSION=re.compile(r"^[a-zA-Z0-9._-]{1,60}$")
VALID_HASH=re.compile(r"^[0-9a-f]{64}$")
FAULTS={"none","after_prepare","after_backup_rename",
        "after_activation_rename","after_validation","after_commit"}
ROLLBACK_FAULTS={"after_rollback_journal","after_reject_rename","after_restore_previous"}

class Denied(ValueError):
    pass

def hash_bytes(path):
    h=hashlib.sha256()
    with path.open("rb") as stream:
        for piece in iter(lambda:stream.read(1024*1024),b""):
            h.update(piece)
    return h.hexdigest()

def fsync_dir(root):
    if os.name != "nt":
        handle=os.open(root,os.O_RDONLY|os.O_DIRECTORY)
        try: os.fsync(handle)
        finally: os.close(handle)

def assert_fixture(root):
    # MUST remain in a disposable fixture, never Program Files or /usr.
    if root.is_symlink() or not root.is_dir():
        raise Denied("untrusted_transaction_root")
    resolved=root.resolve(strict=True)
    temp=Path(tempfile.gettempdir()).resolve(strict=True)
    if not resolved.is_relative_to(temp) or resolved == temp:
        raise Denied("production_install_roots_explicitly_prohibited")
    if (resolved/".p04-fixture").is_symlink() or not (resolved/".p04-fixture").is_file():
        raise Denied("missing_explicit_disposable_root_marker")
    if (resolved/".p04-fixture").read_text(encoding="utf-8") != MARKER:
        raise Denied("wrong_disposable_root_marker")
    for name in ["current","incoming","previous","rejected","user-state","journal.json",
                 "workers.json"]:
        node=resolved/name
        if node.is_symlink():
            raise Denied("transaction_path_symlink")
    return resolved

def identity(root,slot):
    folder=root/slot
    if folder.is_symlink() or not folder.is_dir():
        raise Denied("version_directory_missing_or_linked:"+slot)
    children=sorted(x.name for x in folder.iterdir())
    if children!=["app.bin","version.json"]:
        raise Denied("unexpected_version_directory_contents:"+slot)
    binary=folder/"app.bin"
    manifest=folder/"version.json"
    if binary.is_symlink() or manifest.is_symlink() or binary.stat().st_size>16*1024*1024:
        raise Denied("unreviewed_version_payload")
    try:
        data=json.loads(manifest.read_text(encoding="utf-8"))
    except (ValueError,OSError,UnicodeDecodeError) as exc:
        raise Denied("invalid_version_receipt") from exc
    if data.get("schema")!="lu.ide.p04.fixture-version.v1":
        raise Denied("wrong_version_schema")
    ver,digest=data.get("version"),data.get("sha256")
    if (not isinstance(ver,str) or not VALID_VERSION.fullmatch(ver)
            or not isinstance(digest,str) or not VALID_HASH.fullmatch(digest)):
        raise Denied("invalid_version_identity_or_digest")
    if hash_bytes(binary)!=digest:
        raise Denied("version_binary_digest_changed")
    return {"version":ver,"sha256":digest}

def atomic_journal(root,rec):
    # Persist intent before effect; journal always names fixed locations only.
    path=root/"journal.json"
    if path.is_symlink() or (root/".journal.tmp").exists():
        raise Denied("untrusted_journal_file")
    tmp=root/".journal.tmp"
    try:
        with tmp.open("x",encoding="utf-8") as stream:
            json.dump(rec,stream,sort_keys=True,separators=(",",":"))
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(tmp,path)
        fsync_dir(root)
    finally:
        if tmp.exists():
            tmp.unlink()

def read_journal(root):
    file=root/"journal.json"
    if not file.is_file() or file.is_symlink() or file.stat().st_size>8192:
        raise Denied("missing_or_invalid_transaction_journal")
    try:
        rec=json.loads(file.read_text(encoding="utf-8"))
    except (ValueError,OSError) as exc:
        raise Denied("invalid_transaction_journal") from exc
    if not isinstance(rec,dict):
        raise Denied("journal_must_be_an_object")
    if (rec.get("schema")!=SCHEMA or rec.get("state") not in
            {"prepared","backed_up","activated","validated","committed","rolling_back","rolled_back"}):
        raise Denied("malformed_transaction_phase")
    for name in ("old","new"):
        r=rec.get(name)
        if not isinstance(r,dict) or not isinstance(r.get("version"),str) or not VALID_VERSION.fullmatch(r["version"]) or not isinstance(r.get("sha256"),str) or not VALID_HASH.fullmatch(r["sha256"]):
            raise Denied("malformed_journal_binary_pin")
    return rec

def worker_gate(root):
    path=root/"workers.json"
    if path.is_symlink() or not path.is_file():
        raise Denied("worker_reconciliation_absent")
    if path.stat().st_size>4096:
        raise Denied("oversized_worker_reconciliation_record")
    data=json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data,dict):
        raise Denied("worker_reconciliation_must_be_object")
    # Fixture proof only; real production broker MUST provide kernel receipts.
    if (data.get("activeWorkers")!=0 or data.get("allReconciled") is not True
            or data.get("osTreeEmpty") is not True):
        raise Denied("workers_not_quiescent_for_upgrade")

def same(actual,expected):
    return actual["version"]==expected["version"] and actual["sha256"]==expected["sha256"]

def crash(fault,point):
    if fault == point:
        os._exit(73) # intentionally defeat Python finally/normal teardown

def upgrade(root,fault="none"):
    root=assert_fixture(root)
    if fault not in FAULTS:
        raise Denied("unknown_failure_injection_point")
    if (root/"journal.json").exists() or (root/"previous").exists() or (root/"rejected").exists():
        raise Denied("existing_or_unreconciled_upgrade")
    worker_gate(root)
    old,new=identity(root,"current"),identity(root,"incoming")
    if old==new or old["version"]==new["version"]:
        raise Denied("not_a_real_version_change")
    rec={"schema":SCHEMA,"state":"prepared","old":old,"new":new,
         "recovery":"manual_reconciliation_only","releaseApproved":False}
    atomic_journal(root,rec)
    crash(fault,"after_prepare")
    os.replace(root/"current",root/"previous")
    fsync_dir(root)
    crash(fault,"after_backup_rename")
    rec["state"]="backed_up"
    atomic_journal(root,rec)
    os.replace(root/"incoming",root/"current")
    fsync_dir(root)
    crash(fault,"after_activation_rename")
    rec["state"]="activated"
    atomic_journal(root,rec)
    if not same(identity(root,"current"),new) or not same(identity(root,"previous"),old):
        raise Denied("post_activation_version_attestation_failed")
    crash(fault,"after_validation")
    rec["state"]="validated"
    atomic_journal(root,rec)
    rec["state"]="committed"
    atomic_journal(root,rec)
    crash(fault,"after_commit")
    return "committed_old_version_retained_for_redundancy"

def recover(root,force_rollback=False,fault="none"):
    root=assert_fixture(root)
    if fault not in ROLLBACK_FAULTS|{"none"}:
        raise Denied("unknown_recovery_fault_injection")
    worker_gate(root)
    rec=read_journal(root)
    state=rec["state"]
    if state=="rolled_back":
        if not same(identity(root,"current"),rec["old"]):
            raise Denied("previously_rolled_back_version_changed")
        return "already_rolled_back"
    if state=="committed" and not force_rollback:
        if not same(identity(root,"current"),rec["new"]) or not same(identity(root,"previous"),rec["old"]):
            raise Denied("committed_version_or_redundant_backup_changed")
        return "committed_previous_retained"
    # A rollback can die between the same-volume renames. Persist its intent
    # BEFORE the first effect, then treat the recovery state as resumable on
    # next startup without silently re-activating the rejected candidate.
    if state!="rolling_back":
        rec["state"]="rolling_back"
        atomic_journal(root,rec)
    crash(fault,"after_rollback_journal")
    previous=root/"previous"
    current=root/"current"
    if previous.exists():
        if not same(identity(root,"previous"),rec["old"]):
            raise Denied("saved_previous_version_is_not_verified")
        if current.exists():
            if (root/"rejected").exists() or not same(identity(root,"current"),rec["new"]):
                raise Denied("unreviewed_current_or_rejected_payload")
            os.replace(current,root/"rejected")
            fsync_dir(root)
        crash(fault,"after_reject_rename")
        os.replace(previous,current)
        fsync_dir(root)
        crash(fault,"after_restore_previous")
    else:
        # Crash immediately after PREPARED: no file effects occurred.
        if not current.exists() or not same(identity(root,"current"),rec["old"]):
            raise Denied("missing_original_after_crash")
    if not same(identity(root,"current"),rec["old"]):
        raise Denied("restored_binary_digest_mismatch")
    rec["state"]="rolled_back"
    atomic_journal(root,rec)
    return "original_version_restored_from_durable_disk"

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--root",type=Path,required=True)
    p.add_argument("--operation",choices=["upgrade","recover","rollback"],required=True)
    p.add_argument("--inject",choices=sorted(FAULTS|ROLLBACK_FAULTS),default="none")
    args=p.parse_args()
    try:
        if args.operation=="upgrade":
            result=upgrade(args.root,args.inject)
        elif args.operation=="rollback":
            # Explicitly disposable fixture-only failure injection. Production
            # installers cannot run this engine or authorize signing/SCM.
            result=recover(args.root,True,args.inject)
        else:
            if args.inject!="none":
                raise Denied("recovery_never_accepts_failure_injection")
            result=recover(args.root)
    except (ValueError,TypeError,OSError) as exc:
        print("DENIED: "+(str(exc) if isinstance(exc,Denied) else "transaction_os_operation_failed"),file=sys.stderr)
        return 2
    print(json.dumps({"operation":args.operation,"result":result,
                      "releaseApproved":False,"fixtureOnly":True},sort_keys=True))
    return 0
if __name__=="__main__":
    raise SystemExit(main())
