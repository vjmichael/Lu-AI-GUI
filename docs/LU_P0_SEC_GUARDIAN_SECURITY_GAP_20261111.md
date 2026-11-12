# Lu Core P0-SEC — Linux guardian and process-custody hardening

Source-only candidate, November 11, 2026. Branch: lu/p0-sec-guardian-hardening-20261111, stacked on Lu Core P0-WIN-CHAT #224. Excludes standalone Lu IDE. No merge, release, or native acceptance.

## Security invariants implemented in source

1. Linux process-group and delegated cgroup guardian READY handshakes use a bounded 3-second host poll/read on the already-owned stdout FD. No blocking read_exact/read_line, detached stdout thread, or silent helper is admitted. A failed admission kills/reaps the exact spawned helper. UNAVAILABLE additionally requires the helper to exit successfully within two seconds.
2. When a kernel cgroup is available, Lu verifies the actual admitted worker PID in its original /proc/PID/cgroup before marking a task RUNNING. It separately checks that the custodian remains alive and stays outside its own killable subtree.
3. The active host polls the custodian's lifetime. Premature loss is ProcessUncertain, not a completed task or positive kernel-custody receipt. Lu attempts to stop the known child leader but cannot promise detached descendant cleanup without a surviving guardian.
4. OS stop state distinguishes reaped from positively verified: a failed guardian closure cannot return success on any retry. A terminal worker is only removed from the native supervisor's owner map after the Linux cgroup or process-group guardian reports successful shutdown. A failed stop retains task ownership; neither cancellation nor completion is falsely accepted.
5. The TaskStore persists immutable host-only custody-uncertainty rows with reason and an append-only event. Reboot reconciliation retains the original uncertainty; COMPLETE, CANCELLED, TIMED_OUT and scan receipt completion are rejected after that uncertainty.
6. The original guardian still independently runs cgroup.kill, requires recursive cgroup.events populated 0, and creates the scoped OS receipt only on success. Receipts are historical observations, never execution authority; no model, WebView, or recovered task can select a PID, OS scope, helper path, or worker.

## Changed source

- src-tauri/lu-win-worker-job/src/lib.rs — bounded admission, RAII failure cleanup, live custodian and /proc membership check, Rust native negative handshake cases.
- src-tauri/lu-core/src/native_background_process.rs — verified post-spawn membership, fail-closed active-poll and cleanup ordering, durable uncertainty records on admission/poll/cancel/timeout failures.
- src-tauri/lu-win-worker-job/src/bin/lu-e1-p08-linux-cgroup-crash-parent.rs — independent fixture membership check.
- src-tauri/lu-core/src/tasks.rs — immutable uncertainty reason/event persistence and denial of later positive terminal statuses; native restart fixture.
- lu-desktop/tests/p0-sec-guardian-hardening.test.mjs — source-regression contracts for timeout, membership, loss and ownership ordering.

## OPEN acceptance and implementation boundaries

- Native exact-head Linux and Windows compile not run. Rust unit and JS source-contract fixtures have been authored but not executed in the native exact-head gate. Destructive delegated cgroup-v2 systemd Delegate=yes test of actual host SIGKILL, nested setsid descendant, kernel empty state and unrelated process survival not run on this branch. Source contracts do not establish kernel custody.
- Negative real-worker Linux guardian SIGKILL/stall, cancellation-versus-completion, authority revocation, uncertain-effect durability and restart reconciliation still need executable fixtures with original host journals and proof. In particular, current active-host polling is not an independently surviving guardian watchdog.
- A hostile process with permission to write an ancestor cgroup.procs may migrate outside the originally delegated leaf. Without an OS nonmigratable boundary or explicit negative proof, general descendant containment remains UNVERIFIED.
- Kernel-uninterruptible helper teardown/reaping cannot be guaranteed merely by a user-space timeout. Treat an unobserved reap as uncertain, never success.
- Portable Windows Job subtree and human-installed Windows/Radeon alpha acceptance remain separate, explicitly deferred. No IDE, NVP, automatic CI, PR merge or release operations are included.

## SEC-02 — executable negative cases authored, native acceptance OPEN

