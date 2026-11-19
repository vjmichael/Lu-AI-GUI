#!/usr/bin/env python3
"""Read-only installed Lu acceptance oracle for synthetic CP-05E5 turns."""
import argparse
from collections import Counter
from contextlib import closing
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
    with closing(sqlite3.connect(db.resolve().as_uri() + "?mode=ro", uri=True, timeout=2)) as c:
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
    require(len(result) == 1, f"expected exactly one {kind}; observed {len(result)}")
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

# Failure-only diagnostics. Never export model output, chat text, paths,
# raw error messages or process arguments. Counts and fixed categories only.
_AUDIT_KINDS = frozenset({"USER_REQUEST", "MODEL_OUTPUT", "HOST_ROUTE_SELECTED",
                         "TOOL_FEEDBACK", "FINAL_ANSWER", "REJECTED",
                         "GENERATION_FAILURE"})
_FAILURE_CODES = (
    "InvalidHostInput", "PromptTooLarge", "ModelReplyTooLarge",
    "MissingCompletion", "Cancelled", "InferenceFailed", "TokenLoop",
    "TaskDeadline", "CounterOverflow", "InvalidInferenceEvent", "JournalFailure",
    "Unauthorized", "Catalog", "MalformedModelReply", "UnexpectedFields",
    "TooLarge", "NoAuthorizedTools", "ObservationRequiredBeforeFinal",
    "StepLimit", "NoProgress", "AlreadyFinished", "Revoked",
    "ReadOnlyTurnViolation", "DurableDispatchFailed",
)


def failure_category(payload):
    # Fixed native Rust enum discriminants; do not retain arbitrary suffixes.
    value = payload.removeprefix("ModelReply(")
    for code in _FAILURE_CODES:
        if value == code or value.startswith(code + "(") or value == code + ")":
            return code
    for prefix, code in (("BOUNDARY_BLOCKED:", "BOUNDARY_BLOCKED"),
                         ("LOOP_GUARD_STOP:", "LOOP_GUARD_STOP"),
                         ("POLICY_DENIED:", "POLICY_DENIED"),
                         ("native feedback encoding failed", "FEEDBACK_ENCODING_FAILED"),
                         ("native feedback exceeds bound", "FEEDBACK_TOO_LARGE"),
                         ("host authority no longer permits continuation", "AUTHORITY_REVOKED"),
                         ("task revoked after native tool completion", "AUTHORITY_REVOKED_AFTER_DISPATCH"),
                         ("native tool implementation or codec changed", "IMPLEMENTATION_CHANGED"),
                         ("foreground task wall-time budget exhausted", "TASK_TIME_BUDGET"),
                         ("foreground prompt budget exhausted", "PROMPT_BUDGET"),
                         ("Human declined native inspection", "HUMAN_DECLINED")):
        if value.startswith(prefix):
            return code
    return "UNCLASSIFIED"


def audit_row_category(kind, payload):
    if kind in ("REJECTED", "GENERATION_FAILURE"):
        return failure_category(payload)
    if kind not in ("MODEL_OUTPUT", "TOOL_FEEDBACK"):
        return "PRESENT"
    try:
        obj = json.loads(payload)
    except (ValueError, TypeError):
        return "NOT_JSON"
    if not isinstance(obj, dict):
        return "NOT_OBJECT"
    if kind == "MODEL_OUTPUT":
        if obj == {"selection": "lu_validate_schema"}:
            return "EXACT_SELECTION"
        if obj.get("kind") == "tool":
            if (obj.get("name") == "lu_validate_schema" and
                    obj.get("arguments") == {"source": "current_request"}):
                return "EXPECTED_SOURCE_BOUND_TOOL"
            return "OTHER_TOOL_SHAPE"
        if obj.get("kind") == "final":
            return "FINAL_SHAPE"
        return "OTHER_OBJECT_SHAPE"
    if obj.get("name") == "lu_boundary":
        return "SYNTHETIC_BOUNDARY_FEEDBACK"
    if obj.get("name") == "lu_validate_schema":
        response = obj.get("response")
        if isinstance(response, dict):
            if response.get("ok") is True:
                return "NATIVE_SCHEMA_OK"
            if response.get("ok") is False:
                return "NATIVE_SCHEMA_ERROR"
        return "NATIVE_SCHEMA_UNEXPECTED_RESPONSE"
    return "OTHER_FEEDBACK"


def failure_diagnostic(root, phase):
    out = {"diagnosticOnly": True, "phase": phase, "scope": "exact_synthetic_turn"}
    prompt = {"approve": APPROVE, "deny": DENY}.get(phase)
    if prompt is None:
        prompt = ("No tools. Write an extremely long original fictional story with "
                  "100 detailed paragraphs about " + MARKERS[phase] + ".")
    try:
        chat, task, _, events = locate_human(root, prompt)
    except (OSError, ValueError, sqlite3.Error, KeyError):
        return {**out, "originalTurn": "UNAVAILABLE_OR_NOT_UNIQUE"}
    out["originalTurn"] = "EXACT"
    allowed_events = {"assistant_completed", "assistant_final", "assistant_cancelled",
                      "assistant_failed", "assistant_progress", "assistant_started"}
    out["chatEventCounts"] = dict(Counter(k if k in allowed_events else "OTHER"
                                         for _, _, k, _ in events))
    for name, database in (("selection", "lu-capability-negotiation-audit.sqlite"),
                           ("utility", "lu-core-utility-model-audit.sqlite")):
        try:
            rows = query(root / database, """SELECT kind,payload FROM lu_model_boundary_audit
                         WHERE conversation_id=? AND task_id=? ORDER BY sequence LIMIT 65""",
                         (chat, task))
        except (OSError, ValueError, sqlite3.Error):
            out[name] = {"status": "UNAVAILABLE"}
            continue
        summary = []
        for kind, payload in rows[:64]:
            kind = kind if kind in _AUDIT_KINDS else "OTHER"
            summary.append({"kind": kind, "category": audit_row_category(kind, payload)})
        out[name] = {"status": "READ_ONLY", "truncated": len(rows) > 64,
                     "kindCounts": dict(Counter(r["kind"] for r in summary)),
                     "orderedCategories": summary}
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("profile", type=pathlib.Path)
    parser.add_argument("phase", choices=["approve", "deny", "stop", "graceful", "forced", "all"])
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    phases = ["approve", "deny", "stop", "graceful", "forced"] if args.phase == "all" else [args.phase]
    report = []
    for phase in phases:
        try:
            report.append(record(args.profile, phase))
        except (OSError, ValueError, sqlite3.Error, KeyError, TypeError) as error:
            try:
                print("CP05_FAILURE_DIAGNOSTIC: " + json.dumps(failure_diagnostic(args.profile, phase)),
                      file=sys.stderr)
            except Exception:
                # Diagnostics cannot override or convert the original failure.
                print("CP05_FAILURE_DIAGNOSTIC_UNAVAILABLE", file=sys.stderr)
            raise error
    require(len({r["generation"] for r in report}) == len(report), "reused generation identity")
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))

if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, sqlite3.Error, KeyError, TypeError) as error:
        # All input turns here are synthetic, never dump entire DB or process arguments.
        print("CP05_INSTALLED_ORACLE_NOT_ACCEPTED: " + str(error), file=sys.stderr)
        sys.exit(1)
