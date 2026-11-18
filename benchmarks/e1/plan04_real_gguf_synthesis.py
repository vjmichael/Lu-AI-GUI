#!/usr/bin/env python3
"""PLAN-04 original, unmodified genuine GGUF /completion fixture.

The source evidence graph/prompt is emitted by native Lu Core, not Python.
A SHA-pinned real llama.cpp server loaded with a SHA-pinned genuine GGUF
returns the raw proposal. The Rust host validator then accepts or refuses it.
This script cannot authorize any task/tool. No synthetic fallback is allowed.
"""
import argparse
import hashlib
import json
import pathlib
import urllib.request


# The pinned llama.cpp server supports strict GBNF for complete JSON.
# A generic JSON grammar constrains serialization only: it does not supply
# requirement IDs, source statuses, task descriptions, priorities or criteria.
# The model must generate those values; Lu Core independently checks them.
_JSON_GRAMMAR=r'''
root ::= object
value ::= object | array | string | number | ("true" | "false" | "null") ws
object ::= "{" ws (string ":" ws value ("," ws string ":" ws value)*)? "}" ws
array ::= "[" ws (value ("," ws value)*)? "]" ws
string ::= "\"" ([^"\\\x7F\x00-\x1F] | "\\" (["\\bfnrt] | "u" [0-9a-fA-F]{4}))* "\"" ws
number ::= ("-"? ([0-9] | [1-9] [0-9]{0,15})) ("." [0-9]+)? ([eE] [-+]? [0-9] [1-9]{0,15})? ws
ws ::= | " " | "\n" [ \t]{0,20}
'''

def verify_decoder_gbnf(port):
    """A genuine Qwen reply is not accepted if grammar transport is ignored."""
    probe={
        "prompt":"<|im_start|>user\nSay HELLO in a plain sentence.\n<|im_end|>\n<|im_start|>assistant\n",
        "n_predict":32,"stream":False,"temperature":0,"cache_prompt":False,
        "grammar":'root ::= "LU_PLAN04_GBNF_VERIFIED"'
    }
    request=urllib.request.Request(
        f"http://127.0.0.1:{port}/completion",
        data=json.dumps(probe).encode(),method="POST",
        headers={"Content-Type":"application/json"})
    with urllib.request.urlopen(request,timeout=90) as reply:
        data=reply.read(262145)
    if len(data)>262144:
        raise ValueError("Oversized original decoder preflight")
    result=json.loads(data)
    if result.get("content")!="LU_PLAN04_GBNF_VERIFIED":
        raise ValueError("Original loaded llama.cpp did not honor exact grammar")
    print("PLAN04_GBNF_NATIVE_PRECHECK_VERIFIED=1")

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--fixture",required=True)
    p.add_argument("--output",required=True)
    p.add_argument("--port",type=int,required=True)
    args=p.parse_args()
    if not 1024<=args.port<=65535:
        raise ValueError("Not an explicitly admitted localhost port")
    fixture=json.loads(pathlib.Path(args.fixture).read_text(encoding="utf-8"))
    if fixture.get("schema")!="lu-plan04-original-evidence-native-fixture-v1":
        raise ValueError("Missing ORIGINAL native evidence schema")
    prompt=fixture["prompt"]
    if not isinstance(prompt,str) or not 100<len(prompt.encode())<=14336:
        raise ValueError("Invalid source-bound model prompt")
    # Use the unmodified original prompt; only ChatML framing is transport.
    wrapper=(
        "<|im_start|>system\n"
        "Return precisely one JSON object. No chain of thought, prose, or Markdown."
        "<|im_end|>\n<|im_start|>user\n"
        +prompt+
        "\n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    )
    verify_decoder_gbnf(args.port)
    payload=json.dumps({
        "prompt":wrapper,"n_predict":3072,"temperature":0,
        "cache_prompt":False,"stream":False,
        "stop":["<|im_end|>"],
        "grammar":_JSON_GRAMMAR,
    }).encode()
    request=urllib.request.Request(
        f"http://127.0.0.1:{args.port}/completion",data=payload,
        method="POST",headers={"Content-Type":"application/json"}
    )
    with urllib.request.urlopen(request,timeout=280) as response:
        raw=response.read(262145)
    if len(raw)>262144:
        raise ValueError("Oversized genuine model server result")
    data=json.loads(raw)
    response_text=data.get("content")
    if not isinstance(response_text,str) or not response_text.strip():
        raise ValueError("Genuine GGUF returned no completion")
    if len(response_text.encode())>24576:
        raise ValueError("Genuine GGUF returned oversized task JSON")
    pathlib.Path(args.output).write_text(response_text,encoding="utf-8")
    print("PLAN04_ACTUAL_MODEL_RESPONSE_BYTES=",len(response_text.encode()))
    print("PLAN04_ACTUAL_MODEL_REPLY_SHA256=",hashlib.sha256(
        response_text.encode()).hexdigest())
    print("PLAN04_ACTUAL_MODEL_RAW_PREVIEW=",repr(response_text[:512]))
    print("Host Rust validator must assess every claim; output is NOT accepted here.")

if __name__=="__main__":
    main()
