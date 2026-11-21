#!/usr/bin/env python3
"""A source green never authorizes native execution or a signed release."""
import argparse
import json
from pathlib import Path
import re
import sys
class Denied(ValueError):pass
EXPECTED={"P02":"pending_exact_head_installed_os","P03":"pending_protected_native_os",
          "P04":"pending_signed_upgrade_rollback","P05":"source_only_pass",
          "P06":"source_under_development","P07":"deferred_until_post_lu_v1"}
def check(data):
    if data.get("schema")!="lu.ide.p06.acceptance-matrix.v1" or not re.fullmatch(r"[0-9a-f]{40}",str(data.get("sourceBaseline",""))):
        raise Denied("unverified_matrix_schema")
    for key in ("releaseApproved","signingIdentityReviewed","licenseLegalReviewApproved","protectedExecutionAccepted"):
        if data.get(key) is not False:
            raise Denied("unverified_release_promotion:"+key)
    rows=data.get("phases")
    if not isinstance(rows,list) or len(rows)!=6 or {x.get("id") for x in rows if isinstance(x,dict)}!=set(EXPECTED):
        raise Denied("missing_native_or_deferred_gate")
    for row in rows:
        if row.get("state")!=EXPECTED[row["id"]]:
            raise Denied("unverified_gate_accepted:"+row["id"])
        if row["id"]!="P05" and row.get("receipt") is not None:
            raise Denied("invented_os_evidence")
        if row["id"]=="P05":
            receipt=row.get("receipt")
            if not isinstance(receipt,dict) or receipt.get("headSha")!=data["sourceBaseline"] or not isinstance(receipt.get("runId"),int) or receipt["runId"]<=0:
                raise Denied("historical_source_evidence_missing")
    return {"sourceMatrixValid":True,"releaseApproved":False,"nativeAccepted":False}
def main():
    p=argparse.ArgumentParser()
    p.add_argument("--matrix",type=Path,required=True)
    a=p.parse_args()
    try: result=check(json.loads(a.matrix.read_text(encoding="utf-8")))
    except (OSError,TypeError,ValueError,KeyError) as e:
        print("DENIED: "+str(e),file=sys.stderr)
        return 2
    print(json.dumps(result,sort_keys=True))
    return 0
if __name__=="__main__":raise SystemExit(main())
