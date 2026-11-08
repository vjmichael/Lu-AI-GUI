# Lu E1 native acceptance — exact-head evidence integrity

Status as audited on November 7, 2026 (Arizona), corresponding to the
GitHub November 8 UTC Actions records. These entries are evidence gates,
not source-feature implementation claims.

## Independent native execution still missing

- E1-P03 real Windows/Linux child execution, task-revocation kill, OS
  reaping and restart: 6 ignored native process tests must execute.
  Original exact-head run: https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42444912163
- E1-P04B actual DOCX/XLSX OS child stdout custody, cancellation and
  publication: 5 ignored native process tests must execute.
  Original exact-head run: https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42444912254
- E1-P05C genuine local Qwen GGUF + two native arms: source SHA must match
  checked-out source head; raw `e1_p05_raw.json` must be present and
  contain actual paired task rows, executable and model digests,
  cross-language DOCX/XLSX witnesses and reproducible p50/p95.
  Candidate run at parity-corrected DOCX head:
  https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42446930912
- E1-P06B genuine Git/Python third-party process stdout, cancellation,
  revocation, timeout, broker-drop reaping and journal restart:
  6 ignored native tests. Original run:
  https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42447911228
- E1-P06C actual Git HEAD and clean/dirty tracked-status operations:
  3 ignored native tests. Original run:
  https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42448960329
- E1-P06D real Git branch/history and detached-HEAD plus P06C regressions:
  5 ignored native tests. Original run:
  https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42449349976

At this audit, each selected dedicated run remained in queued/pending
source-checkout state. No Windows/Linux native test job or benchmark
artifact was present. Do not treat cancelled superseded runs, ordinary
Desktop CI or a pure Git command-level test as native worker acceptance.
GitHub Actions status had no announced Nov 7 incident, but that cannot
identify the cause of this repository's scheduling backlog.

## Repairs prepared by this exact-head evidence branch

1. Six E1 workflow files now force `actions/checkout@v4` to the exact
   immutable `github.event.pull_request.head.sha` for PR events,
   falling back to `github.sha` on push/dispatch. Each job asserts
   `git rev-parse HEAD` against that SHA, avoiding synthetic PR merge
   commit attribution. Branch-keyed concurrency reduces duplicate
   push/PR queues for identical heads.
2. Dedicated P03/P04B/P06B/P06C/P06D workers must show respectively
   **6 / 5 / 6 / 3 / 5 actual passing ignored tests** in native log
   lines. Cargo exiting 0 after selecting no tests is not acceptance.
3. E1-P05 now uses the real checked-out commit for `--source-commit`.
   P05 p95 uses empirical nearest-rank:
   `sorted(samples)[ceil(0.95*n)-1]`, with sample count and estimator
   disclosed. With two trials it is the larger observation, not the
   smaller observation as in the original implementation. This is a
   small-sample measurement and should not be interpreted as a stable
   production tail latency estimate.
4. The independent `verify_e1_p05_raw.py` contract runs after real
   native-only and full three-arm execution. It rejects missing rows,
   duplicate tasks, wrong source SHA, absent GGUF/engine digest,
   unsupported DOCX native task, nonexistent independent OOXML
   semantic witness, missing host veto and inconsistent summary
   statistics. Seven small synthetic fixtures test this *validator
   contract*; they are not presented as native measurements.

## Required closeout sequence

Keep each original gate OPEN until its exact-head hosted matrix and all
required per-OS artifacts are available. Inspect compiler/test failure
logs if they appear; fix specific faults, then rerun the complete matrix
at the new final commit. Merge/reconcile stacked branches only after
all required evidence is green. Do not merge this provenance change as
a replacement for testing P03/P04B/P05/P06B/C/D.

Lu Core itself is a native OS-user assistant, **not sandboxed by default**.
Process-group/Job Object child supervision remains a reliability and
recovery control rather than mandatory confinement of Lu Core itself.
