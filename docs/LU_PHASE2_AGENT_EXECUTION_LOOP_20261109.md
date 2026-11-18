# Lu Core — Phase 2: verified, human-directed multi-step agent loop

Date: November 9, 2026. Target: draft PR #200, Lu Core/installed desktop.

## Engineering goal

Connect Lu's existing AGENT-01A/B/C/D/I/J/K, model-to-tool and reviewed native worker into a useful **multi-step** workflow without inventing authority, receipts or a new generic process executor. GitHub CI is supporting evidence, not a prerequisite to development.

## Previous actual gap and repair

The installed `lu_agent_prepare_next_step` verified the preceding independently witnessed Git/FFprobe operation and created a separately reviewed child plan, but always created exactly **one** child step. This stranded plans needing 3+ operations.

The Phase 2 `remaining_reviewed_steps` operation now projects the *entire unexecuted, dependency-ordered suffix* of the original immutable reviewed revision into a fresh child plan. It renumbers dependencies without rewriting prior receipts, source SHA, goal or historical authority. Repeating this from each verified/released child supports a bounded 3+ step chain. The original owner remains released, and **each child** requires its own authenticated human preparation, exact plan approval, native OS resource picker, one fresh explicit tool grant and independently checked original Git/FFprobe worker. No old permission is inherited.

Only `INDEPENDENT_CHECK → INDEPENDENT_WITNESS` is bound to the existing reviewed native worker today. Future `PRESENT`, `ANALYZE`, `COLLECT_SOURCE` and other typed steps are preserved in the new plan **as unexecuted typed data**, not silently recast as Git. A non-sequential dependency graph requires fresh human-reviewed replanning instead of being flattened or executed incorrectly.

## Recovery and independent verification

Lu already uses AGENT-01C's immutable independent outcome/oracle verification, AGENT-01D's per-host fenced scheduler, AGENT-01I/J's child intent+lineage, AGENT-01K's independent recovery report and AGENT-01L's OS process custody. Phase 2 **reuses those production paths**. Only the verified worker outcome can be a historical plan witness; a model's final answer, staged plan, `APPLIED` task state or a source-looking SHA cannot advance a step. The previous source and process tree are rechecked at each child worker admission. Live authority remains with the native broker.

## Replanning

- The installed desktop has a typed, human-editable **replacement plan outline** (1–8 steps) and an exact reason. The browser may only propose step kinds, deliverable types and backward-only dependencies.
- A native OS confirmation creates a fresh saved human turn. `lu_agent_replan_lineage` stages an immutable original-plan/revision/turn/reason intent **before** another plan draft is created; `stage_initial_draft` uses the current original ANSF goal and records the proposed graph with no tool authority. A second immutable table binds the created child, and a separate native plan review is still mandatory.
- An interruption between intent and child binding makes the child **unlinked, nonexecuting and requiring reconciliation**. A plan created while the previous worker was uncertain can be reviewed but **cannot execute** until independently verified prior process custody/effect evidence is accounted for. On execution, all replan ancestors are bounded to 32 and reverified. Invalid, stale, mutated and cyclic ancestry is rejected.
- A previous never-claimed, all-unexecuted plan can be replaced after a fresh user review without fabricating a process termination receipt. Any prior claimed work goes through full independent recovery first.
- Old execution journals remain unchanged, append-only and non-replayable; the replacement plan gets its own human review, owner, permission and effect journal.

## Independent native checks

One exact-head **manual** Phase 2 Windows/Linux group verifies: real Core SQLite immutable replan intents, interrupted/restart detection, stale revision refusal, bounded child remainder and non-linear graph refusal in production Tauri, frontend step typed validation, existing independent receipts/recovery and actual Git/FFprobe reviewed operation on Linux. It does **not** assert model selected the correct tool, that a physical Windows GGUF+AMD Vulkan installer ran, or that future specialist adapters exist.

## Remaining downstream tasks (outside Phase 2)

- Phase 3 — ANSF source/reasoning/procedural-memory integration.
- Phase 4 — richer workspace UI, thread editing, model management and progress.
- Phase 5 — installed Windows GGUF/Vulkan, NSIS upgrades/rollback and release-grade lifecycle.
- Specialist adapter extensions beyond Git/FFprobe require implementation and independent authority/review, not bypasses of this general orchestration boundary.

This handoff implements safe sequential multi-step reviewed execution, independent result gating, deliberate replanning and durable child/replan recovery. It does not authorize **unattended or implicitly approved** multi-step execution.
