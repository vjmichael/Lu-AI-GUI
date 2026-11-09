# DEV-04.5C — Installed project debugger, bundle integrity, cancellation and recovery

**Status: implementation in progress; technical acceptance NOT established.**
DEV-04.5C stacks on DEV-04.5B PR #174, which stacks on DEV-04.5A
PR #171 and DEV-04.4 PR #167. Do not merge or claim native installed
debugging based on source checks or CI jobs that have not run.

## Latest independent CI inspection

At latest review, PR #174 exact head `523cd04aae6dd896b2e7d916bc7d779d8db4bf5f`
had 37 workflows registered. Its WO6 native Linux and Windows Cargo →
project LLDB jobs, DEV-04.4 exact-head compilation matrix and temporary
source formatter were queued/pending. The completed Linux 26.04 installed
editor diagnostic run `42564174798` **failed** the genuine GTK folder
chooser: 240/240 frontend tests and Debian failed-upgrade recovery passed,
but the chooser stayed open after four bounded native actions. No completed
project LLDB OS acceptance evidence existed; this GTK issue is independent.

## C1. Reviewed installed LLDB candidate bytes

`ide/packaging/verify_dev045c_debug_bundle.py` validates an independently
prepared **candidate artifact** lock, without downloading or executing any
debugger. The manual signed-candidate workflow requires the lock at
`candidate/dev045c-project-debug-bundle.lock.json`. Contract:

- `schema`: `lu.ide.dev045c.installed-project-debug-bundle.v1`
- `scope`: `technical-installed-test-only`
- `sourceSha`: exact lowercase 40-character candidate commit
- `platform`: `windows-x86_64` or `linux-x86_64`
- `releaseApproved`, `redistributionApproved`,
  `userSelectedExecutable`: **false**
- `files`: bounded array of exact `{path, sha256, bytes}`; the fixed
  adapter and fixed test probe plus only reviewed in-bundle native DLL/SO
  dependencies. Every byte is rehashed from the installed
  `resources/debug-adapters/lldb-dap` tree. No extra, linked, hardlinked,
  case-colliding, traversed, ambiguous, oversized or missing files.
- `notice`: exact `{path, sha256, bytes}` for an *actual original*
  debugger/dependency license file under `THIRD_PARTY_LICENSES/`.

Candidate metadata and a matching file SHA are **not** a vendor signature,
review of LLVM redistribution rights or real kernel containment. The
verifier explicitly emits `nativeHostTreePinVerified:false`,
`osContainmentVerified:false`, `installedProjectDebuggerAccepted:false`
and `releaseApproved:false`. Actual product installation still requires
the independent native Rust bundle-tree pin compiled into the application,
the protected service identity and existing per-run native consent.

Eight hostile tests exercise exact bytes, forged approvals, stale source,
DLL and debuggee mutation, unexpected files, case/path collisions, links,
original notice tampering and Linux binary names. The separate
`ide-dev04-5c-installed-project-debug.yml` matrix checks the verifier
and the signed native UIA scenario source; its green result would establish
**source/test readiness only**.

## C2. Positive installed Windows project debugger GUI

The existing `ProtectedBuildDebug` installed scenario was explicitly a
**fixed debuggee self-test** and never proved Cargo → project LLDB.
DEV-04.5C adds separately selectable, real Windows UI Automation modes:

- `Dev045CInstalledProjectDebugger`: sign-checked installed desktop,
  independently validated installed SCM service, real OS folder picker,
  a first-party Cargo fixture, native `rust_build` selection, fresh native
  one-use Cargo approval, completed zero-exit worker receipt and output seal,
  **different** native `Debug this exact project build` approval, a real
  project-only LLDB session (not a fixed self-test), build receipt/binding,
  stopped-thread/threads/stack, worker-process baseline, and unchanged
  original source files.
- `Dev045CInstalledProjectRestartDenied`: independently **relaunches the
  installed app**, verifies the durable debug journal, requires the old
  project-debug button disabled (no same-process sealed target, no grant
  replay) and verifies no native debugger/worker was auto-started.

`.github/workflows/ide-wo8-signed-protected-e2e.yml` now requires
the candidate lock and both positive scenarios, followed by real NSIS
uninstall, SCM service disappearance, installed executable removal and
native process baseline. The workflow is **manual**, has the existing
`lu-ide-native-e2e` environment review, and requires an actual independently
signed original WO7 candidate artifact. It must fail closed when candidate,
license, service or runtime pins are absent. The default unsigned installer
continues to deny execution.

## C3. Genuine project-debug cancellation

Separate native project DAP cancellation modes are now added to both OSes:

- Linux `--project-debug-cancel`: protected Cargo produces the actual
  executable, independently verifies ELF/DWARF, and starts a **different**
  approved LLDB in delegated cgroup-v2/Bubblewrap custody. Only after
  genuine stopped/thread/stack evidence, dropping the DAP session must
  journal `CancelRequested` then `Cancelled` with kernel descendants
  reaped. Reopened durable journal must retain completed build and cancelled
  debugger as separate entries; their old grants cannot be replayed and
  private target staging is removed.
- Windows `--project-debug-cancel`: genuine AppContainer Cargo, PE/PDB,
  separate SCM-protected project LLDB, real stop/thread/stack, native Job
  cancellation, ordered durable receipts, and replay/target-cleanup checks.
  This is a **CI fixed test**, not an installed UI cancellation claim.

The existing `ide-wo6-native-os-acceptance.yml` includes those fixed
modes in its real native OS jobs. All native release paths remain denied
by default without reviewed installer/runtime pins. **Actual Windows/Linux
kernel run results are required before accepting these cases.**

## C4. Still outstanding, not implied by any source pass

1. Obtain PR #174's exact-head native Windows/Linux compilation and Cargo
   → LLDB normal results; fix actual errors; inspect formatting output and
   delete its temporary formatter before declaring DEV-04.5B accepted.
2. Obtain DEV-04.5C native kernel cancellation results and independent OS
   process-tree witnesses. Add genuinely installed project cancellation
   (distinct from fixed self-test), including a real user cancellation while
   the protected debuggee is still running.
3. Provision an independently reviewed, hash-locked offline LLVM debugger
   bundle, exact compiler/resource pins, service image signature and complete
   original upstream notices. Without the signed candidate artifact and
   approved environment, the manual installed flow is deliberately blocked.
4. Run manual signed installed project normal/restart/uninstall UIA. Inspect
   logs, exact head SHA, installer and service identity, source/receipt
   hashes, native worker cleanup and candidate lock.
5. Inject a **project debug session-specific** host crash and service crash
   on each relevant OS; prove kernel custody after restart and journal
   quarantine/reconciliation. Prior fixed LLDB crash tests are not enough.
6. Confirm legal and redistribution approval only when the final dependency
   set is decided. Technical test candidate is NOT release signing/legal
   approval; no auto-enable, uncontained LLDB or arbitrary paths.

DEV-04.5D source-address mapping, stepping, breakpoints and variable
inspection remain separate and explicitly out of scope here.
