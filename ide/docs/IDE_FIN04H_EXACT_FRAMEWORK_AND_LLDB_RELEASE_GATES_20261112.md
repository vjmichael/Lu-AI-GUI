# IDE FIN-04H — protected framework exact-case source and LLDB release gates
**2026-11-12 · Source-only implementation · NOT accepted for release**

## Source implementation in the existing FIN branch

The existing Rust libtest single-case path remains source-bound. The new
`pytest_exact_case:<saved-relative-test-file>::<saved-function>` and
`node_exact_case:<saved-relative-test-file>::<saved-test-name>` recipes are
new, intentionally restricted native JobPlan identities. They cannot
supply a shell command, executable, dynamic flag, arbitrary path, URI,
glob, test-name regular expression or writable source root.

The Tauri command `ide_run_protected_framework_test_case` accepts only
a registered project's saved conventional-test candidate, source SHA
and fixed pyproject.toml/package.json SHA. The host re-reads the saved
test-symbol inspection, manifest and clean Git HEAD, publishes the
original bounded immutable snapshot, checks the selected file against
its original SHA, and obtains a separate native OS one-use approval.
The original Windows AppContainer/Job or Linux cgroup/Bubblewrap
worker selects the previously pinned Python/pytest or Node runtime
for one exact case; no general package script, user command or
new executable interface is introduced.

The original test report parser accepts only selected pytest verbose
node-ID or Node TAP exact-name case rows. Missing or multiple observations,
truncated output and unreconciled native worker outcomes stay explicitly
uncertain. Case-level pass/fail values are framework **reports**, never
independent assertion verification or human task acceptance.

Static source symbol inspection is not framework enumeration:
Python parametrized/fixture names, Node nested or parameterized labels,
Rust duplicate binary cases and CTest constructed target lists remain
unresolved. In particular, **CTest exact-case dispatch is not enabled**:
a `TEST(...)` C++ source macro is not independently the name of
a built `ctest -N` case. Do not invent compiled tests or run a
source-hint as a CTest executable command.

## Release-grade LLDB event ownership and action control remain missing

The existing D5 first-party Windows LLDB native owner-event pipe,
independent PE/PDB review, D6B one-use consent state machine, D6C
post-step dispatch verifier and D6D desktop dialog exist behind
**debug-only CI/developer features**; they are not available as a
trusted installed release feature. The general release feature
`native-debug` does not inherit D6D authorizations.

Required implementation and acceptance dependencies, in order:

1. **Real live event custody**: package/review the patched first-party
   LLVM owner event source and native release worker. Prove the event
   originates from the protected debugger's *current* Job/SCM or
   cgroup, not from DAP JSON, a recovered file, a renderer token or
   a different debugger/target.
2. **One-use real human authorization**: separate per-command OS dialog,
   fresh native owner sequence, original source and plan identity,
   short expiry, durable Prepared/Approved/Consumed transitions,
   revocation on cancel/change and consume-before-dispatch. No
   reusable renderer/model consent.
3. **Independent results**: after a dispatched step/breakpoint, verify
   the *new* protected OS stop, original process/thread/RIP and
   separately re-read DWARF/PDB mapping against frozen source bytes.
   DAP ACK, adapter stack, a last-stop cache or a journal-only
   ResultVerified string is never sufficient. Variables require
   independently bound frame custody and redaction.
4. **OS-only interrupted-action recovery**: preserve original job
   and action journals, require a separate human recovery request,
   independently prove kernel Job Object/cgroup tree quiescence,
   cross-project/cross-supervisor consistency and unchanged original
   receipt bytes, and emit a separate immutable recovery witness.
   Never reissue a step/breakpoint/variables request after restart.
   Uncertain result remains uncertain even if the process is reaped.
5. **Exact-head/installed acceptance**: positive/negative release
   builds and physical Windows/Linux OS worker evidence; consent
   accept/refuse/expire/cancel; interrupt before and after DAP send;
   step/continue/stop source witness; corrupt/missing journal;
   changed original source; global marker race; owner restart;
   installed UI accessibility and restart. These have NOT been
   executed or accepted during this source slice.

FIN-01D's global marker and existing original-journal read-only
action audit remain mandatory; unresolved action or prior OS worker
blocks new protected debugger admission. FIN-02's production runtime
packaging is still deferred, so no release-grade LLDB authority
should be promoted merely to make a command callable.

## Testing/merge disclosure

Committed native Rust and Node static regression sources have **not**
been executed here; GitHub connector is available for source work
but no native Windows/Linux compiler, pinned runtimes, user dialogs
or original kernel custody are available to this coding session.
No CI batch, PR merge, installer acceptance or release is claimed.
