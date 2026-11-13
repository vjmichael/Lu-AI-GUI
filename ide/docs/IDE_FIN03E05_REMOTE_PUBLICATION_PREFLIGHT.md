# FIN-03 / 6E-05A — Protected remote publication preflight

**Status:** Source implementation only. Default-deny; no remote mutation,
credential acquisition, network request, execution route, OS consent, CI
acceptance, installed acceptance or automatic retry.

## Purpose

The remote mutation backlog follows read-only 6E-01 PR identity, 6E-02
review evidence, 6E-03 partial branch rules and 6E-04 exact-head CI
correlation. Those sources must **never** become an implicit remote write
grant. A same-origin PR and green CI are observations, not publish approval.

This slice adds a native-only, project-ID-only API
`IdeWorkspaceRegistry::review_github_remote_publication(project_id)`
in `ide/host/src/github_remote_publication.rs`:

- Opens **only** the OS-picker registered ordinary Git checkout, denying
  linked worktrees, alternate `.git`, detached/unborn HEAD and invalid
  local branch names, using the existing `exact_repo` guard.
- Derives a canonical GitHub owner/repository solely from verified
  `origin`; rejects arbitrary hosts and reports explicit pushurl
  overrides as blockers. The origin is not accepted as credential proof.
- Snapshots the exact current local branch, direct commit HEAD, bounded
  SHA-256 of the original Git index, Git repository operation state,
  dirty/untracked count, foreign Git lock presence and any locally stored
  `refs/remotes/origin/<branch>` commit. It **never** contacts GitHub,
  runs `git`, reads credential helpers, or mutates Git objects.
- Reports whether HEAD is equal to, descended from or divergent from the
  **stored** tracking ref. A locally observed fast-forward is **not** a
  verified current remote ref and is **not** a safe force/push decision.
- Reobserves all source inputs after analysis; source drift denies the
  entire preview. Emits a stable, scope-bound digest for continuity only.
  That digest is **not** a token, approval, write lease or authority grant.
- Always reports `remoteWriteRouteAvailable: false`,
  `remoteMutationPerformed: false`, `nativeOneUseApprovalGranted: false`
  and `liveRemoteRefVerified: false`, including in clean fast-forward
  cases. Every preview explicitly lists missing live-ref CAS, ruleset
  knowledge, credential custody, independent one-use human consent and
  fsynced publication/restart journal.

## Next distinct gates

**6E-05B** Trusted native-host-only GitHub credentials and a bounded
exact-ref source/readback design, with independently verified rulesets
and a negative fixture for stale/different remote tip. No WebView, model,
project file or content string may select a GitHub credential, remote
repository, endpoint or executable.

**6E-05C** Separately approved, one-use native OS dialog for the exact
source/target/ref and effect; append-and-fsync admission and publication
receipts before and after an independently verified remote compare-and-swap.
Never accept network latency, timeout, 401/403 or successful HTTP status
alone as an independently verified publication.

**6E-05D / 6F** No duplicate remote write after timeout, process death,
revoke, app restart, competing writer or incomplete journal. Reconcile
the original ref and receipt without replaying credentials or consent.
Only then combine Group 6A–6E security/recovery and Windows/Linux native
and installed acceptance. Real physical human approval remains deferred.

## Acceptance

New unit tests exercise registered-root origin integrity, clean local
source identity, local-only ancestry distinction, dirty worktree and
foreign lock denial, and malicious origin or alternate push URL rejection.
At this source commit **tests have not yet been run**. Do not promote
this to a functional remote publication feature from source alone.

**Unrelated blocker preserved:** FIN-04W2-C native original Windows
six-phase benign-worker transaction failed with
`SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED` on
[run #43483699020](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43483699020).
LLDB and W2-D are not authorized. This independent FIN-03 branch does
not change PR #227 or its original acceptance conditions.
