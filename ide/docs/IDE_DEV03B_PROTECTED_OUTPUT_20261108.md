# DEV-03.3B — Actual protected worker output readback

Source branch: `ide/dev03b-protected-output-streams-20261108`, stacked on
DEV-03.3 phase progress exact `f8590b5ee735d7d62cc7b027565395879012164b`.

## Implemented contract
The existing one-use human-approved supervisor, verified frozen source,
platform worker admission and cancellation remain the ONLY execution path.
No new subprocess executor, native filesystem path argument, shell, model
authority or authorization record was created.

`LiveOutput` is an in-memory, native-worker-written rolling window
(max 64 KiB and 256 chunks). Each chunk has a monotonic cursor and fixed
stream identity; each read returns no more than 64 chunks. It is **advisory
untrusted text**, not the durable terminal receipt, accepted process cleanup
or a verified artifact. It is not journaled, never resurrected on restart,
and the host holds at most one active job and one last finished ephemeral
window; a subsequent run replaces the previous window.

**Linux**: the original bounded `collect_limited` stdout/stderr drains
each contribute bytes to the host-owned buffer after cgroup membership and
the native Running transition. Bytes beyond the existing per-stream log
budget are discarded; stdout/stderr retain their distinct stream labels.
The original digest/truncation/exit/cgroup cleanup flow remains.

**Windows**: existing AppContainer/Job/SCM Cargo launcher directs both OS
handles to the same `cargo-diagnostics.log`. The worker opens this **fixed
contained file** read-only (new sharing restriction allows native readback)
during its existing kernel wait loop, caps each poll to 8 KiB, checks native
file metadata and the same output policy limits. It reports stream
`combined`, never fabricated separate stdout/stderr or strict interleaving.
A failed structural read quarantines rather than granting success. The
original terminal output seal and descendant termination flow remains.

**Native readback**: `ide_build_read_output(projectId,jobId,after)`
requires native registered project authority and exact in-memory
active-or-last-finished job identity. It never accepts a path, recipe,
executable, plan override or grant. Readback is unavailable after restart
and unsupported builds deny it. User-facing UI uses untrusted text nodes,
limits byte count, and never promotes logs into accepted outcome.

## Open acceptance
- Exact-head Windows/Linux compiler tests and source tests must pass.
- Real protected Job/cgroup execution must demonstrate true incremental
  stdout/stderr bytes, honest merged Windows provenance, disconnection,
  truncation, cancellation, timing races and worker crashes.
- Windows AppContainer sharing/ACL interactions and read-after-write races
  require genuine native testing.
- Installed NSIS/GTK UX acceptance and accessibility/clipboard snapshots
  are not established.
- No logs are advertised as secret-sanitized; they reflect untrusted
  code output from a human-approved isolated job. Permissions and source
  snapshots, not log string filtering, determine authority.
