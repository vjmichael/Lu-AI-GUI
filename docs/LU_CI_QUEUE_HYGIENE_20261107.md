# Lu CI-QUEUE-01 — stale-run hygiene and targeted Actions paths

## Scope and constraints

One-time cleanup is limited to **unstarted obsolete queued/pending runs**, not to
old but independently required feature branches. A run is eligible only when:
1. An older run is queued/pending/requested/waiting for at least 30 minutes.
2. A newer run in the same workflow, branch, event and PR exists.
3. The newer run is at the current **repository branch SHA**, independently
   resolved through GitHub's branch endpoint, and is not cancelled or failed.
4. The older run is at a different SHA and neither the newest run nor a run
   currently in progress.

The cleanup scanner reads at most 500 recent runs, accepts at most 25 cancel
requests per invocation, and never deletes workflow runs, logs or artifacts.
Run termination is requested using GitHub's standard Actions cancel endpoint,
not force-cancel. A 202 response is only a *request accepted*, not evidence
the run reached cancelled; inspect the exact GitHub run status afterward.

Manual dispatch defaults to dry-run; the dedicated source branch push requests
the guarded apply mode. The action requires only contents:read, actions:write
on the owner repository; a missing privilege fails instead of pretending work
was completed. This workflow **must not be merged** merely to clean past CI.

## Trigger fanout repair

- Removed `source-tree.lock.json` from the `pull_request.paths` triggers
  of 23 narrowly scoped feature workflows. Their meaningful source, test and
  workflow-file paths remain intact, and their jobs still independently verify
  the frozen source lock whenever they actually run.
- Added bounded PR paths and same-ref cancellation to the three previously
  unfiltered TOOLS-02 JSON, TOOLS-03 Git and TOOLS-04 Archive workflows.
  Changed native `lib.rs`, corresponding tests, source and Cargo manifests
  continue to trigger those suites.
- Preserved the always-on `Lu REPO-01` source identity gate and `Lu Desktop CI`
  on PRs. Integration workflows with intentionally broad `src-tauri/**` paths
  remain broad; this is a conservative improvement, not a guarantee of zero
  cross-PR duplication.

## Evidence and outstanding work

The cleanup selection's four local Python unittest fixtures passed, including
same-PR/current-head selection and explicit exclusion of running, newest and
cancelled-successor candidates. This is **not evidence that any GitHub job
was cancelled**. No direct cancel-run operation is available in the current
GitHub connector; actual cancellation will require a successful hosted
`Lu CI Queue Hygiene` action with cancellation receipts and follow-up
status checks.

The older E1-P03 real child-process, E1-P04B artifact-custody, and E1-P05 Qwen
benchmark gate remain open. Their exact-head worker/benchmark runs must not be
marked accepted, cancelled as obsolete, or deleted without independent evidence.
Physical installed Windows/NSIS/Vulkan validation remains deferred near v1.0.
