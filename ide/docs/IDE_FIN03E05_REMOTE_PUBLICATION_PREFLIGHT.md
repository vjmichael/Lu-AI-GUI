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


## 6E-05B — Native-only authenticated LIVE read (Nov 13)

Implemented the next, strictly read-only native module
`ide/host/src/github_remote_live_policy.rs`.

- Takes **only** a registered project ID and a `NativeGitHubReadCredentialVault`
  implemented by the trusted native OS host. It never accepts a model token,
  user-controlled URL, Git configuration credential, cookie, account ID,
  WebView message, arbitrary HTTP method or command string. The OS keychain
  adapter itself is **not provisioned or accepted** yet.
- Executes **six bounded, no-redirect GETs** solely to fixed HTTPS
  `api.github.com/repos/{owner}/{repo}` endpoints: exact current
  `git/ref/heads/{branch}`, branch protection, applied rules for the
  branch, then the same three again.
- Requires each ref body to identify the exact `refs/heads/{branch}`
  with a 40-hex commit SHA. Ref changes, JSON/protocol errors, status
  errors, paginated/incomplete responses, protection/ruleset shape errors
  and policy drift reject rather than infer an unprotected repository.
  Rechecks the original registered local branch/HEAD/index/tracking
  evidence *after* all reads.
- The result is **observation**, not authority: it always reports
  `liveRefCompareAndSwapAuthorized=false`,
  `currentRemoteWriteLease=false`,
  `remoteWriteRouteAvailable=false`,
  `nativeApprovalVerified=false`,
  `remoteMutationPerformed=false` and
  `installedAccepted=false`.
  A stable six-read snapshot can immediately become stale and cannot
  replace an atomic remote ref compare-and-swap.
- Includes pure hostile-response tests for a moved ref, altered applied
  rules, truncated/inapplicable policy, malicious branch input and
  malformed response. An exact-head Windows/Linux CI batch checks
  the module and existing preflight.

**Actual authenticated live GitHub acceptance:** NOT YET ESTABLISHED.
The credential-vault interface is deliberately host-only; no linked OS
keychain implementation is supplied, and no credentials have been used.
Do not call this `6E-05B complete` from unit tests. The remaining
6E-05B integration must add native Windows Credential Manager/Linux
secret service custody and independently prove real API read-only
behavior against a user-authorized test repo; then move to 6E-05C
original one-use physical authorization, atomic publication receipt,
concurrent-writer refusal and recovery.

**Independent FIN-04 gate:** FIN-04W2-C original Windows six-phase
transaction remains blocked. New W2-C2 work must be isolated from
6E-05B and must not admit LLDB or W2-D.

## 6E-05C-01 — Offline publication intent and restart denial (Nov 13)

**Source implementation, not remote publication.** A new host-internal module
`ide/host/src/github_remote_publication_intent.rs` takes the original
registered-project read-only preflight and the separately observed 6E-05B
six-GET read result. It independently reopens the exact local repository and
requires:

- same registered project, canonical origin, branch, HEAD, clean index,
  snapshot digest, non-divergent tracking ref and no foreign Git lock;
- a freshly observed exact ref and stable protection/ruleset result, with
  all 6E-05B authorization and mutation booleans still false;
- the observed remote commit must already exist in the registered local
  object database and be an **actual ancestor** of the new local HEAD
  (not merely a GitHub-provided `localIsDescendant` Boolean).

The resulting intent pins exact old and proposed commit IDs, snapshot hash,
policy hash, source project/repository/branch and timestamp. Its typed grant
fields serialize exclusively as **false**, including publication, native OS
approval, CAS and restart retry. Host-only `create_new` journal preparation
stores a bounded hash-correlated JSON record, calls `sync_all`, rejects
duplicates, and on restart requires **manual reconciliation with no
automatic remote write**. Corrupted/mismatched records reject. Directory
ACL attestation, parent-directory durability, physical OS consent, actual
remote CAS and after-write ref readback remain separate, OPEN gates.

Real Git hostile fixtures cover an observed ancestor, a stale/moved ref,
forged approval and dirty project, plus duplicate prepared journal and
tampered restart denial. The original Windows/Linux 6E-05A/B tests remain
unchanged and are run in the same batch. **No credential usage, remote
write, native OS approval, Git process or installed acceptance** was done.

## 6E-05D — No-retry remote recovery classifier (source, Nov 13)

An interrupted/prepared journal is never an authorization to retry.
Independent native readback must match the exact original project, branch,
source preview and policy digest and be fresh before classifying its remote
tip. Three exclusively **nonterminal, non-writing** results are possible:

