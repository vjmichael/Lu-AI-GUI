#!/usr/bin/env python3
"""PLAN-06 independent read-only witness for original dual-project ANSF installed tests.

No source mutation, no acceptance from claimed UI text, no model evidence substitution.
"""
import argparse,hashlib,json,pathlib,sqlite3

ap=argparse.ArgumentParser()
ap.add_argument("--profile",required=True)
ap.add_argument("--seed",required=True)
ap.add_argument("--output",required=True)
args=ap.parse_args()
profile=pathlib.Path(args.profile)
db=profile/"ansf"/"library.db"
seed=json.loads(pathlib.Path(args.seed).read_text(encoding="utf-8"))
if seed.get("schema")!="lu-plan06-disposable-dual-project-fixture-v1" or seed.get("source")!="TEST_ONLY_SYNTHETIC_ORIGINAL_MODEL_REPLY_NOT_GGUF":
    raise SystemExit("PLAN06 original disposable source fixture invalid")
if len(seed["projects"])!=2 or not seed.get("noExecutionAuthority"):
    raise SystemExit("PLAN06 two original project scopes and authority denial required")
if not db.is_file() or db.is_symlink():
    raise SystemExit("PLAN06 original ANSF SQLite file missing or unsafe")
conn=sqlite3.connect(db.as_uri()+"?mode=ro",uri=True,timeout=3)
conn.execute("PRAGMA query_only=ON")
marker=conn.execute("SELECT marker FROM preserved_user_data").fetchone()
if marker!=("PLAN06_KEEP_606",):
    raise SystemExit("PLAN06 preserved original user data was modified")
if conn.execute("PRAGMA integrity_check").fetchone() != ("ok",):
    raise SystemExit("PLAN06 ANSF SQLite integrity check failed")
sha=lambda data:hashlib.sha256(data).hexdigest()
out=[]
for original in seed["projects"]:
    rid=original["registerId"]
    if not rid.startswith("lu-register-") or not original.get("noExecutionAuthority") or not original["notGenuineGguf"]:
        raise SystemExit("PLAN06 original source identity invalid")
    proposal=conn.execute("""SELECT proposal_json,original_reply_json,execution_authorized
        FROM ansf_project_task_proposals WHERE proposal_sha256=?""",
        (original["originalProposalSha256"],)).fetchone()
    if not proposal or proposal[2]!=0:
        raise SystemExit("PLAN06 missing original proposal or unauthorized execution")
    parsed=json.loads(proposal[0])
    if parsed["digestSha256"]!=original["originalProposalSha256"] or parsed["projectScopeSha256"]!=original["sourceScopeSha256"] or not parsed["noExecutionAuthority"] or len(parsed["tasks"])!=2:
        raise SystemExit("PLAN06 proposal identity was rewritten")
    if sha(proposal[1])!=parsed["originalReplySha256"]:
        raise SystemExit("PLAN06 synthetic original model text mismatch")
    register=conn.execute("""SELECT register_revision,latest_sha256,register_json,
        no_execution_authority FROM ansf_project_task_register_revisions
        WHERE register_id=? ORDER BY register_revision""",(rid,)).fetchall()
    if not register or len(register)>32:
        raise SystemExit("PLAN06 register revision history missing")
    prior=[]
    for i,(version,revision_sha,raw,no_exec) in enumerate(register,1):
        if version!=i or no_exec!=1 or len(raw)>1048576:
            raise SystemExit("PLAN06 register revision invalid")
        snap=json.loads(raw)
        if snap.get("latestSha256")!=revision_sha or snap.get("registerId")!=rid or snap.get("originalProposalSha256")!=original["originalProposalSha256"] or not snap.get("noExecutionAuthority"):
            raise SystemExit("PLAN06 register source or digest changed")
        events=snap["events"]
        if events[:len(prior)]!=prior or len(events)<len(prior):
            raise SystemExit("PLAN06 immutable task revision history overwritten")
        if any(e["revision"]["executionAuthorized"] for e in events):
            raise SystemExit("PLAN06 task history obtained execution authority")
        prior=events
    if register[0][1]!=original["registerSha256"] or len(json.loads(register[0][2])["events"])!=2:
        raise SystemExit("PLAN06 initial task history rewritten")
    boards=conn.execute("""SELECT sequence,source_register_sha256,previous_sha256,
        snapshot_sha256,snapshot_json,no_execution_authority
        FROM ansf_project_task_board_revisions WHERE register_id=? ORDER BY sequence""",
        (rid,)).fetchall()
    prev=None
    last=None
    for i,(seq,source,previous,current,raw,no_exec) in enumerate(boards,1):
        if seq!=i or i>1024 or no_exec!=1 or len(raw)>131072 or not source or not previous:
            raise SystemExit("PLAN06 board row invalid")
        board=json.loads(raw)
        if board.get("registerId")!=rid or board.get("scopeSha256")!=original["sourceScopeSha256"] or board.get("sequence")!=i or board.get("sha256")!=current or board.get("sourceRegisterSha256")!=source or board.get("previousSha256")!=previous or not board.get("noExecutionAuthority"):
            raise SystemExit("PLAN06 board snapshot identity broken")
        if previous != prev and prev is not None:
            raise SystemExit("PLAN06 original board revision chain was overwritten")
        if source not in [row[1] for row in register]:
            raise SystemExit("PLAN06 board claims an unknown register source")
        if set(board.get("orderedTaskIds",[]))!=set(original["taskIds"]) or len(board["orderedTaskIds"])!=2:
            raise SystemExit("PLAN06 board reordered outside original task IDs")
        if board["reviewState"] not in ("draft","human_approved"):
            raise SystemExit("PLAN06 unauthorized plan review state")
        prev=current
        last=board
    out.append({
        "project":original["project"],"scopeSha256":original["sourceScopeSha256"],
        "registerId":rid,"originalRegisterSha256":original["registerSha256"],
        "currentRegisterSha256":register[-1][1],"registerRevisionCount":len(register),
        "originalEventCount":len(json.loads(register[0][2])["events"]),
        "boardSequence":len(boards),"boardSha256":last["sha256"] if last else None,
        "boardReviewState":last["reviewState"] if last else "draft",
        "boardSourceSha256":last["sourceRegisterSha256"] if last else register[-1][1],
        "boardOrder":last["orderedTaskIds"] if last else original["taskIds"],
        "planReviewCurrent":bool(last and last["reviewState"]=="human_approved" and last["sourceRegisterSha256"]==register[-1][1]),
        "noExecutionAuthority":True
    })
conn.close()
report={"schema":"lu-plan06-independent-original-sqlite-readback-v1",
    "fixtureNotRealGguf":True,"profile":str(profile),
    "preservedUserData":True,"projects":out,"noExecutionAuthority":True}
output=pathlib.Path(args.output)
if output.exists():
    raise SystemExit("PLAN06 witness cannot replace prior independent receipt")
output.parent.mkdir(parents=True,exist_ok=True)
output.write_text(json.dumps(report,indent=2,sort_keys=True)+"\n",encoding="utf-8")
print(json.dumps(report,sort_keys=True))
