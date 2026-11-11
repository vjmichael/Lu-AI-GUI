#!/usr/bin/env python3
"""Independent read-only final witness for the PLAN-07B real Qwen ANSF run.

Reads original SQLite and original inference bytes, not Rust's PASS text.
No writes to the owner profile; leaves source, task register and model output.
"""
import argparse
import hashlib
import json
import pathlib
import sqlite3

p=argparse.ArgumentParser()
for name in ("profile","fixture","reply","cancel","inference","host-oracle","output"):
    p.add_argument("--"+name,required=True)
a=p.parse_args()
read=lambda path: pathlib.Path(path).read_bytes()
sha=lambda data: hashlib.sha256(data).hexdigest()
fixture=json.loads(read(a.fixture))
inference=json.loads(read(a.inference))
cancel=json.loads(read(a.cancel))
host=json.loads(read(a.host_oracle))
if fixture.get("schema")!="lu-plan07b-genuine-qwen-original-ansf-fixture-v1":
    raise SystemExit("PLAN07B original native fixture absent")
if inference.get("schema")!="lu-plan07b-genuine-original-qwen-inference-v1" or not inference.get("modelLoadedAndResponded"):
    raise SystemExit("PLAN07B original real GGUF response not proven")
if cancel.get("schema")!="lu-plan07b-real-qwen-stream-cancel-v1" or not (
    cancel.get("realStreamingHttpRequestObserved") and
    cancel.get("clientClosedOriginalStreamBeforeCompletion") and
    cancel.get("realEngineHealthAfterAbort") and
    cancel.get("noSyntheticModelResponse")):
    raise SystemExit("PLAN07B cancelled genuine stream not evidenced")
if inference.get("streamCancellationReceiptSha256")!=sha(read(a.cancel)):
    raise SystemExit("PLAN07B altered original cancellation bytes")
if host.get("schema")!="lu-plan07b-independent-real-qwen-ansf-oracle-v1" or not host.get("realGgufOutputObserved"):
    raise SystemExit("PLAN07B native original ANSF oracle absent")
raw=read(a.reply)
prompt=fixture["prompt"].encode()
if inference["originalReplySha256"]!=sha(raw) or inference["originalPromptSha256"]!=sha(prompt):
    raise SystemExit("PLAN07B actual original model inference SHA mismatch")
if inference["realModelSha256"]!=cancel["originalModelSha256"] or inference["realEngineSha256"]!=cancel["originalEngineSha256"]:
    raise SystemExit("PLAN07B cancellation not against identical real loaded model")
profile=pathlib.Path(a.profile)
db=profile/"ansf"/"library.db"
if not db.is_file() or db.is_symlink():
    raise SystemExit("PLAN07B original ANSF SQLite database missing")
conn=sqlite3.connect(db.as_uri()+"?mode=ro",uri=True,timeout=5)
conn.execute("PRAGMA query_only=ON")
if conn.execute("PRAGMA integrity_check").fetchone()!=("ok",):
    raise SystemExit("PLAN07B SQLite integrity failure")
if conn.execute("SELECT value FROM saved_human_memory").fetchone()!=("KEEP_REAL_QWEN_07B",):
    raise SystemExit("PLAN07B user memory not preserved")
original=conn.execute("""
 SELECT register_revision,latest_sha256,register_json,no_execution_authority
 FROM ansf_project_task_register_revisions WHERE register_id=? ORDER BY register_revision
""",(fixture["registerId"],)).fetchall()
if len(original)!=1 or original[0][0]!=1 or original[0][1]!=fixture["registerSha256"] or original[0][3]!=1:
    raise SystemExit("PLAN07B original human task history changed by model")
r=json.loads(original[0][2])
if r["latestSha256"]!=fixture["registerSha256"] or not r["noExecutionAuthority"]:
    raise SystemExit("PLAN07B source task register is no longer original")
if len(r["events"])!=fixture["taskEventsBefore"] or r["events"][0]["kind"]!="imported":
    raise SystemExit("PLAN07B model appended task acceptance to register")
if any(e["revision"]["independentlyVerifiedRequirement"] or e["revision"]["executionAuthorized"]
       for e in r["events"]):
    raise SystemExit("PLAN07B model independently accepted/authorized tasks")
saved=conn.execute("""
 SELECT original_prompt_text,original_reply_text,suggestion_json,original_prompt_sha256,
 original_reply_sha256,no_completion_authority,no_execution_authority,preview_sha256
 FROM ansf_project_task_update_model_suggestions
""").fetchall()
conn.close()
if len(saved)!=1:
    raise SystemExit("PLAN07B expected exactly one verified original model suggestion")
sp,sr,sj,psha,rsha,no_complete,no_exec,preview=saved[0]
if sp!=prompt or sr!=raw or psha!=sha(prompt) or rsha!=sha(raw) or (
    no_complete,no_exec)!=(1,1) or preview!=fixture["previewSha256"]:
    raise SystemExit("PLAN07B recorded source/model receipt did not match original bytes")
proof=json.loads(sj)
if proof.get("digestSha256")!=host.get("suggestionSha256") or proof.get("expectedRegisterSha256")!=fixture["registerSha256"]:
    raise SystemExit("PLAN07B host proof is detached from original model source")
if not proof["humanReviewRequired"] or not proof["noCompletionAuthority"] or not proof["noExecutionAuthority"]:
    raise SystemExit("PLAN07B model attempted execution/acceptance authority")
if not proof["suggestions"] or any(not t["suggestedOnly"] or t["taskComplete"] or
                                   t["executionAuthorized"] or t["independentlyAccepted"]
                                   for t in proof["suggestions"]):
    raise SystemExit("PLAN07B suggestion automatically accepted work")
if host["registerRevisionsBefore"]!=host["registerRevisionsAfter"] or host["acceptedTasks"]!=0 or host["executedTasks"]!=0:
    raise SystemExit("PLAN07B native host claimed a changed task register")
report={
    "schema":"lu-plan07b-independent-genuine-qwen-final-acceptance-v1",
    "originalSourceRegisterSha256":fixture["registerSha256"],
    "registerUnchangedAfterModelAndCancel":True,
    "originalGGUFReplySha256":sha(raw),
    "originalPromptSha256":sha(prompt),
    "actualPinnedGGUFModelSha256":inference["realModelSha256"],
    "actualPinnedEngineSha256":inference["realEngineSha256"],
    "observedActualGenerationTokens":inference.get("actualTokensPredicted"),
    "realModelStreamAbortedBeforeCompletion":True,
    "originalANSFModelResponseBytesVerified":True,
    "modelOnlySuggestedHumanEvidence":True,
    "taskCompletionAuthorityGranted":False,
    "executionAuthorityGranted":False,
    "registeredHumanTaskAcceptanceCount":0,
    "originalHumanMemoryPreserved":True,
    "physicalInstalledLuAcceptanceVerified":False,
    "luProductionFIFOModelCancellationVerified":False,
}
output=pathlib.Path(a.output)
with output.open("x",encoding="utf-8") as f:
    json.dump(report,f,indent=2,sort_keys=True)
    f.write("\n")
print(json.dumps(report,sort_keys=True))
print("PLAN07B_INDEPENDENT_REAL_GGUF_NO_AUTO_ACCEPTANCE_PASS=1")