- original remote tip unchanged — reconciliation required, **no retry**;
- proposed tip visible — potentially another writer, **not proof Lu pushed**;
- unknown third-party tip — competing-writer quarantine, **no overwrite**.

A changed original journal, mismatched ref/policy, malformed digest or stale
readback denies classification altogether. The code contains no push, token
transport or OS approval path. Native positive remote publication and
cancellation/service death are not tested or accepted here. These tests
only establish conservative offline recovery logic.

## Group 6F-01 — Consolidated original Git recovery veto (Nov 13)

The native module `ide/host/src/git_security_recovery_matrix.rs`
independently reopens the registered ordinary Git repository, reads a
bounded original index SHA256, direct branch HEAD, Git operation state,
foreign lock state and bounded worktree status, then consults the **original**
6B reviewed commit, 6C fast-forward merge, 6C divergent merge, 6C
cherry-pick/rebase, partial stage, conflict resolution, unstage, single-file
restore and stash recovery inspectors. Any inspector error, mismatched
project, schema, unexpected automatic retry/rollback flag, missing/duplicate
domain, foreign lock, dirty worktree or source drift fails closed.

All original journals retain their independent receipt/reconciliation
semantics. A clean/no-pending cross-domain result is an inspection snapshot,
**never** an executable permission, cached OS dialog, GitHub publish grant,
lock removal or installed acceptance. New real-Git hostile fixtures verify
clean nine-domain recovery, foreign lock/dirty worktree rejection, and
single-domain uncertainty overriding other clean domains. The original
Windows/Linux host CI is extended with this module's tests, including
optional OS vault compilation.

Group 6F still requires native/interrupted-process security matrices,
actual authorized remote CAS lifecycle, Windows/Linux race and crash
adversarial results, and installed recovery. Group 6G remains open.

## Group 6F-02 — Cross-domain remote restart gate (source)

The Group 6F inspector now joins the four original local Git recovery
results with the exact 6E-05C prepared-only journal and an independently
supplied, fresh 6E-05B observed remote state. It checks local project/HEAD,
journal/intended commit, freshness, original policy digest and local source
again after the remote comparison. An unchanged tip, a tip matching the
proposal, or a competing writer are ALL classified as requiring operator
reconciliation. **No case permits an automatic push, asserts a completed
publication, reuses consent or releases an OS credential.** The actual live
credential read and remote CAS are deliberately outside this module.

## 6E-05B physical native credential read diagnostic (Nov 13)

Corrected three `cfg(any(windows,linux))` guards to
`cfg(any(windows,target_os="linux"))`. The former is not a reliable
Rust operating-system target predicate: Linux compile jobs could claim
feature completion while the real OS vault type was omitted.

Added `ide/host/examples/fin03_live_vault_probe.rs` compiled in the
original Windows/Linux feature-gated native CI. With an independently
provisioned OS keychain entry, a local operator can run this **diagnostic**
against an already saved and registered IDE project:

`cargo run --manifest-path ide/host/Cargo.toml --features fin03-native-vault --example fin03_live_vault_probe -- <absolute-existing-catalog.json> <existing-project-id> --confirm-os-keychain-read`

The diagnostic only reopens an already registered project, borrows a fixed
repository-scoped token from the OS credential vault, sends 6 read-only
pinned GitHub GETs, and prints **sanitized PASS/FAIL codes**. It never
prints/stores a token or response body, requests credentials, pushes
a ref, edits the project or claims installed acceptance. An OS keychain
item has NOT been provisioned or accessed by this CI run. A physical
operator must independently provision/approve any actual live test.

## Group 6F-03 — Original native Git mutation admission enforcement

Before each existing original approved Git write method reaches its first
mutation point, it now invokes `require_group6f_no_pending_recovery`.
This covers reviewed commit, fast-forward/divergent merge, cherry-pick/rebase,
partial staging, conflict resolution, unstage, single-file restore and stash
save. An unresolved original operation journal, foreign lock or active Git
operation vetoes another operation. Conflict-resolution alone may work in
a pre-existing Git conflict state, subject to its unchanged original
candidate/source/one-use checks; it cannot waive other pending journals.
Ordinary dirty files are NOT globally blocked, because partial staging,
restore and stash legitimately operate on modified sources.

This new gate does not replace or mint original OS approval, lock/CAS
witnesses, private file receipt or native installed consent. Exact-head
Windows/Linux regressions and hostile cancellation/recovery remain pending.
