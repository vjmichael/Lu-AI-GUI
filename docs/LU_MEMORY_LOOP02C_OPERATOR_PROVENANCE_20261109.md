# MEMORY-LOOP-02C — Operator-visible, original-source recovery provenance

Status: **source implementation candidate / native OS and installed desktop acceptance OPEN**.  
Parent: [MEMORY-LOOP-02B draft PR #182](https://github.com/vjmichael/Lu-AI-GUI/pull/182).  
Work order: [issue #183](https://github.com/vjmichael/Lu-AI-GUI/issues/183).

## Problem and design

The actual small Qwen2.5-0.5B diagnosis scored 0/18 strict evidence/action comprehension cases. A model explaining source evidence cannot own the recovery policy. 02B implements the pure native four-class `HostSafeNext` policy. 02C exposes that decision to the operator directly **without making the model's explanation an execution or retry grant**.

The new `agent_recovery_provenance.rs` Tauri command accepts only the host-authenticated **current chat ID**. It selects the host profile from Lu's `catalog_path`, verifies the original conversation, reads original ANSF host-human turn IDs and reopens historical per-turn `TaskStore`/`EffectJournal` with the existing `reviewed_turn_continuity`. The latter validates the original chat binding, effect status, linked reviewed worker, independent verdict, original receipt SHA-256, matching source-oracle method and digest. `HostOriginalJobCustody` independently examines original Windows Job / Linux delegated-cgroup descendant evidence **only when semantic source verification is eligible**. SQLite journals are opened read-only by the existing reader. No arbitrary stored/model-selected path or turn ID is accepted.

The returned typed report contains only known original human-turn IDs, approved operation labels, historical effect state, independent source verdict, verified observed-data digest (when valid), original model/worker links, OS descendant witness status, and closed host next-action class. It contains **no** old model output, prompt instruction, worker stdout, arbitrary path, process handle, replayable tool invocation, approval token, scheduler claim or raw effect receipt.

## UI

The **Native tool activity → Original-source recovery provenance → Inspect original evidence** button invokes this read-only host command. Its per-chat snapshot is discarded before each new inspection; errors cannot leave an earlier successful snapshot looking current. The UI explicitly displays:

- Historical verdict vs **OS custody** as two separate observations; a source-only verified operation with unknown Job/cgroup witness requires OS reconciliation.
- Original ANSF human-turn ID and durable journal status; unknown or missing origin is a stop, not a completed operation.
- Independently observed historical SHA-256 digest only when source integrity and host provenance pass.
- Most-restrictive `HostSafeNext` for every operation, turn, and inspected conversation window; history ordering cannot override a more restrictive original record.
- Missing paired journal, unread per-turn operations (cap four), and bounded original-human history (last one plus up to twelve prior originals) as explicit incomplete-coverage conditions. These **prevent** a global positive recovery claim.
- Current source freshness **NOT RECHECKED**; old verified evidence and process custody are **not** proof of the present resource or liveness.
- No permission reuse, no auto-retry, no model override, no worker resumption and no action triggered by this inspection.

The intentionally neutral display uses no green verified badges. The existing one-step broker, user-native picker and authorization remain unchanged.

## Native and source tests submitted

`agent_recovery_provenance.rs` has four native Rust tests:

1. Real original ANSF/task/effect SQLite source oracle, independent source digest and OS-unknown STOP; close/reopen, compare stable host snapshot; mutate original independent receipt SHA and require manual reconciliation.
2. Missing paired original effect journal and cross-chat source cannot claim verified completion, cannot create a fake journal.
3. Pure source+kernel-witness historical success still requires **fresh** source check and approval; source-only unknown OS, conflicting oracle and omitted history escalate appropriately.
4. More host-original turns than the bounded window must be reported as unread and cannot grant a global positive result.

`memory-loop02c-operator-provenance.test.mjs` includes ten Node source-contract guards for Tauri scope, original independent SQLite/OS evidence, closed policy, missing/replayed/foreign data refusal, source display and stale-read rejection. `.github/workflows/lu-memory-loop02c-provenance.yml` requests exact-source, real React build/check and native Windows 2025/Ubuntu 24.04 Tauri tests. These are **submitted candidates, not an independently observed native PASS** until actual CI logs and artifact evidence arrive.

## Deliberate open gates

- Exact-head Windows/Linux native compile, 02C Rust tests, Node tests, React build and desktop check.
- Real **installed** Lu app acceptance, including surface inspection and restart on physical Windows and Linux.
- Positive Windows Job / genuinely delegated Linux cgroup-v2 escaped-descendant kernel custody and OS-reconciliation display.
- Genuine larger Qwen 3.8 27B explanation quality, model swap and source-bound performance measurements. Model quality never controls the host decision.
- Existing AGENT-01G fresh reviewed step and independent worker/oracle E2E, installed NSIS, AMD Vulkan, and multi-step per-step fresh approval acceptance.

Cognitum remains deferred to Lu 2.0/3.0. The first-party Lu core remains unsandboxed; reviewed external effects retain native per-operation supervision.

## Next acceptance sequence

Observe real current-head OS logs; fix compilation and UI failures; obtain inspected screenshot/video from installed app on both operating systems; re-open the same original journals across restart and a different GGUF, confirm identical host classification, and validate an independently witnessed positive OS-custody fixture. Do not infer any of these from source-only tests.