- CI-only Rust binary: src-tauri/lu-win-worker-job/src/bin/lu-p0-sec02-linux-negative.rs; feature gate e1-p08-crash-fixture. The production helper PID accessor is unavailable without that feature.
- Independent Linux verifier: benchmarks/e1/p0_sec02_linux_guardian_negative.py. It records kernel /proc worker membership, guardian identity outside the subtree, nested setsid descendant, original process PID handles and unrelated live control.
- guardian-death: verifier SIGKILLs the original admitted guardian, and the fixture MUST return GUARDIAN_LOST_UNVERIFIED with NO cgroup-empty receipt. A surviving nested descendant is evidence of the guarantee boundary. The verifier cleans the exact disposable subtree.
- guardian-stall: verifier SIGSTOPs the guardian after worker admission. Host stop must time out and return GUARDIAN_STALLED_UNVERIFIED without publishing a receipt. Verifier independently recovers the disposable subtree.
- worker-stall: deliberately unresponsive worker plus nested setsid descendant; a healthy guardian performs genuine recursive kernel cleanup. Only an original populated-zero receipt, missing subtree and terminated descendant qualify as WORKER_STALL_NATIVE_CGROUP_EMPTY.
- Missing delegation, false receipts, missing descendants, failure to stop, accidental unrelated kills, or cleanup failure are NOT valid positive security acceptance.

Manual native run sequence, under a genuinely delegated unprivileged Linux cgroup-v2 service (not an ordinary read-only container):

    cargo build --locked --manifest-path src-tauri/lu-win-worker-job/Cargo.toml \
      --features e1-p08-crash-fixture \
      --bin lu-linux-cgroup-guard --bin lu-p0-sec02-linux-negative
    export LU_AGENT01L_TEST_CGROUP_GUARD="$PWD/src-tauri/lu-win-worker-job/target/debug/lu-linux-cgroup-guard"
    for mode in guardian-death guardian-stall worker-stall; do
      python3 benchmarks/e1/p0_sec02_linux_guardian_negative.py \
        --fixture "$PWD/src-tauri/lu-win-worker-job/target/debug/lu-p0-sec02-linux-negative" \
        --mode "$mode"
    done

These commands are future acceptance instructions, NOT tests reported as run.

## SEC-03 — authority and cancellation race candidate, native acceptance OPEN

- P0-AUTH original ANSF REQUESTED rows have a monotonically increasing host-owned row-ID watermark. The reviewed turn captures its baseline BEFORE native source selection/approval. The exact original chat must match the baseline again after queueing, before opening task authority, and on EVERY reviewed tool authorization.
- The final reviewed ANSF AssistantCompleted append and cancellation REQUESTED append are serialized under the same ANSF catalog mutex. The original answer may complete BEFORE cancellation, but a cancellation recorded first must suppress later completion. A separately approved later turn obtains a NEW watermark and is not permanently poisoned by the earlier cancellation.
- New native ANSF tests use a real chat store to exercise revoke-before-complete, a fresh subsequent human turn, and a simultaneous revoke-vs-complete race. Reviewed policy rejects tool dispatch when the cancellation token is set or the durable REQUESTED watermark changed.
- The ordinary native background-process supervisor now exposes cancel_workspace_owned: it enumerates actual exact-workspace workers while holding its admission/poll ownership lock, stops all even if one fails, leaves failed stops owned and marked uncertain, and does not depend on a potentially truncated UI dashboard. P0-AUTH uses this for unscheduled workers. Reviewed per-turn workers remain subject to the original generation cancellation lane.
- Native Linux tests use actual /bin/sleep workers for concurrent poll-versus-cancel and two-target/one-foreign workspace isolation. A task may end in exactly one terminal receipt; wrong-chat cancellation must not affect other workers.

## Remaining security gates and limits

