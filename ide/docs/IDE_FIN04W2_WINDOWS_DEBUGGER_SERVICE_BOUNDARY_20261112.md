# FIN-04W2 — Windows kernel-supervised debugger service boundary

Status: **Engineering design / NOT authorized for production execution**
Owner task: IDE FIN-04 native debugger; successor to FIN-04W restricted-token experiment.
Source/receipt evidence: PR #227; FIN-04W original native run 43381432237.
No source branch merge, installation, elevation, debugger command or UI exposure is authorized by this document.

## Why FIN-04W1 is retired as a launch strategy

The **real** Windows kernel accepted a disposable `NT AUTHORITY\RESTRICTED` SID
with `CreateRestrictedToken` but rejected the job-specific AppContainer SID,
the All Application Packages SID and their combination with Win32 error 87.
Separating LUA into two consecutive `CreateRestrictedToken` calls still
failed when adding either class of package restricting SID. In native output:
`nt_restricted_sid=accepted_never_launch`,
`package_sid_only=rejected`,
`all_packages_only=rejected`,
`two_stage_exact_restricting_sids_denied`.
The temporary NT SID token was closed immediately and provides no authority.
The inability to construct the intended two-SID token is **not** grounds to
substitute NT RESTRICTED alone, give BUILTIN Users/Everyone access, weaken
package or filesystem ACLs or launch an uncontained debugger.

## Replacement execution boundary: FIN-04W2

Use two isolated trust domains and the existing authenticated SCM service
and native Job infrastructure; retain AppContainer for the untrusted IDE
host/UI side. The debugger's Windows-native debug APIs will execute **only**
in a separate, OS-restricted, service-principal worker owned by a new
reviewed service-specific job, not by a token minted from the host user's
primary token.

1. **SCM identity**: reuse the proven native restricted LocalService service
   authentication pattern (`windows_service_security.rs`) but make the
   debugger worker identity and executable hash separate from the recovery
   broker. The service process token MUST be kernel-attested against its
   installed SID, original binary hash and service principal. No LocalSystem,
   interactive user impersonation, UI process handle or ambient host token
   may become the LLDB worker token.
2. **Immutable source/binary**: the service accepts only an already-reviewed
   LLDB-DAP digest and one original, completed, journal-bound Cargo debug
   target; the source_head, snapshot SHA, manifest SHA, target binding SHA,
   build-terminal receipt and debugger plan hash must all match. Stage only
   those bytes in a private OS-ACL directory accessible to the service
   identity. No relative traversal, host-user home mount, foreign exe,
   arbitrary environment or caller-supplied process ID.
3. **Kernel process custody**: create an initially empty, separately named
   Windows Job Object with exact service-owned lease; apply bounded memory,
   process-count, CPU and kill-on-close limits before the child resumes;
   ensure the debugger and all debuggee descendants remain in that Job.
   A failed enrollment or guardian death must force termination and a
   durable uncertain/recovery-required state, never a false terminal.
4. **Network and filesystem denial**: before any execution approval, verify
   independent Windows-kernel tests under the actual worker token: attempts
   to read a seeded unrelated user file, mutate foreign source, create
   unreviewed children, open external network sockets, or use named-pipe
   escape paths must be denied. The service's restricted account alone does
   not establish network isolation: a scoped OS firewall/WFP-denial rule
   must be provisioned, independently read back and tested from the child.
   If this cannot be established and revoked reliably, the worker MUST NOT
   be admitted. No registry or global ACL widening is permitted.
5. **Authenticated fixed IPC**: the AppContainer-side host may send only
   typed bounded requests tied to the reviewed plan, one-use creator
   authorization and original journal receipts. The independent service
   verifies client process token, installed-client digest, fresh challenge,
   original job binding and receipt. No raw DAP passthrough, arbitrary
   command name, shell, user-selected PID/path, network request or generic
   service method. Reject all reverse adapter requests.
6. **Debugger evidence**: successful `initialize` is not a stop. Require
   original kernel thread/RIP, Job membership, event-owner continuation,
   sealed PE image base, independent PDB line mapping, exact source snapshot
   and both fsynced journals before enabling any reviewed step/inspection.
7. **Revocation and recovery**: one-use grants consumed at native admission;
   revocation races must terminate the job and leave durable receipts.
   Cancellation must persist intent BEFORE kernel termination, then
   independently verify tree-empty/service release before `Cancelled`.
   Restart cannot replay an old grant, resume an unknown process or
   invent a completed stop.

## Engineering sequence and executable acceptance gates

- **W2-A, protocol and service identity**: define strict fixed request/receipt
  schema and independently installed identity. Negative cases: forged client
  token, old/nonmatching receipt, foreign source, duplicate authorization,
  malformed method, unknown PID/path, adapter reverse command. Must all deny.
- **W2-B, OS worker and ACL/WFP proof**: launch reviewed binary as restricted
  service principal into service-owned empty Job, prove actual child token,
  target/sealed tools readable, unrelated user file/network unreachable and
  no descendant escapes; guardian-loss/kill/revocation verified.
- **W2-C, genuine native source debugger**: original project Cargo build and
  independent PE/PDB; LLDB project stop, OS thread/RIP, owner-event and
  service Job/SCM/receipt equality; no success on DAP claims alone.
- **W2-D, full hostile/recovery matrix**: one-use approval/replay, unexpected
  DAP methods, cancellation before and after stop, service death, parent
  death, restart, journal tampering, protected file/handle leakage, network
  denial and OS process-tree cleanup. Verify on actual Windows kernel.
- **W2-E, one-SHA reconciliation**: successful native Windows W2 and Linux
  source stop, DWARF, cancellation/recovery using the same final source SHA;
  only afterward consider installed-human/owner authorization.

## Existing evidence to preserve

- Original Windows recovery SCM service is intentionally restricted
  LocalService, not LocalSystem. Original independently authenticated pipe
  and original Job lease are usable foundations, but the new debugger service
  requires separate identity, protocol, and original tests; this design
  document does NOT claim it is already wired.
- Linux protected project debug on run 43381432373 successfully produced
  the native stopped event, thread, stack, kernel PC/cgroup, sealed source
  and independent LLVM/DWARF original line under a bounded 48-task project
  debugger cgroup. Subsequent cancel evidence initially failed only because
  the fixture erroneously expected `CancelRequested → Cancelled` adjacent;
  the original supervisor requires `CancelRequested → ReleasePending →
  Cancelled` with independently verified release. The corrected fixture
  must pass a new native exact-head run before full acceptance.

**Explicit prohibition**: this plan does not enable a feature flag, service
request, kernel debugging authority, UI control, installation, or merge.
FIN-04 remains open until executable native evidence proves each gate.
