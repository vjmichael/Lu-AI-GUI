# AGENT-01J — Bounded read-only continuation ancestry and recovery (draft)

Stack: AGENT-01J on AGENT-01I PR #168. Parent AGENT-01I's exact head and Windows NSIS job are intentionally unchanged.

## Problem
AGENT-01I creates separately authorized child plans, but a crash may interrupt its write sequence between immutable continuation intent, child draft registration, and durable lineage. The UI must distinguish an orphan child from a fully linked historical ancestry, and no recoverer may infer that a worker has stopped or automatically retry the effect. More-than-one-generation ancestry also needs explicit tamper/cycle protection.

## Source implementation
- New Tauri-independent Lu Core `agent_lineage_recovery.rs` reads only the existing bounded regular SQLite plan journal. No model, native executable, scheduler owner, ConversationAuthority, grant or retry is created.
- Trace parent/child ancestry for the authenticated chat up to 32 generations; validate exact plan IDs, human origin, distinct parent identity, SHA-bound revision, step index and matching immutable original continuation intent. Disallow cycles, cross-chat origin, missing parent/intent, damaged records, and unlinked orphan drafts.
- Distinguish `ROOT_PLAN_NO_CONTINUATION`, `LINEAGE_RECORDED_REVIEW_ONLY`, and explicit `STOP_*` reconciliation states. Even successful lineage history returns `NOT_AUTHORIZED_FOR_EXECUTION`. These states do NOT revalidate current source facts or worker custody.
- The read-only `agent_plan_progress` projection exposes ancestry state, parent ID and generation count in the original conversation. Any `STOP_*` suppresses the next-step preview, so an orphan or corrupt draft does not offer a native run button.
- Existing AGENT-01I reviewed child broker still separately reopens current source, original independent receipts and scheduler fence before authorization. The new ancestry UI is advisory, not a replacement for those execution-time checks.

## Acceptance
- Native Rust/SQLite tests: normal root + two-generation chain, orphan + cross-chat refusal, cycle + damaged revision, and actual SQLite-file reopen from orphan to recorded link.
- Six Node source-boundary guards and exact-head Windows 2025 / Ubuntu 24.04 native Lu Core + Tauri compilation.
- This is a SOURCE CANDIDATE until genuine native job logs and installer acceptance are observed; do not mark production recovery or automatic multi-step execution complete.
- Existing AGENT-01I pinned real Qwen3 two-step OS/oracle, OS descendant custody and unsigned physical Windows NSIS CPU test remain separate acceptance gates.

## Next gate
Inspect exact-head real Windows/Linux compiler output, repair any failures, and only then independently test post-crash ancestry resolution in the installed NSIS build. No automatic replay when either original worker or its independent effect oracle is uncertain.
