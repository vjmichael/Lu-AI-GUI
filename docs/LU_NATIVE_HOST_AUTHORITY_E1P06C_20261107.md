# Lu native-host authority — Access ≠ Permission ≠ Authority

**Decision:** Production Lu Core is a native desktop assistant, not a
sandboxed AI, Docker container, limited-root workspace, MCP-only agent,
AppContainer-constrained application or restricted account.

## The distinct ideas

- **Access:** Lu Core operates as an ordinary native user process and can
  inspect and operate the host computer with that OS user's capabilities.
  There is no blanket confinement to a predefined project or app list.
- **Permission:** a human and trusted host policy may permit an action or
  resource scope. Discovery of a tool or executable is not such permission.
- **Authority:** an exact model-proposed action requires the current
  authenticated human-user turn, effect, tool, scope and host-bound resource
  plus a durable revocable grant. Neither model text nor outside programs
  grant their own authority.

Windows Job Objects and Linux process groups/cgroups are reliability controls
for child process cancellation, monitoring and cleanup, **not sandboxing of
Lu**. Lu needs no mandatory MCP transport for native first-party tools.

## External programs versus Lu Core

Lu can use trusted local applications directly when the user/host grants
the action. Risk-based optional OS restrictions can apply to selected
untrusted third-party code, downloaded DLCs or debugger workers; they
must never be misrepresented as restricting Lu itself. A Job Object or process
group also does NOT demonstrate filesystem or network sandboxing.

## E1-P06C typed useful Git read operations

The host-approved external Git binary supports two exact, read-only actions:
1. HEAD revision through: git --no-optional-locks -C [approved-repo]
   rev-parse --verify HEAD
2. Tracked working-tree status through: git --no-optional-locks -C [approved-repo]
   status --porcelain=v1 --untracked-files=no

The host selects the absolute canonical repository root and reviewed
executable SHA-256. The model supplies neither the repository path nor arbitrary
CLI arguments. The original human turn must grant **both ReadWorkspace and
ExecuteCode**, and the separate host policy must approve. The Git process is
supervised with an exact operation, ≤16 KiB bounded private stdout and a
≤30-second deadline. The HEAD parser accepts only 40/64 lowercase hexadecimal;
status parses valid porcelain lines and records a changed-tracked-file
count, accepting empty stdout as clean.

No raw filenames or stdout are persisted in SQLite; only the normalized
result, stdout SHA-256/length, exact call identity and custody flags.
Failures, revocation or indeterminate completion remain UNCERTAIN and
are never blindly replayed.

Only ordinary local .git directories and symlink-free absolute paths are
included in this first adapter. Linked Git worktrees, reparse/symlink roots
and missing repositories are rejected. Path stat/canonical checks reduce
misrouting but DO NOT eliminate TOCTOU; stronger handle-pinned resource
custody is needed for future higher-risk write operations.

This external-app adapter does not supersede Lu's existing first-party
native Git reader. It proves the typed third-party application path while
preserving normal native Lu Core access. Git push, checkout, edits, arbitrary
shell, arbitrary app commands and generic model-driven process launches
remain disabled in this adapter until designed and authorized separately.

## Native acceptance gates

On Windows-2025 and Ubuntu-24.04, compile/run actual OS Git through this
host-reviewed path and upload genuine logs showing correct HEAD and clean/
dirty tracked statuses; compare actual index bytes before and after status
to show no optional index rewrite. Require host-veto, missing ReadWorkspace
grant, forged user turn, wrong scope, invalid/symlink repo and CLI injection
to be denied without an applied effect; duplicate call IDs cannot replay.
Persisted journal receipts must survive host restart.

E1-P06B real external process cancellation, timeout, revocation and
restart/recovery tests remain independently required. An exact source
inventory passing does not close a native acceptance gate.

**E1-P06C remains OPEN until real hosted Windows/Linux logs and artifacts
prove those checks.** Broader third-party read/write, media/Office tools,
publisher review and physical installed Windows/NSIS/Vulkan acceptance are
separate later phases.
