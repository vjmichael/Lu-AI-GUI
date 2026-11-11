# Lu P0-AUTH — Unified host authorization, OS custody and recovery

**Date:** November 11, 2026  
**Stacked branch:** lu/p0-auth-unified-custody-20261111  
**Parent commit:** c5f644bc96974e6d8379fc3c6b3154c970960798  
**Status:** source implementation candidate, NOT native/compiler/GGUF/installed/CI/regression accepted.

## Existing architecture preserved

The authoritative one-use `ConversationAuthority` remains bound to a genuine, persisted ANSF human user turn, exact tool IDs/effects and a host-owned workspace scope. The original native Git, FFprobe, PDFInfo/ExifTool, Browser, Reader, OCR, Observe and background process workers retain their own native supervisors, revocation callbacks, cancellation, bounded output, custody and durable effect/task journals. No new command runner, shell, privileges, model grant, database or worker queue is introduced.

## P0-AUTH gap fixed — launch/revoke race

An original reviewed Git/FFprobe/specialist operation previously checked both permissions and launched a worker, then **registered** that child with its synchronous revocation observer. A concurrent task revocation could finish between the check and registration and miss the already admitted child.

`ConversationAuthority::with_authorized_worker_launch` now checks the exact read and execute grants, current task origin, expiry, scope and independent host policy, then holds the immutable task authority read fence only through the short synchronous native journal prepare, process launch and active-worker registration. Revocation must acquire the corresponding write fence and cannot complete before the child is visible to its registered cancellation callback. The fence **never** remains held across worker execution, network, human dialogs or inference.

Updated all three reviewed worker brokers:
- Git read-only HEAD/status/branch/history;
- FFprobe bounded native WAV/MP4 facts;
- Specialist PDFInfo and ExifTool metadata.

Each retains its original exact-format arguments, source and executable hashes, one-use `TaskStore`/`EffectJournal`, native process custody, independent result oracle, conservative cancellation and original human authorization. A rejected second grant or duplicate effect cannot create a worker. Failed/partially started effects become **UNCERTAIN** and are never automatically retried. The new source-level tests cover revoked task/cross-scope rejection and an actual concurrent revoke/launch registration fence, but they have **not** been executed.

## P0-AUTH operator status and cancellation

Added a bounded, exact-chat native **authority posture**:
- Queued/active model generation and Reader nested-step presence.
- Active host-owned Browser grant; scope-safe Observe status that cannot accidentally revoke another conversation.
- Offline OCR cancellation flag, scheduled and unscheduled native background worker ownership, interrupted task rows.
- Existing original reviewed-effect recovery classification and original immutable cancellation intent/outcome audit.
- All prior permission, automatic retry and historical OS custody fields stay explicitly **false**.

A new user-visible Native Tool Activity panel can inspect that posture or request **Stop only this conversation's native work**. Stop requires a fresh OS-native confirmation. The host derives actual generation IDs, Browser task, Observe scope, OCR flag, scheduled task IDs and unscheduled worker handles from their own owner records; the renderer sends only the currently selected authenticated chat ID and never a PID, path, executable, scope override or model-supplied cancellation target.

The operator Stop uses existing per-domain controls. Only the original owner may stop its own resource:
- Browser: revoke exact task authority, interrupt its model read and stop the owned Chromium process.
- Observe: revoke only this chat's active separate consent/capture, never a second chat's session.
- OCR: set the matching native cancellation flag; do NOT claim the worker was reaped.
- Model/Reader: cancel through the exact-chat generation queue, interrupt the approved Reader's nested step, and issue the supervised inference cancellation when active.
- Scheduled and native background: enumerate only original chat-filtered task IDs and invoke existing host-owned scheduler/process cancellation. Attempt every matched worker even if one fails; an uninspected bounded overflow is marked uncertain.

## Recovery audit in existing ANSF Library

The original ANSF Library SQLite gains a **small append-only host cancellation event table**, not a new memory database. The host writes a `REQUESTED` event **before** attempting any stop, and, after trying every subsystem, an immutable `RESULT` event. Every event is strict-typed, JSON length-bounded and SHA-256 checked on read. The table stores only opaque attempt/chat identity and finite domain status codes—not PIDs, handles, model text, image contents, OS paths or capability tokens. No model/API can write these events.

If power loss or a process crash occurs between REQUESTED and RESULT, the original audit explicitly reports **CANCELLATION_REQUESTED_OUTCOME_UNKNOWN**. A completed result remains **CANCEL_RETURNED_REQUIRES_OS_AND_EFFECT_RECONCILIATION**; neither a returned stop call nor an SQLite terminal row independently proves that every OS descendant is empty. Tampered/invalid records fail closed. The original worker/effect journals and platform custody oracles retain their own authority, and interruption never authorizes replay.

## Scope and incomplete acceptance

This task group improves the shared authorization race for **reviewed external Git/media/specialist worker families** and adds a unified human cancellation/status experience over the existing model, Browser, Reader, OCR, Observe and native background owners. It does **not** claim one global lock across every legacy file write, Browser launch, OCR scheduling or external app; those retain their existing independently reviewed paths. Uncertain source/OS effects remain subject to full reconciliation.

The actual Linux delegated cgroup-v2 and Windows Job Object/physical installed end-to-end process-tree custody, refusal/cancel/restart matrices, native dialogs, source mutation races, file effect recovery, genuine loaded GGUF operations, compiler and regression results remain **unverified** until the consolidated acceptance phase. No PR or CI was opened; no merge/release is authorized.

## Next task group

**P0-GATE (deferred by the owner):** after implementation-first development, do one codebase-wide source reconciliation/repair followed by batched real compiler/native, genuine GGUF, Windows/Linux installed UI, independent custody/receipt, cancellation, restart and regression acceptance. This is not P0-AUTH implementation and is not performed here.

**Stop after P0-AUTH.**