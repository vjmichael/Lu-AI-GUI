# FIN-03 — Protected single-file unstage

A registered ordinary repository with exactly **one staged change** and
no other dirty paths can produce a source-bound review. The native
host derives the single path (WebView cannot select one), authenticates
the exact HEAD, index hash and index blob and refuses conflicts, linked
worktrees, symlinks, unknown Git state or foreign index locks.

A separately invoked one-use native OS dialog precedes an index-only
libgit2 `reset_default` limited to that path. Pre-write intent and
post-write hashes are appended and fsynced to a fixed local journal.
Interrupted `prepared`/`publishing` records prevent silent replay.
It never checks out source, runs Git CLI/hooks, writes branch refs,
or acquires remote credentials.

This is a deliberately narrow first authorized Git mutation. Native
compiler/OS human consent, interruption/reconciliation, and full
multi-path unstage acceptance are still pending.
