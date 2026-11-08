#!/usr/bin/env python3
"""Offline, mutation-safe queue policy tests. No GitHub credentials required."""
from datetime import datetime, timezone
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from prune_superseded_repo_actions import (
    audit, eligible, replacement_exists, allowed_branch
)

NOW = datetime(2026, 11, 8, 20, 0, tzinfo=timezone.utc)
HEAD = "a" * 40
OLD = "b" * 40
BRANCH = "lu/e1-p15-conversation-continuity-candidate-20261108"
IDE = "ide/b5-large-project-editor-working-set-20261108"


def run(rid, *, sha=OLD, branch=BRANCH, workflow=4, status="queued",
        event="pull_request", time="2026-11-08T19:30:00Z"):
    return {"id": rid, "workflow_id": workflow, "head_branch": branch,
            "head_sha": sha, "event": event, "status": status,
            "created_at": time, "name": "Protected native check"}


class FakeGithub:
    token = "test-token"
    def __init__(self, old_runs, latest, *, drift=False):
        self.old_runs = old_runs
        self.latest = latest
        self.drift = drift
        self.cancelled = []
    def pages(self, route, params):
        return self.old_runs if params["status"] == "queued" else []
    def branch_head(self, branch):
        return "c" * 40 if self.drift else HEAD
    def current_runs(self, head):
        return self.latest if head == HEAD else []
    def request(self, method, route):
        if method == "GET":
            rid = int(route.rsplit("/", 1)[-1])
            return 200, next(r for r in self.old_runs if r["id"] == rid)
        assert method == "POST" and route.endswith("/cancel")
        self.cancelled.append(int(route.rsplit("/")[-2]))
        return 202, {}


class Tests(unittest.TestCase):
    def test_only_lu_and_ide_scopes(self):
        self.assertTrue(allowed_branch(BRANCH))
        self.assertTrue(allowed_branch(IDE))
        self.assertFalse(allowed_branch("main"))
        self.assertFalse(allowed_branch("cognitum/native-tests"))
        self.assertFalse(allowed_branch("ide/queued-actions-prune-20261108"))

    def test_strict_status_and_age(self):
        self.assertTrue(eligible(run(1), BRANCH, HEAD, NOW))
        self.assertFalse(eligible(run(2, sha=HEAD), BRANCH, HEAD, NOW))
        self.assertFalse(eligible(run(3, status="in_progress"), BRANCH, HEAD, NOW))
        self.assertFalse(eligible(run(4, event="workflow_dispatch"), BRANCH, HEAD, NOW))
        self.assertFalse(eligible(run(5, time="2026-11-08T19:59:00Z"),
                                  BRANCH, HEAD, NOW))

    def test_unique_workflow_and_event_are_protected(self):
        old=run(1)
        self.assertFalse(replacement_exists(old,[run(2,sha=HEAD,workflow=5)],
                                            BRANCH,HEAD))
        self.assertFalse(replacement_exists(old,[run(2,sha=HEAD,event="push")],
                                            BRANCH,HEAD))
        self.assertFalse(replacement_exists(old,[run(2,sha=HEAD,status="cancelled")],
                                            BRANCH,HEAD))
        self.assertTrue(replacement_exists(old,[run(2,sha=HEAD)],BRANCH,HEAD))

    def test_read_only_preview_never_writes(self):
        api=FakeGithub([run(1)],[run(2,sha=HEAD)])
        receipt=audit(api,apply=False,now=NOW)
        self.assertEqual(receipt["eligibleSupersededCount"],1)
        self.assertEqual(receipt["acceptedRequests"],[])
        self.assertEqual(api.cancelled,[])

    def test_apply_rechecks_current_head_replacement(self):
        api=FakeGithub([run(1)],[run(2,sha=HEAD)])
        receipt=audit(api,apply=True,now=NOW)
        self.assertEqual(receipt["acceptedRequests"],[1])
        self.assertEqual(api.cancelled,[1])
        self.assertEqual(receipt["outcomes"][0]["outcome"],
                         "github_202_cancel_requested_not_terminal")

    def test_drift_to_new_head_preserves_stale_candidate(self):
        api=FakeGithub([run(1)],[run(2,sha=HEAD)])
        original=api.branch_head
        calls=0
        def head(branch):
            nonlocal calls
            calls += 1
            return HEAD if calls == 1 else "c" * 40
        api.branch_head=head
        receipt=audit(api,apply=True,now=NOW)
        self.assertEqual(receipt["eligibleSupersededCount"],1)
        self.assertEqual(receipt["acceptedRequests"],[])
        self.assertEqual(api.cancelled,[])

    def test_lu_prioritized_and_non_replaced_id_preserved(self):
        stale=[run(1,branch=IDE),run(2,branch=BRANCH),
               run(3,branch=BRANCH,workflow=19)]
        current=[run(4,branch=IDE,sha=HEAD),run(5,branch=BRANCH,sha=HEAD)]
        api=FakeGithub(stale,current)
        receipt=audit(api,apply=False,now=NOW)
        self.assertEqual([r["runId"] for r in receipt["candidates"]],[2,1])


if __name__ == "__main__":
    unittest.main()
