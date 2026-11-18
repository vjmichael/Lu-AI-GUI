# Lu CI — Task-group acceptance, not per-commit workflow stacking

**Owner directive (2026-11-09):** Do not dispatch native Windows/Linux, real-GGUF, E1 OS/worker, specialized tools, and all release acceptance workflows after every tiny source change. Run the relevant acceptance **once at the end of the task group** against its exact frozen head. Do not misrepresent queued jobs as passing.

## Trigger contract (R00-CI-GROUP-01)

- Preserve only **one lightweight automatic PR baseline** in the Lu Core branch: `lu-repo01-full-source.yml` (source identity). The full `lu-ci.yml` desktop/native suite is **task-group-triggered**, not a per-commit smoke. Both retain manual entry points. This source check is not final native acceptance.
- The other **67 specialist and desktop Lu Core workflows** are **workflow_dispatch** only (or preserve existing explicit `schedule`/`workflow_call` where defined); no `pull_request` or `push` event can automatically create a separate workflow for a change to `source-tree.lock.json`, `src-tauri/lu-core/**`, or a small documentation update.
- For each task group, finish implementation, tests and ledger changes first. Then refreeze the source lock, create one exact-head group commit/PR, and request only the matching acceptance workflows using GitHub Actions **Run workflow** / explicit dispatch. Prefer one Windows+Linux matrix per coherent group.
- For the MEMORY-LOOP-03B/03C group: use `lu-memory-loop03b-native.yml` plus real-Qwen `lu-memory-loop02-real-qwen.yml` only when actual required native fixture/model path is ready. For OS/process custody use the distinct `lu-agent01l-os-custody.yml` and `lu-agent01l-linux-cgroup-v2.yml`, not generic source-only checks. Test-gating in the existing workflows is unchanged.
- Keep *exact* revision, tests, failure logs and artifacts. After the grouped acceptance, repair actual failed jobs then retry only necessary failed jobs; do not fan out all workflows again.
- The **standalone Lu IDE** has its own workflow family and should adopt the same grouped policy in its separate branch, but this change does NOT touch or silently disable IDE workflows.
- Current PR branch workflows that are not on the default branch may not be dispatchable in GitHub's workflow UI yet. If so, explicitly record blocked acceptance; never claim a dispatch was run.

## Superseded queued runs

Do **not** blindly cancel all queued runs, especially other projects, still-current exact SHA acceptance, manually requested approvals, or in-progress native tests. The GitHub connection used for this change does not provide workflow cancellation. A repository Actions-write token can run the reviewed `scripts/lu-ci-superseded-queue.mjs` operator tool, which defaults to dry-run, requires the exact branch and current head confirmation, and only cancels QUEUED older-SHA runs for that branch.

Run dry-run first:
```bash
GITHUB_TOKEN=... node scripts/lu-ci-superseded-queue.mjs --repo vjmichael/Lu-AI-GUI --branch lu/memory-loop03b-procedural-belief-ccm-20261109
```
After reviewing the selected run IDs, use `--apply --confirm-head <actual 40-character branch SHA>`. Cancellation outcomes must be independently verified through Actions; a request is not proof the queue was cleared.

## Regressions

`node --test scripts/lu-ci-group-policy.test.mjs` checks the checked-in workflow roster and negative trigger fixtures; `node scripts/lu-ci-group-policy.mjs` prints the exact inventory. Source identity stays separate. The task-group policy is not a CI success claim, OS containment guarantee, or release signature.
