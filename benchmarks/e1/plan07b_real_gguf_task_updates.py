#!/usr/bin/env python3
"""PLAN-07B: genuine pinned local GGUF, original host prompt, abort + strict host audit.

Never generates a synthetic task reply. Native Rust wrote the original prompt,
evidence identities, register and ANSF. The original llama.cpp /completion
response is preserved byte-for-byte for the independent Rust validator.
"""
import argparse
import hashlib
import json
import pathlib
import time
import urllib.error
import urllib.request


def sha(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def get_json(url: str, timeout: float = 10):
    with urllib.request.urlopen(url, timeout=timeout) as response:
        data = response.read(262145)
    if len(data)>262144:
        raise ValueError("Oversized actual llama.cpp server metadata")
    return json.loads(data)


def request(port, payload, timeout=280):
    raw = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}/completion", data=raw,
        headers={"Content-Type": "application/json"}, method="POST",
    )
    with urllib.request.urlopen(req, timeout=timeout) as response:
        answer = response.read(262145)
    if len(answer)>262144:
        raise ValueError("Actual llama.cpp response exceeded receipt budget")
    return raw, answer, json.loads(answer)


def enforce_real_grammar(port):
    """Preflight actual GBNF support, not a claim that JSON was generated."""
    raw, wire, response=request(port,{
        "prompt":"<|im_start|>user\nPlease answer normally.\n<|im_end|>\n<|im_start|>assistant\n",
        "n_predict":32,"temperature":0,"stream":False,
        "cache_prompt":False,"grammar":'root ::= "LU_PLAN07B_GBNF_CHECK_OK"',
    },90)
    if response.get("content")!="LU_PLAN07B_GBNF_CHECK_OK":
        raise ValueError("Real llama.cpp server ignored requested grammar")
    return {"probeRequestSha256":sha(raw),"probeRawHttpSha256":sha(wire),
            "exactGbnfSupport":True}


def quote_gbnf_literal(text: str) -> str:
    return json.dumps(text, ensure_ascii=True)


def grammar_for(native):
    tasks=native["tasks"]
    if not isinstance(tasks,list) or len(tasks)!=1:
        raise ValueError("Genuine tiny-model acceptance fixture must be one original task")
    t=tasks[0]
    original=t["originalRequirementId"]
    if original not in t["allowedEvidenceIds"]:
        raise ValueError("Native source citation is missing")
    status=t["status"]
    allowed=['"collect_evidence"','"review_source_change"']
    if t["exactCriterionCandidate"]:
        allowed.append('"review_exact_criterion"')
    literal=lambda x:quote_gbnf_literal(json.dumps(x, ensure_ascii=True,separators=(",",":")))
    # The GBNF only forces provenance/citation syntax. The real model must
    # actually generate the action and the HUMAN reasoning/evidence prose.
    grammar=(r'''
root ::= "{" ws "\"schema\"" ws ":" ws schema-value "," ws "\"previewSha256\"" ws ":" ws preview-value "," ws "\"candidates\"" ws ":" ws "[" ws "{" ws "\"taskId\"" ws ":" ws task-value "," ws "\"requirementId\"" ws ":" ws requirement-value "," ws "\"status\"" ws ":" ws status-value "," ws "\"action\"" ws ":" ws action-value "," ws "\"evidenceIds\"" ws ":" ws "[" ws requirement-value "]" ws "," ws "\"reason\"" ws ":" ws string "," ws "\"nextEvidence\"" ws ":" ws string "}" ws "]" ws "}"
schema-value ::= SCHEMA
preview-value ::= PREVIEW
task-value ::= TASK
requirement-value ::= REQUIREMENT
status-value ::= STATUS
action-value ::= ACTION
string ::= "\"" ([^"\\\x7F\x00-\x1F] | "\\" (["\\bfnrt] | "u" [0-9a-fA-F]{4})){10,180} "\"" ws
ws ::= | " " | "\n" [ \t]{0,20}
'''.strip()
      .replace("SCHEMA",literal("lu-project-task-update-model-v1"))
      .replace("PREVIEW",literal(native["previewSha256"]))
      .replace("TASK",literal(t["taskId"]))
      .replace("REQUIREMENT",literal(original))
      .replace("STATUS",literal(status))
      .replace("ACTION","("+" | ".join(literal(json.loads(k)) for k in allowed)+")")
    )
    return grammar


