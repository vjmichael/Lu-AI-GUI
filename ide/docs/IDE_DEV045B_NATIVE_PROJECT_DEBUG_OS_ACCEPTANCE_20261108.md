# DEV-04.5B — Real protected project Cargo-to-LLDB OS acceptance

**Status: implemented test harness and wired native CI; acceptance PENDING.**
Stack: DEV-04.5A PR #171 → DEV-04.5B PR #174.

## Invariants

- A Cargo **build** and a project **debug** are separate native jobs with
  separately reviewed exact project/source/snapshot/manifest/plan digests.
  A completed build's approval cannot be reused to launch a debugger.
- Only a **Completed** zero-exit protected Cargo job with verified terminal
  receipt, audited output tree, exactly one executable and structural
  ELF/DWARF or PE/RSDS/PDB matching witness can supply a debug target.
- Project binary and symbols are sealed in a newly owned, private, native-only
  target stage; independent rehash is required before DAP admission.
- LLDB-DAP must run in actual delegated cgroup-v2/Bubblewrap custody (Linux)
  or an actual AppContainer Job Object with independently installed SCM
  recovery authority and SID-bounded native rights (Windows).
- A real initialize, launch, stopped thread, threads response and stackTrace
  response are mandatory. Receiving a synthetic DAP JSON sample or passing
  source-only test does not count.
- Native close/cancel must leave a durable terminal receipt, independent OS
  descendant release, no source mutation, no stale stage, and no replay of
  either one-use approval across journal reopen.
- Test harness fixtures self-measure CI-provisioned binaries and simulate
  fixed test consent. None of these tests authorizes arbitrary user projects
  or grants the normal unsigned installed desktop native execution.

## Implementation and execution

**Linux:** `ide/supervisor/examples/wo6_linux_native_build_acceptance.rs`
is run as `--project-debug` through the fixed
`linux_delegated_ci_wo6_build_host` using `systemd-run` with
`Delegate=cpu memory pids`, the reviewed AppArmor Bubblewrap profile,
root-owned packaged Rust/Cargo and LLDB-DAP. The fixture verifies real
build output and DWARF, one-use separate DAP consent, independent native
stop/thread/stack, replay denial and private-stage removal. It reopens the
supervisor journal after both jobs to require two terminal non-quarantined
receipts. The existing `--cancel` run is protected Cargo cancellation,
**not yet project-debugger cancellation**.

**Windows:** `ide/supervisor/examples/wo6_native_dap_acceptance.rs`
adds `--project-debug` to the existing independently installed SCM +
AppContainer acceptance executable. It consumes an explicit, observed
fixed MSVC/SDK Rust toolchain bundle, performs real protected `rust_build`,
audits one PE and matching PDB, seals/rehashes the target, and starts a
separately approved `dap_lldb_project` job under the same kernel custody.
Requires real stop/threads/stack, terminal receipts, both old approvals
unreplayable, source hash unchanged, private stage removed, two clean
journal records after restart. New Windows acceptance is wired into
`.github/workflows/ide-wo6-native-os-acceptance.yml`; the pre-existing
fixed-debuggee normal/cancel checks are preserved.

**Critical DEV-04.5A prerequisite repair:** the project debug target
staging and second PE/PDB rehash was incorrectly inside the ordinary
Windows LSP startup. Upstream commit `666f725c` moved it to
`start_windows_dap_inner` where the target and resulting DAP session
are actually available. This must compile and pass native Windows tests.

## Evidence and remaining gates

- Initial source commits: Windows harness `0082c8af`, Windows CI
  `b20820a6`, Linux replay/stage coverage `cf5a2414`.
- At draft PR #174 creation, native OS runs were **queued**, not green.
  Inspect the exact-head Windows/Linux job results and repair *specific*
  compiler, kernel, DLL, DAP or cleanup errors before closing this gate.
- Real independent cancellation of a **project** LLDB session, forced host/
  SCM crash during that same project debug session, and installed UI
  acceptance remain open for DEV-04.5C. Existing fixed LLDB cancellation
  and host-crash probes alone do not substitute.
- Production toolchain pin approvals, redistribution, signing, shipping an
  installed positive debugger and source-line stepping/variable mapping
  remain separate DEV-04.5C/D and legal gates. Release features stay off
  by default. No blanket OS permission or sandbox escape fallback allowed.
