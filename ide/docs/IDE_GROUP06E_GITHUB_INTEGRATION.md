# IDE Group 6E — GitHub integration register

Group 6E follows 6D conflict resolution and precedes 6F security/recovery and 6G native acceptance. Work remains isolated in the IDE portion of `vjmichael/Lu-AI-GUI`. GitHub is the user's repository, not a CI-only gating requirement for continued source development.

## 6E-01 — Read-only exact-origin pull request inspection

- Build on existing `github_live` read-only local/remote branch comparison and `github_ci` exact-HEAD CI observation; do not replace their evidence semantics.
- Derive repository from a canonical registered local Git checkout's validated `origin`, and branch/HEAD from local Git refs. No arbitrary URL, custom host, model string or browser-supplied remote is accepted.
- Make a single explicit human-triggered unauthenticated HTTPS GET to `api.github.com`, with no redirect, credentials, network subprocess, Git fetch, ref update, PR comment/review submission, or branch publication. Public repositories only.
- Bound response size, time, PR count and displayed title. Match returned PR head and base to the exact origin repository; require the selected branch and SHA shape. Construct GitHub PR URLs from validated source identifiers, never trust server-supplied `html_url`.
- Re-read source repository, branch, HEAD, and origin after the network call. Label different SHAs **ancestry unknown**, and missing/partial PR listings as **not proof of absence**. PR metadata never signifies review approval, passed CI, task completion, authorization or a safe merge.
- Implement native host, Tauri invoke, IDE React panel, and hostile source-boundary + pure Rust parser fixtures.

**Status:** source implemented in draft. Real native Windows/Linux, installed interaction and public-network responses remain unverified until original native logs exist.

## Subsequent Group 6E slices (proposed engineering split)

- **6E-02 — Review state inspection:** distinguish pending/approved/changes-requested, stale review commits, user identities and untrusted comment text. No review submission.
- **6E-03 — Remote branch candidates:** compare exact remote refs, protected branch settings and ancestry locally without any remote publication authority.
- **6E-04 — PR checks and CI receipts:** host-verified job identities, correct SHA and run IDs, partial/pending/failed distinctions and verifiable artifacts; do not confuse GitHub workflow status with real installed-human acceptance.
- **6E-05 — Protected remote actions (if separately authorized):** separate one-use native OS confirmation, exact ref CAS, durable journal, cancellation and readback. No silent push, PR review, merge or branch deletion.
- **6E-06 — Consolidated native/installed acceptance:** one explicitly triggered four-job Windows/Linux native and Windows NSIS/Linux Debian installed matrix; physical-human approval separately deferred.

## Constraints

Group 6D PR #221 is source implemented but its native matrix/installed acceptance are still pending. Group 6E stacks as a separate draft PR; no merge to `main`, release, forced remote publication or broad automatic CI fanout. Do not add a per-commit 6E matrix; batch only after the relevant source work is ready. The existing physical-human acceptance deferral in issue #218 remains in effect.
