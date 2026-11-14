# FIN-03 — Local stash inventory (not mutation)

Adds an exact registered-repository native `refs/stash` reflog read.
Only indexed stash commit IDs are exposed; no messages, source contents,
Git process, index write, stash pop, apply or drop. No ambient URL,
path or executable is selected by WebView. A busy/detached repo or
source-HEAD drift fails closed. Results are capped at 32 entries.

This does not close the remaining Git mutation backlog, which requires
distinct reviewed one-use authority, durable journal and recovery.
Native, CI and installed acceptance deferred.