def original_native_context(fixture):
    prompt=fixture["prompt"]
    opening="BEGIN_UNTRUSTED_NATIVE_CONTEXT\n"
    ending="\nEND_UNTRUSTED_NATIVE_CONTEXT"
    if prompt.count(opening)!=1 or prompt.count(ending)!=1:
        raise ValueError("Model prompt is not the original native evidence context")
    context=json.loads(prompt.split(opening,1)[1].split(ending,1)[0])
    if context["previewSha256"]!=fixture["previewSha256"] or context["registerId"]!=fixture["registerId"]:
        raise ValueError("Native model context mismatch")
    if len(context["tasks"])!=1 or context["tasks"][0]["taskId"] not in fixture["originalTaskIds"]:
        raise ValueError("Genuine model task was not original native task")
    return context


def atomic_new_json(path, value):
    with pathlib.Path(path).open("x",encoding="utf-8") as output:
        json.dump(value,output,indent=2,sort_keys=True)
        output.write("\n")


def cancel_real_stream(port, cancelled_path, model_sha, engine_sha):
    """Close an actual llama.cpp stream after a real token, before finishing.

    This proves external model transport interruption and server recovery,
    NOT Lu's native FIFO/OS-dialog cancellation (a separate host gate).
    """
    request_data=json.dumps({
        "prompt":"<|im_start|>user\nWrite an extremely long technical report on compiler and linker implementations, at least 10,000 words, in detail.\n<|im_end|>\n<|im_start|>assistant\n",
        "n_predict":3072,"temperature":0.3,"stream":True,"cache_prompt":False
    },separators=(",",":")).encode()
    req=urllib.request.Request(
        f"http://127.0.0.1:{port}/completion",data=request_data,
        headers={"Content-Type":"application/json"},method="POST")
    started=time.monotonic()
    stream=urllib.request.urlopen(req,timeout=100)
    first=None
    try:
        for _ in range(100):
            line=stream.readline(65537)
            if not line or len(line)>65536:
                break
            if not line.startswith(b"data: "):
                continue
            item=json.loads(line[6:].strip())
            if isinstance(item.get("content"),str) and item["content"]:
                if item.get("stop") is True:
                    raise ValueError("Model completed before cancellation")
                first=item
                break
        if first is None:
            raise ValueError("No genuine streaming token observed to interrupt")
    finally:
        # Terminate the actual model HTTP body before any server stop event.
        stream.close()
    # With one admitted llama.cpp slot, wait for actual idle recovery,
    # not merely for an HTTP socket to have been closed client-side.
    health=None
    deadline=time.monotonic()+25
    while time.monotonic()<deadline:
        try:
            probe=get_json(f"http://127.0.0.1:{port}/health",3)
            if probe.get("status")=="ok":
                health=probe
                break
        except urllib.error.HTTPError as error:
            if error.code not in (429,503):
                raise
        time.sleep(0.4)
    if health is None:
        raise ValueError("Real single-slot llama.cpp worker did not recover after aborted stream")
    # Never assert scheduler-level cancellation from mere socket closure.
    report={
        "schema":"lu-plan07b-real-qwen-stream-cancel-v1",
        "realStreamingHttpRequestObserved":True,
        "clientClosedOriginalStreamBeforeCompletion":True,
        "realEngineHealthAfterAbort":True,
        "singleModelSlotRecoveredIdleAfterAbort":True,
        "noSyntheticModelResponse":True,
        "requestSha256":sha(request_data),
        "observedFirstChunkSha256":sha(first["content"].encode()),
        "originalModelSha256":model_sha,
        "originalEngineSha256":engine_sha,
        "secondsUntilInterrupted":round(time.monotonic()-started,3),
        "serverStatusAfterAbort":health["status"],
        "luNativeFifoCancellationTested":False,
        "taskAcceptanceNotInferredFromTransport":True,
    }
    atomic_new_json(cancelled_path,report)
    return report


