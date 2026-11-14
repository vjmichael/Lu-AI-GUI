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



# Opt-in decoder grammar: it permits BOTH tool and final forms, never
# specifies a tool name, command, arguments, authority or a correct answer.
# It only prevents a small Qwen GGUF from wasting the entire token budget
# on an unfinished <think> block or invalid JSON. Native Lu still decides
# whether the chosen tool exists and whether a fresh human approved it.
GENERIC_REPLY_JSON_SCHEMA = {
    "oneOf": [
        {"type": "object",
         "properties": {
             "kind": {"type": "string", "enum": ["tool"]},
             "name": {"type": "string"},
             "arguments": {"type": "object"},
         },
         "required": ["kind", "name", "arguments"],
         "additionalProperties": False},
        {"type": "object",
         "properties": {
             "kind": {"type": "string", "enum": ["final"]},
             "text": {"type": "string"},
         },
         "required": ["kind", "text"],
         "additionalProperties": False},
    ]
}


def model_payload(prompt: str, tokens: int, protocol: str) -> dict:
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
        result["json_schema"] = GENERIC_REPLY_JSON_SCHEMA
    return result


def main():
    request = json.load(sys.stdin)
    if not isinstance(request, dict) or set(request) != {"prompt", "max_tokens"}:
        raise ValueError("Expected exactly prompt and max_tokens")
    prompt, tokens = request["prompt"], request["max_tokens"]
    if not isinstance(prompt, str) or not 0 < len(prompt.encode()) <= 131072:
        raise ValueError("Invalid native-model prompt")
    if type(tokens) is not int or not 1 <= tokens <= 512:
        raise ValueError("Invalid generation budget")
    port = int(os.environ["LU_E1_P07_LLAMA_PORT"])
    if not 1024 <= port <= 65535:
        raise ValueError("Only the configured localhost model port is permitted")
    protocol = os.environ.get("LU_E1_P07_RESPONSE_PROTOCOL", "raw")
    payload = json.dumps(model_payload(prompt, tokens, protocol)).encode("utf-8")
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
