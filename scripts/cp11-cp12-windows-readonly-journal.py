#!/usr/bin/env python3
"""CP-11/12 installed Windows witness: read-only, allowlisted SQLite observations.

Never writes a Lu database, fabricates an approval, or reports an OS-process outcome.
Only fixed synthetic reminder text is matched; run/chat IDs and prompts stay private.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sqlite3
from urllib.parse import quote

def digest(value):
    return hashlib.sha256(value.encode("utf-8")).hexdigest()

def read(profile, mode, expected):
    filename = "lu-background-schedule.sqlite" if mode == "reminder" else "lu-dx-traces.sqlite"
    target = profile / filename
    if not target.exists():
        return {"present": False}
    if not target.is_file() or target.is_symlink() or target.resolve().parent != profile.resolve():
        raise ValueError("untrusted journal path")
    uri = "file:" + quote(str(target.resolve()), safe="/:") + "?mode=ro"
    with sqlite3.connect(uri, uri=True, timeout=0.1) as db:
        db.execute("PRAGMA query_only=ON")
        if mode == "check":
            row = db.execute("SELECT run_id,state,body FROM active_check WHERE slot=1").fetchone()
            if row is None:
                return {"present": False}
            body = json.loads(row[2])
            results = body.get("results", [])
            if not isinstance(results, list) or len(results) > 6:
                raise ValueError("invalid check results")
            # Rust CheckBody has #[serde(default,skip_serializing_if="Not::not")]
            # on model_suite. A FALSE value is therefore omitted in genuine
            # persisted non-model reports; absent is NOT unknown/invalid.
            # An explicit malformed type still fails closed.
            model_suite = body.get("modelSuite", False)
            if type(model_suite) is not bool:
                raise ValueError("invalid model suite flag")
            return {
                "present": True, "runHash": digest(row[0]), "state": row[1],
                "prefixHash": digest(json.dumps(results, sort_keys=True, separators=(",", ":"))),
                "kinds": [x.get("kind") for x in results],
                "verified": [x.get("verifiedOperations") for x in results],
                "passed": [x.get("passed") for x in results],
                "expanded": body.get("expanded"), "modelSuite": model_suite,
                "reason": body.get("reason"), "count": len(results),
            }
        if mode == "reminder":
            if not expected or not expected.startswith("CP11-WINDOWS-TEST-"):
                raise ValueError("synthetic reminder tag required")
            rows = db.execute(
                "SELECT id,workspace_id,state,next_due,repeat_seconds,last_due,fired_count,unread_count "
                "FROM lu_user_reminders WHERE text=?", (expected,)
            ).fetchall()
            if len(rows) > 1:
                raise ValueError("ambiguous matching reminder")
            if not rows:
                return {"present": False}
            x = rows[0]
            return {"present": True, "idHash": digest(x[0]), "ownerHash": digest(x[1]),
                    "state": x[2], "nextDue": x[3], "repeatSeconds": x[4],
                    "lastDue": x[5], "firedCount": x[6], "unreadCount": x[7]}
        if mode == "traces":
            rows = db.execute("SELECT state,COUNT(*) FROM dx_trace GROUP BY state").fetchall()
            if any(state not in ("live", "recovered", "sealed") for state, _ in rows):
                raise ValueError("unrecognized diagnostic state")
            return {"present": True, "states": dict(rows), "count": sum(n for _, n in rows)}
    raise ValueError("unsupported mode")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", type=Path, required=True)
    parser.add_argument("--kind", choices=("check", "reminder", "traces"), required=True)
    parser.add_argument("--tag", default="")
    args = parser.parse_args()
    if not args.profile.is_dir() or args.profile.is_symlink():
        raise SystemExit("No owned installed test profile")
    print(json.dumps(read(args.profile, args.kind, args.tag), sort_keys=True))
