# FIN-03A — Same-origin public GitHub reviews

Implemented a native read-only endpoint requiring the registered project's
exact public GitHub origin and an open PR on the **same local HEAD**. Only
the PR number can be submitted from WebView. A bounded unauthenticated
request with no redirects inspects latest reviewer decisions, distinguishes
stale commit reviews and dismissals, and refuses positive approval claims
when pagination is incomplete. A second native PR observation checks that
origin, branch and SHA have not drifted. The GUI displays the counts
without interpreting them as branch-policy or merge approval.

FIN-03 remains **open**: 6E-03 remote branch/branch-protection evidence,
6E-04 full check/artifact receipt binding, 6E-05 carefully authorized
remote publication, and typed local revert/stash/unstage/worktree
management have not been implemented. Protected remote operations require
a separate native credential and OS consent design; no unsafe plain Git
command or token handling has been introduced.
