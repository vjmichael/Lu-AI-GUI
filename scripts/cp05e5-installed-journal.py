#!/usr/bin/env python3
"""Read-only installed Lu acceptance oracle for synthetic CP-05E5 turns."""
import argparse
import hashlib
import json
import pathlib
import sqlite3
import sys

APPROVE = 'Could you validate this JSON schema {"schema":{"type":"integer"},"data":17}'
DENY = 'Could you validate this JSON schema {"schema":{"type":"integer"},"data":18}'
MARKERS = {
    "stop": "CP05-WINDOWS-STOP-ORCHID",
    "graceful": "CP05-WINDOWS-CLOSE-AZURE",
    "forced": "CP05-WINDOWS-DEATH-CEDAR",
}

def require(ok, msg):
    if not ok:
        raise ValueError(msg)

def query(db, sql, args=()):
    require(db.is_file(), "journal not yet available")
    with sqlite3.connect(db.resolve().as_uri() + "?mode=ro", uri=True, timeout=2) as c:
        return c.execute(sql, args).fetchall()

def locate_human(root, key):
    hits = []
    for book in sorted((root / "ansf" / "books").glob("*.sqlite")):
        rows = query(book, """SELECT seq,event_id,session_id,user_turn_id,generation_id,text
                         FROM ansf_live_chat_events
                         WHERE kind='user' AND text=?""", (key,))
        for row in rows:
            hits.append((book, row))
    require(len(hits) == 1, "expected exactly one saved original synthetic human turn")
    book, row = hits[0]
    seq, eid, chat, uid, gid, text = row
    require(uid and chat, "original task identity missing")
    events = query(book, """SELECT seq,event_id,kind,text FROM ansf_live_chat_events
                        WHERE seq>? AND session_id=? AND
                        (user_turn_id=? OR user_turn_id=? OR generation_id=? OR generation_id=?)
                        ORDER BY seq""", (seq, chat, uid, eid, uid, gid))
    return chat, uid, eid, events

def from_audit(root, database, chat, task, required=True):
    db = root / database
    if not db.exists() and not required:
        return []
    return query(db, """SELECT kind,payload FROM lu_model_boundary_audit
                         WHERE conversation_id=? AND task_id=? ORDER BY sequence""",
                 (chat, task))

def one(rows, kind):
    result = [value for k, value in rows if k == kind]
    require(len(result) == 1, "expected exactly one " + kind)
    return result[0]

def answer_hash(s):
    return hashlib.sha256(s.encode()).hexdigest()

def record(root, phase):
    prompt = {"approve": APPROVE, "deny": DENY}.get(phase)
    if prompt is None:
        prompt = ("No tools. Write an extremely long original fictional story with "
                  "100 detailed paragraphs about " + MARKERS[phase] + ".")
    chat, task, original, events = locate_human(root, prompt)
    kinds = [row[2] for row in events]
    out = {"phase": phase, "originalEvent": original, "conversation": chat,
           "generation": task, "events": [
               {"seq": seq, "id": eid, "kind": kind, "textSha256": answer_hash(text)}
               for seq, eid, kind, text in events]}
    if phase in ("approve", "deny"):
        selection = from_audit(root, "lu-capability-negotiation-audit.sqlite", chat, task)
        require(one(selection, "USER_REQUEST") == prompt, "selection source differs")
        require(json.loads(one(selection, "MODEL_OUTPUT")) == {"selection": "lu_validate_schema"},
                "genuine selection mismatch")
        utility = from_audit(root, "lu-core-utility-model-audit.sqlite", chat, task,
                             phase == "approve")
        if phase == "deny":
            require(one(selection, "REJECTED") == "Human declined native inspection", "refusal missing")
            require(not any(k == "HOST_ROUTE_SELECTED" for k, _ in selection), "refusal became grant")
            require(not utility, "refused turn reached native utility")
            out.update({"originalInput": 18, "refused": True, "nativeDispatches": 0})
        else:
            route = json.loads(one(selection, "HOST_ROUTE_SELECTED"))
            require(route["intentApproved"] is True and route["executed"] is False and
                    route["candidate"] == "lu_validate_schema", "native intent approval missing")
            feedback = json.loads(one(utility, "TOOL_FEEDBACK"))
            response = feedback["response"]
            require(feedback["name"] == "lu_validate_schema" and response["ok"] is True and
                    response["result"]["valid"] is True and response["result"]["draft"] == "2020-12",
                    "actual native validation result missing")
            raw = [json.loads(p) for k, p in utility if k == "MODEL_OUTPUT"]
            require(any(x.get("kind") == "tool" and x.get("name") == "lu_validate_schema" and
                        x.get("arguments") == {"source": "current_request"} for x in raw),
                    "actual model request missing")
            answer = one(utility, "FINAL_ANSWER")
            require(answer.strip() and any(x.get("kind") == "final" and x.get("text") == answer
                                          for x in raw), "model-generated final missing")
            require("assistant_completed" in kinds or "assistant_final" in kinds,
                    "native result not persisted to original conversation")
            out.update({"originalInput": 17, "nativeDispatches": 1, "valid": True,
                        "finalAnswerSha256": answer_hash(answer), "actualAnswer": answer})
    else:
        require(not any(k in ("assistant_final", "assistant_completed") for k in kinds),
                "interrupted work committed completion")
        if phase in ("stop", "graceful"):
            require("assistant_cancelled" in kinds, "explicit cancellation receipt missing")
        out["completed"] = False
    return out

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("profile", type=pathlib.Path)
    parser.add_argument("phase", choices=["approve", "deny", "stop", "graceful", "forced", "all"])
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    phases = ["approve", "deny", "stop", "graceful", "forced"] if args.phase == "all" else [args.phase]
    report = [record(args.profile, phase) for phase in phases]
    require(len({r["generation"] for r in report}) == len(report), "reused generation identity")
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))

if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, sqlite3.Error, KeyError) as error:
        # All input turns here are synthetic, never dump entire DB or process arguments.
        print("CP05_INSTALLED_ORACLE_NOT_ACCEPTED: " + str(error), file=sys.stderr)
        sys.exit(1)
