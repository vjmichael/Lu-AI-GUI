# E1-P06D — read-only Git branch and history context

## Reason for adding this operation group

The first external Git adapter (E1-P06C) reads the repository HEAD and
tracked-file status. E1-P06D extends the same **typed, host-reviewed native
application** route with two useful workflow facts:

- **Current branch**: the exact host-owned Git command is
  `git -c core.fsmonitor=false --no-optional-locks -C [host-approved-repo] branch --show-current --no-color`.
  The result is a bounded, single-line branch identifier, or null for an
  intentionally detached HEAD. No arbitrary Git options, repository path or
  application binary can come from the model.
- **History count**: the exact command is
  `git -c core.fsmonitor=false --no-optional-locks -C [host-approved-repo] rev-list --count HEAD`.
  The parser requires a single nonnegative decimal u64 with no leading
  zeros or trailing junk; overflow and multiline output fail closed.

The existing HEAD and tracked-status operations remain intact. Each typed
operation still needs independent **ReadWorkspace + ExecuteCode** grants from
the authenticated original human turn, separate trusted host-policy
approval, a reviewed Git binary SHA and OS file identity, a host-approved
repository root and a durable effect journal. It runs under Lu's
existing Windows Job Object or Linux process-group supervisor. These process
owners ensure cancellation, deadline and reaping; **they are not a mandatory
sandbox on Lu Core**. Native Lu itself operates with its OS user's rights.

## Native OS verification

The dedicated workflow `.github/workflows/lu-e1-p06d-git-context.yml`
must run actual installed Git on Windows-2025 and Ubuntu-24.04 at the
exact source head. Its test matrix includes:

1. Strict parsed branch/count formats; rejects injected newlines, escapes,
   branch traversal syntax, bad decimal and numeric overflow.
2. Real disposable Git repository: actual HEAD, existing clean/dirty
   status, `branch --show-current --no-color` and `rev-list --count HEAD`.
3. Actual detached HEAD: Git returns successful zero-byte output, and
   Lu must journal null branch, rather than fabricate a branch name.
4. Existing host-veto, missing original read grant, forged model origin,
   wrong scope, repository symlink and injected command rejection.
5. Real OS process supervisor confirms exit/reap before an APPLIED
   journal receipt, and its output remains bounded/one-use.

Prior E1-P06C native acceptance (PR #117), P06B cancellation/timeout/restart,
P03/P04B worker custody and P05C real-model measurements remain independent
gates. Compilation/runtime acceptance cannot be claimed from code review or
from independent local Git CLI output.

## Source-level command check (not Lu acceptance)

Using the actual Linux Git executable in a disposable initialized repository
with an empty process environment, the four fixed command patterns returned
expected commit SHA, clean tracked status, branch name, and history count 1.
After changing a tracked file, status returned its change line; after checking
out detached HEAD, branch returned empty output with status code 0. This is
useful command-contract evidence **only**. It does not compile the Lu Rust
broker, prove Windows Job Object reaping or discharge any native CI gate.

## Next application-adapter scope

Once exact-head native results pass, broaden beyond Git using a similarly
typed application-specific protocol and independent output parser—for example,
read-only `ffprobe` media facts, with a separately approved input file and
format schema. Do not create a model-selected generic process runner, do not
require Docker/MCP, and do not sandbox Lu itself.
