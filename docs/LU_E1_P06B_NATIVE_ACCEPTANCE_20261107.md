# E1-P06B — real host-supervised third-party version execution

## Execution boundary

The host-selected external **Git** application is a real installed third-party
binary, not a synthetic worker. Its reviewed canonical file bytes and SHA-256
are validated under the existing original-user-turn ExecuteCode grant and
host policy, and revalidated by the OS process supervisor just before spawn.
The only production operation is the immutable host-owned --version or
-version argument chosen in the approved adapter record. No PATH scan,
untrusted plugin manifest, scripts, arbitrary flags, model-provided command,
working-directory selection, stdin, model-supplied PID, shell or external
file publication is admitted.

The process is started through BackgroundProcessSupervisor with an empty
environment, null stdin and stderr, a local bounded stdout pipe (1–4096 bytes),
a 1–15s deadline, Windows Job Object or Linux process-group ownership, and
the supervisor's 256 MiB resident-memory policy for this special version
capture. Linux process-group ownership is not equivalent to a verified
kernel-level network/filesystem sandbox; real cgroup attachment and Windows
Job memory enforcement must be observed in native acceptance.

The version reader verifies **actual OS-owned child stdout** and rejects
missing, oversized or unreadable data. The broker independently checks a
single strict version line for the host-reviewed app family, such as Git's
'git version 2.47.3' or Python's 'Python 3.13.5'. It rejects control bytes,
multiple lines, fake prefix and unacceptable version characters. The raw
output stays in one-use host memory, is zeroed when discarded, and is never
stored as chat history or a model prompt. SQLite stores only normalized
version, hash, length and custody flags.

Success is marked APPLIED **only after** the actual child exited 0, stdout
was captured within the declared bound, the OS supervisor reaped the child,
the appropriate independent parser accepted the single version line, and
the durable journal receipt was written. Failed launch, host veto, missing
output, nonzero exit, oversized output, timeout, revoked authority, killed
child and ambiguous recovery cannot publish a success receipt. Duplicated
call IDs are never automatically replayed.

## Native host acceptance design

The CI matrix deliberately uses **actual host-installed binaries**:
- Git selected by the trusted runner's Python shutil.which('git') and
  canonicalized to an existing real absolute OS executable. Git --version
  must produce a genuine bounded output; the test asserts TaskStore
  COMPLETED, journal APPLIED, parsed version and stdout SHA-256, no remaining
  supervisor-owned process, duplicate-call denial and persistence across
  journal reopen.
- Python 3.13 selected via a trusted setup-python installation and exact
  canonical sys.executable. For cancellation/revocation/timeout only, Rust
  unit tests compile a **test-only** fixed Python '-I -S -c' sleep profile.
  This special profile is not in release binaries. It intentionally never
  completes successfully during those tests, proving that the broker sends
  OS-owned cancellation to the real interpreter, denies model-forged task
  origins, reaps the child, marks task CANCELLED or TIMED_OUT, marks journal
  UNCERTAIN and prevents re-execution/replay after host restart.
- Dropping the actual Lu broker while the external Python process is still
  running must terminate and reap that child. After reopening both SQLite
  journals under exclusive host ownership, the same call ID remains UNCERTAIN
  and cannot be prepared or executed again.
- A separate test-only fixed Python process emits 8192 bytes against a
  512-byte output limit. Its entire stdout must be denied, no APPLY receipt
  issued, and no output exposed after worker cleanup.
- The ordinary adapter/preflight and stdout parser tests also cover host
  policy veto, effect separation, revoked grants, unknown adapter and
  control/multiline output rejection.

CI: .github/workflows/lu-e1-p06b-real-thirdparty.yml. It uploads per-OS
logs including the real Git normalized-version observation and exact Rust
test results. Each run must be tied to its actual source head SHA; a GitHub
synthetic PR merge checkout SHA is not a substitute for the source SHA.

## Honest security and verification limits

This feature is not a general third-party application sandbox, no MCP
required, no signature/publisher legal attestation, no visible desktop GUI
interaction, no independent network isolation test, and no arbitrary
document/media operation. A matching reviewed digest plus OS containment
is not sufficient to grant app permissions; the user-task ExecuteCode and
host policy remain independently necessary each turn.

**E1-P06B source candidate is not accepted until genuine Windows-2025 and
Ubuntu 24.04 process tests have completed and passed**, with complete logs,
exact external binary/version/hash proof, Windows Job / Linux process-group
termination, no surviving owned children, and durable journal observations.
Physical installed Windows/NSIS/Vulkan remains deferred until near Lu v1.0.
No benchmark comparison or late stdout response may be substituted for
native execution.

## E1-P06C architecture clarification — Lu is not sandboxed

Production Lu remains native and has the ordinary OS user's host access. **Access ≠ Permission ≠ Authority**: Job Objects and Linux child process groups provide lifecycle management, while authenticated user-turn grants govern tool action authority. Neither is a mandatory sandbox around Lu. Risk-based optional restrictions for selected untrusted external code are a separate design choice, not the default constraint on the native Lu assistant. See docs/LU_NATIVE_HOST_AUTHORITY_E1P06C_20261107.md.
