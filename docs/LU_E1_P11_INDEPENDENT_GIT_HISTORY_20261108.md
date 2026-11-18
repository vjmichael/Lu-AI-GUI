# E1-P11 — Independently verified Git commit history

Date: November 8, 2026

## Actual next deliverable

Extend E1-P10's **independent operation outcome** infrastructure, but
do not turn a reviewed Git command's successful exit or model's answer
into a claim that history was verified. This E1-P11 subtask is a
standalone, intentionally bounded first-party verifier for reachable
**loose Git commit objects**, including merges.

The user continues to select a local repository using the native
picker and grants one-turn, read-only authority. The model is offered
the same named zero-argument history-count capability as before. Lu
Core is **not sandboxed** and no generic shell, Docker, MCP, extra
file chooser or new model permission is introduced.

## Independent source mechanism

`src-tauri/lu-core/src/independent_git_history.rs`:

- Starts from a host-reviewed `.git/HEAD` reference resolved by
  E1-P10's direct source observer, not a model-supplied commit ID.
- Opens each loose object from `.git/objects/xx/...` with an explicit
  small size limit, refuses symlink/reparse components and decompresses
  native zlib bytes with a hard upper bound.
- Checks the complete Git object header/declared payload size and
  recomputes the **actual object ID**, using Git's standard SHA-1
  format for 40-hex repositories or SHA-256 for 64-hex repositories.
  The minimal SHA-1 implementation is for matching Git's legacy object
  representation only; it is not a recommended modern signature or
  security credential hash.
- Parses the actual commit's `tree` and every `parent` header,
  rejecting invalid hashes, duplicate parents, self-references,
  malformed object boundaries, and unbounded parent lists.
- Walks all reachable parents iteratively, counts **unique commit
  objects** (including merges without double counting) with a strict
  limit of 4,096 and a bounded pending queue.
- Does **not execute Git again**, run hooks, use the shell, trust
  `git rev-list` stdout as an oracle, or inspect arbitrary user paths.

A direct count that equals the previously received reviewed Git result
is `VERIFIED`, and a disagreeing count becomes durable `CONFLICT`
and blocks the model's result. This feeds E1-P10's existing separate,
receipt-SHA-bound, immutable `lu_independent_outcomes` journal and
the E1-P09 read-only tool activity summaries.

## Explicitly unsupported cases

The oracle refuses to call a history count independently verified
unless **every** reachable object is available and validated as a
loose commit. This means returning `UNSUPPORTED` for repositories
with packed-only commits, a shallow history, replace/graft semantics,
an external object-store alternate, a linked worktree, malformed or
oversized objects, unusual object encodings, and histories over 4,096
unique commits. Git tracked-status and MP4 validation remain separate
open work.

The actual supervised Git operation can still return its normalized
read-only result, but the model receives a fixed
`independentOutcome.status = UNSUPPORTED` rather than a false
verification claim. No automatic repeat or migration to a privileged
worker occurs.

## Independent acceptance matrix

`.github/workflows/lu-e1-p11-independent-git-history.yml`:

1. Exact-head immutable source checkout and source-tree lock on native
   Windows 2025 and Ubuntu 24.04.
2. Native Rust SHA-1 known-answer vectors, synthetic real zlib commit
   DAG and merge/ancestor deduplication; corruption, shallow history,
   packed replacement refs and malicious IDs must decline proof.
3. A genuinely installed Git binary builds a real **four-commit merge**
   repository on each OS; the independent oracle must report the same
   count **without itself invoking Git**, reject a forged history
   count, then become UNSUPPORTED after real Git packs/prunes the
   loose objects.
4. Separate native full reviewed model-tool loop using actual Git and
   FFprobe, the existing supervised OS workers, exact host-selected
   resources, two durable journal layers and the E1-P10 semantic
   outcomes. Five operations must yield **four independently VERIFIED**
   (HEAD, branch, history, WAV) and **one UNSUPPORTED** (tracked status).
5. Real native log artifacts; no PASS is inferred from source tests,
   job submission, previous commit results or a simulated process.

The test harness uses Git **only to create and ground its fixtures**.
The implementation under test does not call Git a second time.

## Remaining gates

Exact-head Windows/Linux test results, physical installed-Windows
acceptance, live GGUF model choosing the actual tool, stronger host
resource identity/race handling, independent index/worktree validation
for status, packed Git object traversal for broader history support,
and MP4 independent semantics remain OPEN until proven. E1-P08C
cgroup-v2 descendant custody remains a separate security guarantee.
