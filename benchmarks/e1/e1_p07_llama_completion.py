#!/usr/bin/env python3
"""E1-P07 acceptance bridge: raw replies from an actual local llama.cpp GGUF.

TEST-ONLY HTTP transport for ModelStepGenerator, not Lu's production backend.
No fabricated replies, tool-name rewriting, retries or host-selected routing.
Only a localhost llama.cpp /completion endpoint may answer.
"""
import json
import os
import sys
import urllib.request



# Explicit GBNF for llama.cpp /completion. The observed pinned-Qwen
# failure was a complete tool object FOLLOWED by unstructured text, despite
# the previously supplied JSON-schema hint. The grammar constrains the
# decoder's complete reply, rather than deleting the trailing model bytes.
# The model still chooses the tool name and ALL argument values; host-side
# catalog, original-human authority, bounded effects and independent oracles
# remain unchanged. This is a TEST transport, not a synthetic tool call.
_JSON_RULES = r'''
tool ::= "{" ws "\"kind\"" ws ":" ws "\"tool\"" ws "," ws "\"name\"" ws ":" ws string ws "," ws "\"arguments\"" ws ":" ws object ws "}"
final ::= "{" ws "\"kind\"" ws ":" ws "\"final\"" ws "," ws "\"text\"" ws ":" ws string ws "}"
object ::= "{" ws (string ws ":" ws value (ws "," ws string ws ":" ws value)*)? ws "}"
array ::= "[" ws (value (ws "," ws value)*)? ws "]"
value ::= object | array | string | number | "true" | "false" | "null"
string ::= "\"" ([^"\\] | "\\" (["\\/bfnrt] | "u" hex hex hex hex))* "\""
hex ::= [0-9a-fA-F]
number ::= "-"? ("0" | [1-9] [0-9]*) ("." [0-9]+)? ([eE] [+-]? [0-9]+)?
ws ::= [ \t\n\r]*
'''
GENERIC_REPLY_GBNF = {
    "required": "root ::= tool\n" + _JSON_RULES,
    "auto": "root ::= tool | final\n" + _JSON_RULES,
}

def model_payload(prompt: str, tokens: int, protocol: str, tool_mode: str = "auto") -> dict:
    if tool_mode not in ("auto", "required"):
        raise ValueError("Unknown reviewed observation mode")
    if tool_mode == "required" and protocol != "json_schema":
        raise ValueError("Required observation mode needs strict JSON schema")
    if protocol not in ("raw", "json_schema"):
        raise ValueError("Unknown explicit test-only response protocol")
    if protocol == "json_schema":
        # Only for known Qwen3 ChatML prompts. An empty, closed reasoning
        # block is a decoding interface, not a model-selected tool.
        if prompt.endswith("<|im_start|>assistant\n"):
            prompt += "<think>\n\n</think>\n\n"
    result = {
        "prompt": prompt, "n_predict": tokens, "temperature": 0,
        "stream": False, "cache_prompt": False,
        "stop": ["<|im_end|>"]
    }
    if protocol == "json_schema":
        # Host review policy already requires an observed effect before any
        # final answer in these tests. Narrow only the FIRST step to a
        # tool-shaped envelope, without supplying a tool NAME/arguments.
        # Qwen must still select the advertised live catalog tool.
        # Host validates authorization, exact effects, feedback and restart.
        # The former json_schema/oneOf hint did not prevent a real
        # pinned-Qwen reply from appending prose after a valid tool object.
        # llama.cpp supports a directly supplied complete-output GBNF.
        # No host-side JSON slicing, answer rewriting or tool fabrication.
        result["grammar"] = GENERIC_REPLY_GBNF[tool_mode]
    return result


def main():
    request = json.load(sys.stdin)
    if not isinstance(request, dict) or set(request) != {"prompt", "max_tokens", "tool_mode"}:
        raise ValueError("Expected exactly prompt and max_tokens")
    prompt, tokens = request["prompt"], request["max_tokens"]
    tool_mode = request["tool_mode"]
    if not isinstance(prompt, str) or not 0 < len(prompt.encode()) <= 131072:
        raise ValueError("Invalid native-model prompt")
    if type(tokens) is not int or not 1 <= tokens <= 512:
        raise ValueError("Invalid generation budget")
    port = int(os.environ["LU_E1_P07_LLAMA_PORT"])
    if not 1024 <= port <= 65535:
        raise ValueError("Only the configured localhost model port is permitted")
    protocol = os.environ.get("LU_E1_P07_RESPONSE_PROTOCOL", "raw")
    payload = json.dumps(model_payload(prompt, tokens, protocol, tool_mode)).encode("utf-8")
    url = f"http://127.0.0.1:{port}/completion"
    http_request = urllib.request.Request(
        url, data=payload, headers={"Content-Type": "application/json"},
        method="POST"
    )
    with urllib.request.urlopen(http_request, timeout=150) as reply:
        raw = reply.read(262145)
        if len(raw) > 262144:
            raise ValueError("Oversized model-server envelope")
    result = json.loads(raw)
    content = result.get("content")
    if not isinstance(content, str) or not content.strip() or len(content.encode()) > 65536:
        raise ValueError("Missing or oversized genuine model output")
    # The response is emitted exactly as returned by the local model.
    # The Rust model/tool boundary will strictly parse or reject it.
    print(json.dumps({"reply": content}, ensure_ascii=False))


if __name__ == "__main__":
    main()
