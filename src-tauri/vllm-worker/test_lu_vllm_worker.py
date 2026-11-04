"""Offline vLLM adapter protocol and abort tests: no vLLM/GPU required."""
import asyncio
import importlib.util
from pathlib import Path
import unittest

worker_path = Path(__file__).with_name("lu_vllm_worker.py")
spec = importlib.util.spec_from_file_location("lu_vllm_worker", worker_path)
module = importlib.util.module_from_spec(spec)
assert spec and spec.loader
spec.loader.exec_module(module)


class FakeOutput:
    def __init__(self, text, finished=False, reason=None):
        self.text = text
        self.finish_reason = reason
        self.finished = finished
        self.outputs = [self]


class FakeEngine:
    def __init__(self):
        self.terminated = asyncio.Event()
        self.aborted = []
    async def generate(self, prompt, params, request_id):
        yield FakeOutput("first", False)
        await self.terminated.wait()
        yield FakeOutput("first and done", True, "stop")
    async def abort(self, rid):
        self.aborted.append(rid)
        self.terminated.set()
    def shutdown(self):
        pass


class FakeSampling:
    def __init__(self, max_tokens):
        self.max_tokens = max_tokens


class WorkerTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.engine = FakeEngine()
        self.worker = module.Worker(self.engine, FakeSampling)
        self.events = []
        async def capture(payload):
            self.events.append(payload)
        self.worker.output = capture

    async def test_request_scoped_abort_uses_native_engine_abort(self):
        await self.worker.handle({"op": "generate", "id": "req_1",
                                  "prompt": "hello", "max_new_tokens": 30})
        await asyncio.sleep(0.01)
        await self.worker.handle({"op": "abort", "id": "req_1"})
        await asyncio.sleep(0.01)
        self.assertEqual(self.engine.aborted, ["req_1"])
        self.assertIn("abort_ack_unverified", [x["event"] for x in self.events])
        self.assertIn("finished", [x["event"] for x in self.events])

    async def test_native_abort_plus_empty_engine_reports_verified_stop(self):
        class VerifiedEngine(FakeEngine):
            def get_num_unfinished_requests(self):
                return 0 if self.terminated.is_set() else 1
        self.engine=VerifiedEngine()
        self.worker.engine=self.engine
        await self.worker.handle({"op": "generate", "id": "req_1",
                                  "prompt": "hello", "max_new_tokens": 30})
        await asyncio.sleep(0.01)
        await self.worker.handle({"op": "abort", "id": "req_1"})
        await asyncio.sleep(0.01)
        self.assertIn("abort_verified_empty_engine",
                      [x["event"] for x in self.events])
        self.assertEqual(self.engine.aborted, ["req_1"])

    async def test_status_explicitly_does_not_claim_scheduler_verification(self):
        await self.worker.handle({"op": "status", "id": "req_1"})
        self.assertIs(self.events[-1]["independent_scheduler_verified"], False)

    async def test_reject_invalid_and_duplicate_ids(self):
        await self.worker.handle({"op": "generate", "id": "../../escape",
                                  "prompt": "hello", "max_new_tokens": 4})
        await self.worker.handle({"op": "generate", "id": "req_1",
                                  "prompt": "hello", "max_new_tokens": 4})
        await self.worker.handle({"op": "generate", "id": "req_1",
                                  "prompt": "again", "max_new_tokens": 4})
        self.assertIn("invalid_request_id", [x["event"] for x in self.events])
        self.assertIn("rejected", [x["event"] for x in self.events])
        self.worker.tasks["req_1"].cancel()
        await asyncio.gather(self.worker.tasks["req_1"], return_exceptions=True)

    async def test_refuse_shutdown_while_request_active(self):
        await self.worker.handle({"op": "generate", "id": "req_1",
                                  "prompt": "hello", "max_new_tokens": 4})
        more = await self.worker.handle({"op": "shutdown"})
        self.assertTrue(more)
        self.assertIn("shutdown_refused_active", [x["event"] for x in self.events])
        self.worker.tasks["req_1"].cancel()
        await asyncio.gather(self.worker.tasks["req_1"], return_exceptions=True)


if __name__ == "__main__":
    unittest.main()
