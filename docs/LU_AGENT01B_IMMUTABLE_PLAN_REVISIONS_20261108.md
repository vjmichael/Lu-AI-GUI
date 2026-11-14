# AGENT-01B — Immutable goal-aware plan revisions (draft candidate)

**Date:** November 8, 2026. **Parent:** AGENT-01A draft PR #152.  
**Acceptance:** SOURCE ONLY until exact-head native Windows/Linux proof; no approval of live autonomous execution or installed-app release.

## From concept to understandable work

Lu's goal is to turn a human concept into small steps whose purpose can be explained, challenged, improved, and recovered later:

1. The **original human request** is a saved, authenticated ChatStore turn.
2. The **current goal** is identified by an immutable ANSF `goal-<32hex>` and `m-<32hex>` SetGoal event (not copied AI prose).
3. An **unexecuted plan proposal** has fixed step purposes, backward-only dependencies, typed expected deliverables and opaque `d-<32hex>` result identities. Each assumption is a stable ANSF source locator; the host must check current access to the source. Text copied from model output cannot authorize references.
4. Any revision becomes a **new append-only numbered proposal**, linked to its parent's exact SHA-256. The caller must supply the expected parent SHA for compare-and-swap; stale or duplicate conflicting updates fail.
5. The Context Change Monitor compares actual previous/new goal IDs and goal-event IDs, assumption sources, step dependency/kind graph and deliverable IDs/types. It records the actual change flags and rejects no-op or misleading reason classifications. A goal change can invalidate earlier assumptions and must be reviewed; it is not silently executed.
6. **Human review is a separate record**. Before approving, the host must recheck that the exact referenced ANSF goal event is still the **current** goal revision and that every assumption source remains accessible. A stale proposal may always be explicitly rejected; it cannot be newly approved as current.  A new persisted human turn must explicitly approve/reject the **exact revision digest** through a host-only review verifier. Merely existing as a human chat message is NOT proof that it approved a particular revision. Model and WebView calls cannot manufacture this verifier.
7. Reviewing a plan, even APPROVED_PLAN_ONLY, **never authorizes execution**, does not reopen old per-turn grants, does not schedule tools, cannot resume an abandoned OS worker and cannot claim deliverables are produced. Active AGENT-01C/01D remain separately gated.

## Source changes

- New `src-tauri/lu-core/src/agent_plan_revisions.rs`, exposed in Lu Core. The revision store shares the actual AGENT-01A plan SQLite path and validates the original plan's exact conversation.
- `lu_agent_plan_revisions`: immutable revision ID, sequential revision index 1–64, saved human source turn, goal ID/event ID, explicit revision reason, previous digest and payload digest.
- `lu_agent_plan_revision_steps`: each revision's immutable 1–32 typed step records, one dependency on a lower index, one unique expected deliverable ID/type.
- `lu_agent_plan_revision_assumptions`: 0–16 unique, parseable stable ANSF source references only, with scope/readability checked by a separate host reference verifier.
- `lu_agent_plan_revision_reviews`: at most one immutable explicitly verified human decision per revision. Changes are added as a subsequent revision, not an update.
- SQLite triggers deny UPDATE/DELETE on revisions, steps, assumption references and human reviews. A read reconstructs the full bounded chain and recomputes digests, rejecting a damaged or inconsistent revision.
- Source-based delta flags: goal changed, assumptions changed, dependency graph changed, and expected deliverables changed. A model-supplied change reason without a matching actual change is rejected.
- Original initial revision must match the registered AGENT-01A step-kind/dependency structure, so a revision cannot masquerade as an older plan.

## Tests and evidence requested

Dedicated `.github/workflows/lu-agent01b-plan-revisions.yml` targets Ubuntu 24.04 and Windows 2025 with exact-head source-lock verification. It requests:

- Five genuine production Rust SQLite tests: close/reopen revision chain; stale concurrent expected-parent hash; unauthorized/malformed goal and assumption refs; immutable human approval/rejection; duplicate/reordered dependencies; source-computed change-delta classification.
- Portable native-chat test uses the *real* ChatStore saved human turn and ANSF SQLite goal-event mirror, not a model-mock user identity. It separately records an explicit host review for the exact revision hash; then appends an authentic superseding ANSF SetGoal event, rejects an attempted stale goal reference, creates a new unapproved revision and verifies that the old approval survives only as history after restart. No execution is granted.
- Runner log artifacts with the source SHA and exact compiled test names. Windows/Linux compile/test status must be inspected, not inferred from workflow submission. Real installed-Windows GUI approval testing is deferred until near v1.0, following owner scheduling.

## Explicit limitations / next group

- No production desktop/ChatStore binding for the new revision API yet. The host reference verifier and exact-review verifier are interfaces, not an installed and proven approval UI. No model-to-plan generation pipeline or real GGUF planning benchmark has run.
- Source references were checked on proposal creation, **not atomically snapshotted**. A later ANSF revision may supersede the goal: future AGENT-01C must revalidate current references and independent verified work before adoption.
- The latest reviewed plan is not a worker instruction or an active agent checkpoint. The AGENT-01A running-task journal is not rewritten by a revision, including already observed/uncertain steps. Explicit user decisions and a new independently authorized task are still required before actual work.
- Next: AGENT-01C independently verified outcome/receipt-to-deliverable binding; block automatic completion or continuation while E1-P03 Windows worker, P06 Git/FFprobe, P07 genuine GGUF, P10/P12 independent outcome, P13 revision custody and E1-P15 conversation continuity gates remain unaccepted. P08C kernel cgroup-v2 escape custody remains separately scoped.

## Security and UX

No arbitrary executable, arguments, program path, model-generated authority, privileged OS operation, project data duplication or background worker is added. Lu Core itself is not sandboxed. The new object is an immutable planning *proposal* with human review and source evidence, meant to help make AI actions understandable and accountable rather than hide them.
