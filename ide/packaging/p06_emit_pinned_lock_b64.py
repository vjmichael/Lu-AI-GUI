#!/usr/bin/env python3
"""One-off read-only export of hash-reviewed public dependency lock bytes.

No credentials or arbitrary source files are printed. Remove once committed.
"""
import argparse
import base64
from pathlib import Path
import hashlib
from verify_p06_candidate_lock import check

def emit(root, platform):
    root=Path(root)
    result=check(root,platform)
    print("P06_LOCK_REVIEWED_SOURCE_ONLY_NOT_RELEASE_APPROVED")
    for kind,rel,expected in (
        ("cargo","ide/app/src-tauri/Cargo.lock",result["cargoLockSha256"]),
        ("npm","ide/app/package-lock.json",result["npmLockSha256"])):
        data=(root/rel).read_bytes()
        if hashlib.sha256(data).hexdigest()!=expected:
            raise ValueError("lockfile_changed_between_review_and_export")
        payload=base64.b64encode(data).decode("ascii")
        print("P06_LOCK_BEGIN|"+kind+"|"+expected)
        for n in range(0,len(payload),120):
            print("P06_LOCK_CHUNK|"+kind+"|"+payload[n:n+120])
        print("P06_LOCK_END|"+kind+"|"+expected)
    return result

if __name__=="__main__":
    p=argparse.ArgumentParser()
    p.add_argument("--repo-root",type=Path,required=True)
    p.add_argument("--platform",required=True,choices=("windows-2022","ubuntu-24.04"))
    a=p.parse_args()
    emit(a.repo_root,a.platform)