1. Source code changes and test fixtures have NOT passed exact-head cargo compilation, Node contracts, genuine delegated kernel execution or the installed Windows/Linux matrix. P0-SEC remains OPEN; do not represent fixture presence as accepted OS proof.
2. SEC-02's direct guardian fixture confirms the loss-of-custody ceiling but does not itself execute the full TaskStore journal path. The separately authored supervisor and ANSF tests cover host state transitions; an end-to-end combined original-turn journal and real kernel failure run remains mandatory.
3. There is still an unavoidable authorization-check-to-exec gap unless all effectful workers are bound to a revocable native execution gate; a cancellation racing after the last tool check may leave an historical effect. This is uncertain, not permission to replay or a claim of rollback.
4. No guaranteed custody after the active host dies IF its custodian already died. A hostile worker able to migrate into writable ancestor cgroup.procs can escape the leaf. A tested nonmigratable OS boundary and installed Linux guardian watchdog are separate requirements.
5. Follow-on SEC-04/SEC-05/SEC-06 must reconcile process custody, UI cancellation, restart, model-to-tool, Windows Job, kernel authority ceilings and genuine installed OS evidence before accepting P0-SEC or moving to P0-PLAN.

## SEC-04 — durable recovery, original execution-journal reconciliation and operator UI

**Status: SOURCE IMPLEMENTED, native/installed acceptance OPEN.** This is not proof that
descendants died when a custodian was lost or that any external effect was rolled back.

1. TaskStore::resume now accepts only PAUSED tasks. INTERRUPTED work from an OS
   crash cannot be resumed under an old task identity even without a previously
   recorded custody failure. A PENDING task with immutable custody uncertainty
   becomes INTERRUPTED during exclusive startup reconciliation. START, RESUME
   and positive terminal transitions cannot bypass its immutable failure row.
2. TaskStore::security_recovery_rows is bounded and filtered by the original
   exact chat workspace in SQLite BEFORE LIMIT. It exposes no worker argv,
   PIDs, output paths, source bytes, checkpoint payloads or tool authority.
3. reviewed_task_continuity::sec04_reviewed_turn_security opens host-derived
   original task/effect journals READ ONLY, rejects linked/reparse/oversize
   journal files, verifies the sealed original (conversation_id,user_turn_id)
   binding and refuses cross-chat status attribution. Missing/legacy/unbound
   journals become warnings, never evidence that effects have completed.
4. p0_sec_recovery joins original ANSF authenticated turns, original
   per-turn task/effect journals, global background task uncertainty and
   host cancellation REQUESTED/RESULT rows. Missing RESULT remains unknown
   across restart. Capped history is explicit. Read errors produce a
   fail-closed ORIGINAL_SECURITY_RECOVERY_UNAVAILABLE_RECONCILE status.
5. The existing lu_p0_authority_posture command includes the security
   projection. The Lu conversation native tool activity pane shows accessible
   SEC-04 status, original task/turn exceptions, unresolved cancellation
   counts, effects needing review and bounded-history warnings. Viewing
   cannot restore old authority or automatically replay external effects.
6. Rust regression fixtures model guardian loss, EXECUTING effects, immutable
   original task uncertainty, ANSF cancellation, two restart projections,
   cross-chat origin mismatch, uncertain PENDING admission and resume denial.
   JS source contracts verify source/UI wiring.

**Open acceptance:** exact-head native Rust compilation/tests; genuine kernel
guardian SIGKILL to task/effect journal and operator view on ONE original
human turn; Windows Job restart; physical installed operator and permission
matrix; SEC-05 adversarial containment; SEC-06 consolidated acceptance.
Historical SQLite state is NEVER treated as a positive OS cleanup witness.

## SEC-05 — adversarial native security tests and explicit red gates

**Implementation: tests/defenses authored. Native OS acceptance: OPEN.**
Do not interpret the existence of these fixtures as kernel containment proof.

