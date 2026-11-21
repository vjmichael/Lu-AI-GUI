# CI-POLICY-01 — user-authorized retirement of REPO-01 batching audit

Date: 2026-11-21. Scope: Lu Core repository `vjmichael/Lu-AI-GUI`.
Owner decision: retire the *automatic REPO-01 grouped-workflow trigger policy*
if it blocks engineering progress; preserve meaningful integrity, security
and installed native acceptance evidence.

## Reason and exact scope

The policy enforced by `scripts/lu-ci-group-policy.mjs` and its directory-wide
test demanded manual/reusable dispatch for almost every heavy workflow.
It rejected nine previously existing source-scoped push triggers as
`HEAVY_WORKFLOW_AUTOMATIC_FANOUT`, causing the REPO-01 `verify-source` job
to fail even though the exact-source and Windows/Linux IPC native jobs passed.
See issue #242 and original run #44924483338.

The automatic REPO-01 step is **retired**, not marked `continue-on-error`.
The historical checker and negative tests remain in source for explicit
manual diagnosis, but they no longer constitute a mandatory source,
release or merge gate. The nine workflow trigger declarations are unchanged;
this does not grant permission to broaden automatic triggers in new work.

## Controls retained without waiver

- Exact checked-out branch-head SHA and `source-tree.lock.json` equality
- CORE scope, normal-chat/ANSF and protected fixture source contracts
- Core branch convergence, ledger, source-identity malicious-edit tests
- Queued-run cancellation safety suite (`lu-ci-superseded-queue.test.mjs`)
- Exact-SHA hosted native Windows and Linux IPC/SCM regression suites
- Independent authorization, credential, job custody, refusal and replay tests
- Actual signed installer, LocalSystem/SCM, protected credential lifecycle,
  kernel executor custody, and physical Windows crash/restart acceptance

The last item remains **unaccepted** until real installed OS evidence exists.
A passing REPO-01 source job is not a signed installer or physical Windows
acceptance certificate. Source-control governance changes must not authorize
tool execution, erase historical `UNCERTAIN` effects, or bypass user-turn
authorization. Issue #242 can be closed only as **policy retired by owner**,
not as evidence that all nine legacy workflow triggers were converted.
