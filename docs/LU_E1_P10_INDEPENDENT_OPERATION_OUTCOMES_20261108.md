# E1-P10 — Independently verified reviewed application outcomes

Date: November 8, 2026

## Goal and security boundary

Lu should be able to distinguish:

1. A reviewed application **ran** and produced a syntactically valid
   receipt (already implemented by E1-P06/P07 and durable E1-P08).
2. The application **reported a fact**. An exit code and its own stdout
   do not independently prove that fact.
3. A separate host-owned source observer **verified the specific fact**
   at a particular observation time.

This phase implements (3) for a deliberately limited set of read-only
operations. Lu Core remains a normal native application and is not
sandboxed. Model text cannot choose program paths, file paths, a new
permission, or the verification method. No MCP or Docker is required.

## Actual independently implemented observations

| Reviewed operation | Independent observer | Result |
|---|---|---|
| Git HEAD commit | Direct bounded read of approved `.git/HEAD` and loose / packed ref; compare commit hash to worker receipt | VERIFIED on match, CONFLICT on disagreement |
| Git current branch | Direct approved `.git/HEAD` symbolic or detached read; compare normalized branch to Git receipt | VERIFIED on match, CONFLICT on disagreement |
| PCM/IEEE-float WAV facts | Direct bounded, SHA-256 checked read of the original host-approved RIFF/WAVE bytes; independently parse fmt/data, duration, codec, rate and channels | VERIFIED on exact compatible facts, CONFLICT on contradiction, UNSUPPORTED on unrecognized format |
| Git tracked-status count | Full independent index/worktree semantic implementation not yet available | UNSUPPORTED |
| Git reachable-history count | Full independent object graph traversal not yet available | UNSUPPORTED |
| MP4 media facts | Independent ISO BMFF/container+track semantics not yet implemented | UNSUPPORTED |

The alternate observation **does not invoke Git or FFprobe a second
time**. It is not just an application replay, a model's assertion or
a fresh parser of the same stdout.

Git observations are currently limited to standard, non-linked local
repositories; a malformed, unresolvable or linked HEAD cannot produce
a verified outcome. The WAV oracle covers bounded RIFF PCM/floating
formats; other valid codecs can remain unsupported. A maximum
2-millisecond duration rounding difference is accepted. A changed
original WAV SHA-256 is a conflict, not a successful probe. Verification
attests to those facts **when observed**, not permanent resource state.

## Actual conversation integration

`src-tauri/lu-core/src/native_reviewed_app_tool.rs` now invokes
`reviewed_outcome_oracle.rs` **after** the reviewed Git/FFprobe
subprocess completed and the inner EffectJournal reached APPLIED, but
**before** the normalized result is returned as model tool feedback.

- If a verified fact agrees, Lu records `VERIFIED` with a hash of the
  independent factual observation.
- If a factual observation contradicts the reviewed app, Lu records
  `CONFLICT` and **refuses** the result. The existing OS effect stays
  APPLIED—because the application did run—and a separate conflicting
  semantic observation is preserved. No automatic retry is triggered.
- If there is no independent checker, Lu records `UNSUPPORTED`. This
  does not block the prior reviewed read-only operation, but the result
  is plainly tagged as not independently verified.
- If the independent SQLite write fails, Lu must not publish the
  result as verified.

Model-visible normalized tool feedback adds only a small
`independentOutcome` classification (`VERIFIED` / `UNSUPPORTED`
and fixed verifier method ID). No path, arbitrary executable, raw
worker stdout, receipt, fingerprint or verification digest is exposed.

## Durable outcome receipt

The *same host-owned, per-original-turn* effect SQLite book now stores
a separate `lu_independent_outcomes` table. Each row is keyed by the
original worker effect call ID and SHA-256 bound to its **exact APPLIED
receipt bytes**. Only an existing matching APPLIED receipt may receive
a verdict. An identical attempt is idempotent; a conflicting attempt
under the same ID is rejected. The result is committed with SQLite
WAL/FULL; there is no automatic replay on Lu restart.

The E1-P09 Tool activity window can now display aggregate counts of
`independentlyVerified`, `outcomeUnsupported` and
`outcomeConflict` separately from APPLIED, UNCERTAIN, FAILED,
PREPARED and EXECUTING. Missing old outcome tables are interpreted as
**no independent verification**; they are never fabricated while
reading. Contradicted or uncertain effects override a model's
FinalAnswer. A `VERIFIED_AT_OBSERVATION` classification must not be
interpreted as fresh OS process termination evidence.

This is distinct from E1-P08C cgroup-v2 cleanup and does **not**
upgrade its process-tree guarantee, or assert that application side
effects outside the approved read-only resource were absent.

## Real native acceptance (OPEN pending receipts)

The dedicated exact-head workflow
`.github/workflows/lu-e1-p10-independent-outcomes.yml` requires:

- Windows 2025 and Ubuntu 24.04 **actual production Rust oracles**:
  detached/loose/packed Git HEAD, symbolic branch, injected Git ref
  path, true WAV PCM bytes, false claimed channels/duration, modified
  original bytes and deliberate unsupported operations;
- a real EffectJournal APPLIED receipt and durable immutable second
  observation across SQLite reopen, rejecting synthetic promotions,
  changed original receipts or contradictory duplicate outcomes;
- the desktop's production read-only SQLite aggregation and failure
  precedence, including pre-P10 journal compatibility and no raw
  application details crossing the WebView boundary;
- native **installed Git and FFprobe executables on both OS runners**
  through Lu's genuine owned process supervisor and model-tool
  catalog, checking five operations, their ten outer/inner effects,
  and **exactly three VERIFIED + two UNSUPPORTED** outcome rows;
- React source boundary tests, Windows Tauri native compilation and
  frontend build, full source-lock SHA validation and uploaded logs.

A script-only or source-only pass cannot close the actual OS
application evidence. The real model's autonomous tool choice (GGUF),
physical Windows installed application, escape-capable Linux cgroup
custody, independent Git status/history or MP4 source semantics remain
separate OPEN gates, to be assessed on their own exact heads.

## Remaining follow-on work

Add independent Git index/worktree and commit-object graph validators,
independent ISO BMFF and other audio codecs, stronger file-handle
identity/race custody for cross-process edits, per-assistant-message
provenance drill-down and live model/installed-app acceptance. Those
must retain per-original-turn grants and never infer independent
truth from model wording or a subprocess success message.
