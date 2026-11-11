#!/usr/bin/env python3
"""Read-only independent on-disk PLAN-05 installed acceptance witness.

Never insert/update a row, manufacture a model reply, or claim execution.
"""
import argparse
import hashlib
import json
import pathlib
import sqlite3

p=argparse.ArgumentParser()
p.add_argument("--profile",required=True)
p.add_argument("--seed",required=True)
p.add_argument("--output",required=True)
args=p.parse_args()
profile=pathlib.Path(args.profile).resolve(strict=True)
db=profile/"ansf"/"library.db"
if not db.is_file() or db.is_symlink():
    raise SystemExit("PLAN05 real ANSF SQLite file absent or unsafe")
seed=json.loads(pathlib.Path(args.seed).read_text(encoding="utf-8-sig"))
if seed.get("source")!="TEST_ONLY_SYNTHETIC_MODEL_RAW_JSON_NOT_GGUF":
    raise SystemExit("PLAN05 fixture provenance not preserved")
conn=sqlite3.connect(db.as_uri()+"?mode=ro",uri=True,timeout=5)
conn.execute("PRAGMA query_only=ON")
orig=conn.execute("SELECT proposal_json,original_reply_json,execution_authorized "
    "FROM ansf_project_task_proposals WHERE proposal_sha256=?",
    (seed["originalProposalSha256"],)).fetchone()
if orig is None or orig[2]!=0 or not orig[1]:
    raise SystemExit("PLAN05 original model proposal not backed by immutable bytes")
original=json.loads(orig[0])
if original.get("digestSha256")!=seed["originalProposalSha256"] or not original.get("noExecutionAuthority"):
    raise SystemExit("PLAN05 original proposal audit identity changed")
rows=conn.execute("""SELECT register_revision,previous_sha256,latest_sha256,register_json,
    no_execution_authority FROM ansf_project_task_register_revisions
    WHERE register_id=? ORDER BY register_revision""",
    (seed["registerId"],)).fetchall()
if not rows or len(rows)>32:
    raise SystemExit("PLAN05 missing or implausible installed history")
before=None
older=None
for index,(number,previous,latest,raw,no_exec) in enumerate(rows,1):
    if number!=index or previous!=before or no_exec!=1 or len(raw)>1048576:
        raise SystemExit("PLAN05 revision chain invalid on disk")
    snap=json.loads(raw)
    if snap.get("schema")!="lu-project-task-register-v1" or snap.get("registerId")!=seed["registerId"]:
        raise SystemExit("PLAN05 register origin changed")
    if snap.get("originalProposalSha256")!=seed["originalProposalSha256"] or not snap.get("noExecutionAuthority"):
        raise SystemExit("PLAN05 register model authority escalation")
    events=snap["events"]
    if not events or len(events)>1024 or snap["latestSha256"]!=latest or events[-1]["sha256"]!=latest:
        raise SystemExit("PLAN05 event history mismatch")
    if older is not None and (len(events)<=len(older) or events[:len(older)]!=older):
        raise SystemExit("PLAN05 original history overwritten during restart or amendment")
    for event in events:
        revision=event["revision"]
        if revision.get("executionAuthorized") is not False:
            raise SystemExit("PLAN05 execution permission found in task history")
        if event.get("kind")=="human_revalidated" and revision.get("independentlyVerifiedRequirement"):
            if revision.get("observedStatus")!="ACCEPTED" or revision.get("needsHumanReconciliation"):
                raise SystemExit("PLAN05 acceptance not independently sourced")
    task=[e["revision"] for e in events if e["revision"]["taskId"]==seed["taskId"]][-1]
    if task["version"]<1 or task["sourceRequirementId"]!=original["tasks"][0]["sourceRequirementId"]:
        raise SystemExit("PLAN05 task identity was rewritten")
    before=latest
    older=events
conn.close()
report={
    "schema":"lu-plan05-installed-read-only-sqlite-witness-v1",
    "fixtureNotRealGguf":True,
    "registerId":seed["registerId"],
    "sourceProposalSha256":seed["originalProposalSha256"],
    "rowCount":len(rows),
    "eventCount":len(older),
    "latestSha256":before,
    "taskVersion":task["version"],
    "latestEventKind":older[-1]["kind"],
    "latestCorrectionReason":older[-1]["reason"] if older[-1]["kind"]=="human_correction" else None,
    "independentRequirementAccepted":bool(task["independentlyVerifiedRequirement"]),
    "executionAuthorized":False,
    "storedProposalReplySha256":hashlib.sha256(orig[1]).hexdigest()
}
if report["storedProposalReplySha256"]!=original["originalReplySha256"]:
    raise SystemExit("PLAN05 original raw reply mismatch")
out=pathlib.Path(args.output)
if out.exists():
    raise SystemExit("PLAN05 must not overwrite earlier independent receipts")
out.parent.mkdir(parents=True,exist_ok=True)
out.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
print(json.dumps(report,sort_keys=True))
