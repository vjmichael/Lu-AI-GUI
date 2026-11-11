# Lu P0-Next — project task to reviewed agent-plan integration

**Date:** 2026-11-11  
**Scope:** Lu Core / desktop only. No Lu IDE or NVP code changed.  
**Development branch:** lu/p0-next-project-agent-handoff-20261111  
**Parent:** Lu PLAN-07D source fb336d3868a7dccb751ca93027956219e8d8147e  
**Implementation:** Source candidate. **No CI, PR, compiler, native test or regression job submitted or claimed.**

## Product workflow implemented

1. An operator opens the **independent Project Tasks** window, selects a current,
   human-reviewed PLAN-06 board and one original PLAN-05 task revision.
2. They explicitly choose either:
   - **Review-only**: source collection → analysis → human decision, stored as
     typed plan data. These steps have no currently installed automatic
     source/analysis worker adapter; Lu must not pretend they ran.
   - **One reviewed native Git/media check**: the existing
     INDEPENDENT_CHECK → INDEPENDENT_WITNESS plan step, which can use ONLY
     the previously installed reviewed Git/FFprobe worker after *separate*
     plan review, native source selection, fresh user tool authorization and
     independent outcome checks.
3. A fresh native OS confirmation describes the exact original requirement,
   task wording, acceptance criterion, blockers and limited plan mode.
   Declining has no handoff side effects.
4. A fresh OS folder picker reselects the actual project. The host recomputes
   PLAN-02/03 native source evidence and matches original project scope, HEAD,
   observed source and findings. A changed or uncertain source is not
   silently promoted to a current task.
5. Lu creates a **dedicated new ANSF conversation**, a fresh authenticated
   human preparation turn, and a host-recorded original ANSF goal tied to
   the task's source/criterion. It creates a review-only agent plan using
   AGENT-01F's *existing* ledger, immutable revision and host-owned IDs.
6. The source link is PREPARED in the same original ANSF SQLite library before
   appending the new conversation's turn, goal and draft; it becomes STAGED
   only after a real agent revision is durably recorded and read back.
   Each task version admits one link. Interrupted PREPARED links are shown
   as requiring **manual reconciliation**, never automatically executed or
   rewritten to STAGED.
7. The main chat receives a presentation-only update and reloads the ANSF
   chat list without stealing a different conversation's focus. The task
   board displays its original link, task/source lease validity, plan review,
   host custody and historical witness presence from existing read-only
   agent progress. No task status or original requirement acceptance is
   changed by an agent link.

**Security invariant:** A PLAN-06 board approval is not a plan review; agent
plan approval is not a per-step tool grant; model text or historical memory is
not a verified result. Handoff rows carry and independently check
no_execution_authority=1, and the renderer never submits a path or a grant.
A historical independent witness link does not prove present-day source or
effect validity. No OS tool runs from these new commands.

## Source touched

- New \`lu-desktop/src-tauri/src/project_task_agent_handoff.rs\`.
- \`lu-desktop/src-tauri/src/project_task_board_desktop.rs\`: expose the
  existing audited native board reader to this sibling module.
- \`lu-desktop/src-tauri/src/lib.rs\`: register exactly two task-list-only
  Tauri commands, not model tools.
- \`lu-desktop/web/task_board.jsx\`: accessible plan mode, native handoff
  action and durable task-to-plan progress view.
- \`lu-desktop/web/main.jsx\`: new-chat-list notification without changing
  the active conversation.

## Outstanding implementation and integration work after P0-Next

- **P0-Next follow-on:** Expand the *actual* native worker registry to
  implement useful source collection, analysis and artifacts; review-only
  plan steps are deliberately inert today. Map task types to admissible
  verified methods instead of recasting arbitrary project tasks as Git checks.
- **P0-Next follow-on:** Promote independently witnessed task deliverables
  only into **review candidates** and run PLAN-07A/C fresh evidence scanning,
  original criterion matching and distinct human task acceptance. Never infer
  task completion from a model plan or broker "APPLIED" state.
- **Crash recovery:** PREPARED links need a dedicated operator reconciliation
  path that can inspect a partially created ANSF book/goal/plan without
  reissuing a previous permission or worker. Initial implementation fails
  closed and displays the incomplete link.
- **Source concurrency:** Initial current snapshot is checked again under a
  SQLite IMMEDIATE write transaction; after handoff, subsequent source drift
  is detected on register/board leases and by the existing per-step native
  broker. A task list is not a live filesystem watcher.
- **Integration UX:** Human review of the new plan occurs in Lu's existing
  conversation/agent UI; there is no second plan-review implementation in
  the Project Tasks window.
- **Acceptance deferred by owner directive:** No CI submission, PR
  verification or regression testing until the application-building phase
  is complete. Record actual native Windows/Linux and installed GGUF tests
  only when the owner reopens batched acceptance.

## Task-group handoff

**Implemented in source:** A fresh native-confirmed, source-rechecked,
durable ANSF project-task -> agent-plan handoff and progress projection,
without execution authority.

**Not implemented:** Unrestricted project-wide execution, automatic
completion/acceptance, automatic retry of interrupted handoffs, adapters for
generic source/analysis work or physical-machine acceptance.

**Status:** source engineering candidate, compile/runtime unverified per the
owner's instruction to prioritize app implementation over CI.
