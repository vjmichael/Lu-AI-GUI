# TOOLS-08 — Original Jan §7 host-owned loop guard

**2026-11-04. Scope:** internal Lu Core tool-turn safety and progression, not another model tool. **Primary design source:** [unaltered Jan-Mod-Design-Document.md](reference/Jan-Mod-Design-Document.md) §7 and [original-first merged native tool design](LU_NATIVE_TOOLS_MERGED_DESIGN.md) §4. This is a subordinate implementation and CI record.

## Original design audit and corrective interpretation

The original Jan design §7 replaces the old output-pattern breaker with a **structural check of actual tool calls before execution**. It requires:
- A sliding **5-call history** per turn to detect exact same-tool/same-argument requests and **A → B → A → B → A** ping-pong.
- Similar-argument warnings using **Jaccard similarity >0.7** after **3 prior calls to the same tool**, rather than blocking legitimate work with different arguments.
- A separate **per-turn total-call counter** (original warn threshold **20**), and **graduated** actions Allow, Warn, Block and Kill; three consecutive blocks stop the turn.
- Fixed, understandable model-feedback reasons; **no** execution on Block or Kill; preserve user progress and permissions.

**Original sketch defect repaired intentionally:** §7.2 allocates a five-entry history but its pseudocode checks `recentCalls.length >= 20` instead of `totalCallsThisTurn >= 20`. That branch is unreachable. TOOLS-08 counts total attempts *separately*, including blocked proposals. Current native Lu turns independently allow **at most 8 calls** (existing `MAX_TOOL_STEPS`), so the default original 20-call warning cannot fire in a single native turn until the host changes that separate ceiling. We **do not** silently raise the ceiling or change Jan's default just to make the warning trigger. Unit tests prove the independent counter works at 20.

## Implementation map

| Original intent | Lu Core native implementation |
| --- | --- |
| Structural per-turn guard | `src-tauri/lu-core/src/loop_guard.rs` holds `LoopGuard` on each **host-created `NativeToolTurn`**; never global to every conversation or model |
| Stable argument fingerprint | Validated `serde_json` map serialized deterministically; SHA-256 digest. Jaccard similarity uses bounded hashed path/field/primitive tokens and cannot reveal raw arguments |
| History and policy | Host-owned validated `LoopPolicy`, defaults Jan §7: window=5, warn same tool after 3 prior, total=20, block third exact, pingpong=true, similar >0.7, kill after 3 blocks |
| Actual interception | `agent_tool_loop.rs` calls guard **after full model JSON + native codec validation**, **before** `dispatch_bound` or `dispatch_bound_durable`. The host's authority/effect journal are never entered for Block/Kill |
| Allow | Existing native tool dispatch, current `ConversationAuthority`, policy and effect journal, unchanged |
| Warn | Run the authorized native tool normally; add fixed host warning `_lu_loop_warning` inside successful result object; preserve outer strict `{ok,tool,result}` feedback |
| Block | No dispatch, no journal effect; return valid typed model feedback `{ok:false, tool, error:{code:"LOOP_DETECTED", message, retryable:false}}`; consumes one step slot to prevent endless refusal messages |
| Kill | Halt the current turn and return `ToolTurnError::LoopTerminated` (supervisor class `loop_terminated`), prohibit another model continuation or effect |
| Sensitive data | Only fixed, enum-derived feedback; never reflect original tool parameters, file paths, credentials or raw model text in loop warning/error |
| Existing tool ceilings | Independent `MAX_TOOL_STEPS=8` and bounded model JSON/SSE/MBRL remain in force, rather than being replaced |
| Model-facing surface | **No** `lu_loop_guard` model tool, no tool catalog schema, no GUI control needed for correctness; host cannot be instructed by a document to disable guard |

The guard's history tracks **approved/attempted native proposals**; Block/Kill do not append calls, preserving the recognized pattern until a genuinely different request is tried. After an ordinary Allow/Warn, the consecutive-block counter resets. A new task turn starts with a fresh guard; a halted turn cannot continue.

## Verification, remaining production gates

- Real Linux/Windows Rust tests: exact third-call block and three-block termination; A/B alternation with distinct arguments; similarity warning without false blocking; independent 20-attempt counter; invalid policy; real file registry and bounded tool feedback; observer proves native dispatch not invoked on blocks; separate step cap survives; repeat history not shared across newly created turns.
- Re-run existing `native_agent_tool_turn` and `supervised_native_turn` tests because these use the same dispatcher and model feedback. No Cargo dependency or lockfile changes.
- **Durable reconstruction pending:** the `LoopGuard` in-memory history is not currently serialized into ANSF/task journals. A crashed/restarted turn must reconcile recent tool proposals and outcomes from host-owned durable records before authorizing continuation; otherwise conservative halt/review on recovery. This is a **release integration gate**, not a completed feature.
- Actual installed Windows desktop and GGUF model loop behavior, independent long-task cancellation, user-visible termination messaging and audit verification remain separate checks. No change to accepted `0.1.0-alpha.2`; this branch is isolated from unresolved TOOLS-03 Git, TOOLS-04 ZIP, TOOLS-05 schema, TOOLS-06 environment and TOOLS-07 OCR.
