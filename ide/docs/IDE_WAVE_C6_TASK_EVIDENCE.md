# Lu IDE Wave C6 — Task evidence and development ledger

**Date:** 2026-11-05 · **Branch:** `ide/wave-c6-task-ledger-20261105` · **Dependency:** C5 PR #49, then C4 PR #48 and C3 PR #47. This document records source implementation, not full native or installed acceptance.

## Canonical truth and operations

C6 extends the existing `development-ledger.jsonl` in the standalone IDE's native app-profile directory. The `DevelopmentEvent` record retains schema v1 and adds an **optional** `task` field so historical projects/events remain readable. Every task update and checked CI observation becomes a separate journal entry under the same process mutex and durable `sync_all` path. A bounded replay reconstructs up to 64 task summaries per registered project; there is no competing project-local task database, hidden workflow, model-backed execution or guessed task identity.

`task.update` (`ide_record_task_update`) is a native-host recorded **human progress claim**. The request includes `projectId`, bounded `taskId` (ASCII alphanumeric, dash, underscore or dot), `title` (120 chars), `status` (planned, in_progress, blocked, ready_for_review), `nextAction` (240 chars), and one or more distinct gate IDs (up to four). The native host verifies the selected project is registered and active and captures actual local Git HEAD and worktree state. No UI-supplied SHA or external verdict, and no `completed` status.

`task.ciObserve` (`ide_observe_task_ci`) is a separate explicit **network read and local journal write**, both initiated by a user click. The browser supplies only `projectId` and `taskId`. The host independently invokes the C5 exact-HEAD public GitHub Actions inspection, confirms the project, current task-event ID and recorded commit still match, and appends bounded workflow ID/verdict metadata. If a task was edited while the request was in flight, the append is refused as stale; the historical source is not overwritten. Unknown/private GitHub, rate limits, CI errors and missing jobs are observations, never fabricated passing results.

## Materialized gate model

The host replays ordered journal events to create `ProjectContext.tasks`, bound to a particular project. Each task includes its latest human status, selected gates, source commit and dirtiness, last update event, optional attached native CI observation, and per-gate evidence state. Source review, installed Windows acceptance, and human approval remain `not_verified/not_tested/not_recorded` until separately proven by future typed host receipts. CI passing for the exact commit yields `commit_ci_passed_not_task_accepted`, never `task_accepted`. A different current HEAD or a revised task event discards old attached CI evidence for current assessment.

The UI has a human-centric task editor and gate chooser in **Continuity**, an explicit **Check & record task CI** button for the linked HEAD, and per-task status/unknowns. **Copy development checkpoint** is still read-only, reobserves native context, and exports only up to 24 task IDs, statuses, source SHA and gate/CI states, excluding private task titles and next-action text. No additional network request occurs during Copy.

## Scope, failure and acceptance limits

- Task notes are human authored; any claim of progress is distinct from host CI receipt. Journal records are app-profile observations, not cryptographically signed receipts or immutable external CI evidence.
- Event replay validates event kind, task ID, status, gates, source SHA, cross-project scope and task-update binding. Corrupt/incomplete journal data fails explicitly, not as an empty task list.
- No installer, compiler, test, debugger, Git mutation, package install, process worker, Lu Core dependency or AI bridge is activated. C6 extends only the existing private local append right and explicit fixed-origin public GitHub read.
- Passing native CI only establishes source/contract behavior on that commit; **physical installed Windows interactions, live public GitHub checks from that installed app, source review, and task acceptance** require separate evidence.

## Required verification

1. Run exact-head Node tests/frontend build including UI, checkpoint privacy, schema parity and task command wiring.
2. Run Linux Rust host tests + rustfmt, and Windows native/Tauri compilation on the identical commit. Repair any failures.
3. Verify history/restart persisted task replay, CI attached to exact event/HEAD, invalidation on task edit or source change and no "accepted" transition from CI. Test with real installed Windows app separately.
4. Merge C3 → C4 → C5 → C6 in that order only after their respective acceptance gates, preserving unmerged work.
