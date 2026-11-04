#!/usr/bin/env python3
"""Lu-owned vLLM V1 worker (Linux). External engine adapter, not Lu authorization.

Only a verified, host-launched process may speak this bounded JSONL control
protocol on stdin/stdout; native Lu tool authority stays in Rust/Protobuf.
Production packaging must pin Python/vLLM package identity and OS worker
containment before enabling this worker in the GUI.
"""
from __future__ import annotations

import argparse
import asyncio
import json
import re
import sys
from typing import Any

_ID = re.compile(r"^[A-Za-z0-9_-]{1,96}$")
MAX_CONTROL_LINE = 64 * 1024


class Worker:
    def __init__(self, engine: Any, sampling_type: Any, channel=None):
        self.channel = channel if channel is not None else sys.stdout
        self.engine = engine
        self.sampling_type = sampling_type
        self.tasks: dict[str, asyncio.Task[None]] = {}
        self.output_lock = asyncio.Lock()
        self.closed = False

    async def output(self, payload: dict[str, Any]) -> None:
        # stdout is reserved for the Lu control channel.
        async with self.output_lock:
            self.channel.write(json.dumps(payload, separators=(",", ":"), ensure_ascii=True) + "\n")
            self.channel.flush()

    async def generation(self, rid: str, prompt: str, max_tokens: int) -> None:
        last = ""
        try:
            params = self.sampling_type(max_tokens=max_tokens)
            async for result in self.engine.generate(prompt, params, request_id=rid):
                if not result.outputs:
                    continue
                text = result.outputs[0].text
                # vLLM RequestOutput normally carries cumulative text.
                if not text.startswith(last):
                    raise RuntimeError("vllm output prefix changed unexpectedly")
                delta = text[len(last):]
                last = text
                if delta:
                    await self.output({"id": rid, "event": "text", "text": delta})
                if result.finished:
                    reason = result.outputs[0].finish_reason
                    await self.output({"id": rid, "event": "finished",
                                       "reason": str(reason or "unknown")})
                    return
            await self.output({"id": rid, "event": "stream_closed_without_finish"})
        except asyncio.CancelledError:
            # Do not treat local task cancellation alone as engine cancellation.
            await self.output({"id": rid, "event": "reader_cancelled"})
            raise
        except Exception:
            await self.output({"id": rid, "event": "failed"})
        finally:
            self.tasks.pop(rid, None)

    async def handle(self, message: dict[str, Any]) -> bool:
        command = message.get("op")
        rid = message.get("id")
        if command == "shutdown":
            if self.tasks:
                await self.output({"event": "shutdown_refused_active"})
                return True
            self.closed = True
            await self.output({"event": "shutdown"})
            return False
        if not isinstance(rid, str) or not _ID.fullmatch(rid):
            await self.output({"event": "invalid_request_id"})
            return True
        if command == "generate":
            prompt = message.get("prompt")
            max_tokens = message.get("max_new_tokens")
            if (self.tasks or not isinstance(prompt, str)
                    or not prompt.strip() or len(prompt.encode("utf8")) > 16384
                    or type(max_tokens) is not int or not 1 <= max_tokens <= 32768):
                await self.output({"id": rid, "event": "rejected"})
                return True
            self.tasks[rid] = asyncio.create_task(self.generation(rid, prompt, max_tokens))
            await self.output({"id": rid, "event": "accepted"})
        elif command == "abort":
            if rid not in self.tasks:
                await self.output({"id": rid, "event": "not_active"})
                return True
            try:
                await self.engine.abort(rid)
            except Exception:
                await self.output({"id": rid, "event": "abort_failed"})
                return True
            # Only an empty single-request engine *after awaiting the V1
            # EngineCore abort* qualifies for verified request stop. This
            # worker refuses concurrent generate requests above.
            count = getattr(self.engine, "get_num_unfinished_requests", None)
            try:
                empty = callable(count) and count() == 0
            except Exception:
                empty = False
            if empty:
                await self.output({"id": rid, "event": "abort_verified_empty_engine"})
            else:
                await self.output({"id": rid, "event": "abort_ack_unverified"})
        elif command == "status":
            # Reader task is NOT an independent engine scheduler witness.
            await self.output({"id": rid, "event": "status",
                               "reader_active": rid in self.tasks,
                               "independent_scheduler_verified": False})
        else:
            await self.output({"id": rid, "event": "unknown_command"})
        return True


async def serve(worker: Worker) -> None:
    await worker.output({"event": "ready", "protocol": "lu-vllm-control-v1"})
    while not worker.closed:
        # readline runs in a separate thread; the asyncio event loop remains
        # free to stream model output and handle request-scoped abort commands.
        line = await asyncio.to_thread(sys.stdin.buffer.readline, MAX_CONTROL_LINE + 1)
        if not line:
            break
        if len(line) > MAX_CONTROL_LINE or not line.endswith(b"\n"):
            await worker.output({"event": "invalid_frame"})
            break
        try:
            msg = json.loads(line)
        except (UnicodeDecodeError, ValueError):
            await worker.output({"event": "invalid_frame"})
            continue
        if not isinstance(msg, dict) or not await worker.handle(msg):
            break
    # On parent channel loss, explicitly abort in-flight requests before
    # engine shutdown. OS parent supervision remains authoritative if hung.
    if worker.tasks:
        ids = list(worker.tasks)
        try:
            await asyncio.wait_for(worker.engine.abort(ids), timeout=3)
        except Exception:
            pass
        for task in list(worker.tasks.values()):
            task.cancel()
        await asyncio.gather(*list(worker.tasks.values()), return_exceptions=True)
    worker.engine.shutdown()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", required=True)
    parser.add_argument("--max-model-len", type=int, default=32768)
    args = parser.parse_args()
    if not 256 <= args.max_model_len <= 262144:
        parser.error("invalid maximum context")
    # Keep all third-party runtime banners/logs off the bounded stdout
    # control pipe; preserve only the original host-owned channel writer.
    control_output = sys.stdout
    sys.stdout = sys.stderr
    # Keep vLLM import out of module import time, enabling CI protocol tests
    # without CUDA, a GPU or the optional vLLM installation.
    from vllm import AsyncEngineArgs, SamplingParams
    from vllm.v1.engine.async_llm import AsyncLLM

    engine = AsyncLLM.from_engine_args(
        AsyncEngineArgs(model=args.model, max_model_len=args.max_model_len))
    asyncio.run(serve(Worker(engine, SamplingParams, control_output)))


if __name__ == "__main__":
    main()