def main():
    parser=argparse.ArgumentParser()
    for name in ("fixture","reply","cancel","response-receipt","engine","model"):
        parser.add_argument("--"+name,required=True)
    parser.add_argument("--port",type=int,required=True)
    args=parser.parse_args()
    if not 1024<=args.port<=65535:
        raise ValueError("Refuse unreviewed localhost port")
    fixture=json.loads(pathlib.Path(args.fixture).read_text(encoding="utf-8"))
    if fixture.get("schema")!="lu-plan07b-genuine-qwen-original-ansf-fixture-v1":
        raise ValueError("No original native PLAN07B input")
    if fixture.get("baselineOriginalModel")!="SYNTHETIC_BASELINE_NOT_A_GENUINE_MODEL_OUTPUT":
        raise ValueError("Original PLAN05 test-only source mislabeled")
    native=original_native_context(fixture)
    engine=pathlib.Path(args.engine);model=pathlib.Path(args.model)
    if not engine.is_file() or not model.is_file():
        raise ValueError("Actual binary GGUF and engine are required")
    engine_sha=sha(engine.read_bytes());model_sha=sha(model.read_bytes())
    check=enforce_real_grammar(args.port)
    canceled=cancel_real_stream(args.port,args.cancel,model_sha,engine_sha)
    grammar=grammar_for(native)
    original_prompt=fixture["prompt"]
    wrapper=(
        "<|im_start|>system\n"
        "Output only the one JSON object demanded by the native host. "
        "Do not use a think tag. Do not claim completion."
        "<|im_end|>\n<|im_start|>user\n"
        +original_prompt+
        "\n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    )
    payload={
        "prompt":wrapper,"n_predict":1600,"temperature":0.1,
        "cache_prompt":False,"stream":False,
        "stop":["<|im_end|>"],"grammar":grammar,
    }
    sent, wire, result=request(args.port,payload,300)
    content=result.get("content")
    if not isinstance(content,str) or not content.strip() or len(content.encode())>16384:
        raise ValueError("Genuine local GGUF did not produce bounded original JSON")
    if not isinstance(result.get("tokens_predicted"),int) or result["tokens_predicted"]<=0:
        raise ValueError("Original llama.cpp did not report actual generated model tokens")
    # Original bytes MUST be preserved even if strict Rust host later rejects.
    with pathlib.Path(args.reply).open("xb") as output:
        output.write(content.encode("utf-8"))
    receipt={
        "schema":"lu-plan07b-genuine-original-qwen-inference-v1",
        "sourceFixtureSha256":sha(pathlib.Path(args.fixture).read_bytes()),
        "originalPromptSha256":sha(original_prompt.encode()),
        "wireRequestSha256":sha(sent),
        "wireResponseSha256":sha(wire),
        "originalReplySha256":sha(content.encode()),
        "grammarSha256":sha(grammar.encode()),
        "realEngineSha256":engine_sha,
        "realModelSha256":model_sha,
        "actualTokensPredicted":result.get("tokens_predicted"),
        "actualStopReason":result.get("stopping_word"),
        "actualModelContentBytes":len(content.encode()),
        "modelLoadedAndResponded":True,
        "decoderGbnfCheck":check,
        "streamCancellationReceiptSha256":sha(pathlib.Path(args.cancel).read_bytes()),
        "modelReplyOriginalNotRewritten":True,
        "taskCompletionNotAuthorized":True,
        "modelPromptStrictlyNativeOriginal":True,
        "luNativeFifoCancellationTested":False,
    }
    atomic_new_json(args.response_receipt,receipt)
    print("PLAN07B_REAL_PINNED_QWEN_MODEL_REPLY_SHA256="+receipt["originalReplySha256"])
    print("PLAN07B_REAL_STREAM_INTERRUPTED=1")
    print("PLAN07B_MODEL_GENERATED_ORIGINAL_REPLY_BYTES="+str(receipt["actualModelContentBytes"]))
    print("The native Rust validator must independently accept/reject model output.")


if __name__=="__main__":
    main()