| Threat | Implemented native/first-party adversary | Pass / failure requirement |
|---|---|---|
| Linux parent cgroup escape | `ancestor-migration` in gated Rust fixture and independent pidfd Python verifier | If ancestor write is denied, guardian must kill nested setsid process; if migration succeeds, a surviving escaped child MUST NOT receive a positive leaf-empty witness. A false witness is RED. |
| Guardian failure/stall | Existing genuine SIGKILL/SIGSTOP, worker-stall and nested descendant cases | Unverified guardian termination NEVER becomes a positive OS stop. No unrelated process may die. |
| Windows cross-chat Job custody | Two actual cmd/ping worker trees under separate native Jobs | Cross-chat stop refused, exact target Job cleaned, unrelated worker remains live until separately stopped. |
| Cancellation timing | 32 concurrent real ANSF cancellation-vs-terminal-append collisions plus new fresh turn | Old watermark never permits late completion; unrelated chat and freshly approved later turn retain their own identity. |
| Forged cancellation | Mutated phase/hash/chat, missing request, forged result, direct deletion attempts | Latest intent checksum and scope checked before each reviewed tool grant; immutable catalog rows reject direct write/delete while parent chat privacy deletion remains supported. |
| SQLite task tampering | Direct state, task-ID, workspace-ID, uncertainty reason, task/event deletion attacks | Immutable custody remains; cannot forge RUNNING/COMPLETED/CANCELLED/TIMED_OUT, rekey, or erase original event. |
| Cross-chat leakage | Wrong-origin reviewed task journal, independent chat status query, capped result window | No task identity leaked to a foreign conversation; bounded history never implies complete clearance. |
| Failed original recovery | Corrupted effect SQLite, symlinked original journal, restarted uncertain task | No automatic retry or grant; fail closed with visible reconciliation required, and unrelated target files unchanged. |

### Manual native attack entrypoint (not run)

On an unprivileged Linux host whose own cgroup-v2 parent is actually delegated:

    bash benchmarks/e1/p0_sec05_native_adversarial_manual.sh

The manual script verifies exact Git source inventory, Node contracts, core
and Tauri native adversarial tests, compiles the first-party feature-gated
guardian and fixture, then executes guardian death/stall, worker stall and
ancestor-migration with independent PIDFD and cgroup evidence. It exits
nonzero for unavailable delegation, a false positive cleanup receipt or
any failed test. Evidence is written to the reported temp directory (or
`SEC05_EVIDENCE_DIR`). No CI dispatch, PR merge or production service is
created. Windows native test: filter `sec05_windows_two_jobs_cross_chat`
from the lu-core Rust test crate on the physical Windows host.

### Hardening implemented

- The original P0-AUTH cancellation watermark now validates the most recent
  immutable REQUESTED row's raw JSON, SHA-256, scope and fixed schema before
  admitting each reviewed tool dispatch. Historical mismatches deny authority.
- ANSF original cancellation rows have no-update and guarded no-direct-delete
  SQLite triggers. Deleting an entire human-owned chat via the original
  parent FK cascade remains supported; model/renderer cannot select rows.
- TaskStore's existing immutable uncertainty row now has SQLite state, owner
  rekey, parent-delete and PROCESS_UNCERTAIN event guards. This supplements
  Rust transitions and protects against simple direct-connection forgery.
- Test-only Linux fixture signals original PIDFD handles and explicitly kills
  a deliberately escaped descendant after any RED result, rather than
  leaving a leaked test process alive. No arbitrary PID or cgroup is supplied
  by a model, WebView or saved task.

### Unclosed security limits and next gate

1. No actual delegated-kernel SEC-05 run, compiler/test result, Windows native
   run, or installed UI security test was obtained in this implementation
   pass. SEC-05 acceptance is OPEN.
2. A same-UID worker able to write an ancestor cgroup.procs may escape the
   original leaf. cgroup.events populated 0 is evidence for that LEAF only.
   The new adversarial gate will report RED if this yields a false receipt;
   a nonmigratable OS isolation boundary is still required for full process
   tree containment. Do not call the guardian escape closed from source alone.
3. SQLite triggers and local SHA-256 are application integrity controls, not
   protection from an attacker who has privileges to modify the SQLite
   schema, remove triggers, or replace the database. The desktop OS account
   and profile permission boundary remains an independent acceptance gate.
4. An external effect committed before cancellation cannot be rolled back
   by these tests. Its historical effect journal and operator-visible
   uncertainty must remain and require a fresh human decision.
5. SEC-06 consolidated Windows/Linux native, installed-app, recovery and
   independent OS custody acceptance remains separate. No release or merge.
