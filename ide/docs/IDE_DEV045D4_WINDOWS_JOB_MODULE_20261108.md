# DEV-04.5D4 — Windows Job/SCM live PE image witness (stage A)

**Status: DRAFT IMPLEMENTATION. NOT OS/installed accepted.** Stacked on
DEV-04.5D3 PR #181; earlier protected debugger gates B/C/D1/D2/D3
remain independently open until exact-head native execution passes.

## Why this step is necessary

LLDB-DAP `stackTrace` may report an instruction-pointer reference
(`0x...`), but the adapter alone does not prove the executed binary, the
OS-loaded address, a true stopped thread context, or the source line.

The existing Windows native worker already owns an exclusive AppContainer
Job Object, restricted SCM recovery service lease, and durable per-turn
one-use journal. This implementation obtains a read-only witness *through
that owned Job handle* instead of accepting a PID or address map supplied
by DAP, a frontend, or a downloaded program.

## Implemented

- `ide/supervisor/src/windows_kernel_pc.rs` uses
  `QueryInformationJobObject(JobObjectBasicProcessIdList)` to enumerate
  actual live process members of the **already admitted** Job. The list
  is bounded to 64, rejects truncated/ambiguous membership, and is
  queried a second time to detect changes.
- `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` and
  `QueryFullProcessImageNameW` confirm the process executable is the
  exact native privately-staged, previously SHA-256-verified Cargo build
  executable. ToolHelp `CreateToolhelp32Snapshot` +
  `Module32FirstW/Module32NextW` independently observe the module base,
  size, image name, and whether the *claimed* PC lies within that loaded
  module. A partial failed snapshot is **not** a pass. No raw process
  handles, PIDs, paths or module data are forwarded to the renderer.
- A separate bounded PE32+ reader checks the executable format,
  preferred image base and SizeOfImage, compares that size to the
  OS module, and derives the preferred unmapped instruction address
  from the actually loaded image base. ASLR is accounted for without
  trusting the adapter's module base or applying an invented slide.
- `WindowsLspProcess` alone can invoke this through its already owned
  Job handle, live SCM lease and protected DAP gateway. Mock/fixed/LSP
  workers use a trait default-deny. `WindowsDapSession` also checks the
  original completed Cargo build terminal receipt and distinct debugger
  Authorized/Running receipt and plan SHA, the target binding, source
  snapshot and native rehashes. Both the Job and the receipts are checked
  while the session remains Running.
- The existing Windows SCM/AppContainer acceptance executable has an
  additional fixed `--project-debug-module` mode. It must use the
  identical protected real Cargo → native LLDB worker path and produce
  `DEV045D4_WINDOWS_REAL_JOB_PE_MODULE=PASS` only after live Job,
  loaded module and receipt checks. The separately signed installed
  consumer release is **not** enabled or covered.

## Explicit nonclaims

**A loaded PE address range is not a stopped-thread IP witness.** Until
a verified Windows x64 thread context is independently obtained through
kernel-controlled handles while LLDB has the thread stopped, no claim
that the actual stopped RIP equals DAP's instruction address is allowed.
Likewise, a PE/PDB GUID-age match is not a CodeView source-line record.
`os_thread_context_ip_verified=false`,
`independent_pdb_line_verified=false`,
`source_line_mapping_verified=false`, and
`installed_app_accepted=false` are mandatory on every D4A result.

Windows thread-context observation and independently pinned LLVM
PE/PDB line lookup against the *same* stopped RIP, original Cargo
snapshot and immutable PDB are the DEV-04.5D4B follow-on.
Only **after** both systems' full protected native OS results pass should
DEV-04.5D5 introduce bounded separately approved
`setBreakpoints`, step/continue and read-only scopes/variables.
`evaluate`, `setExpression`, `runInTerminal`, arbitrary attach/launch,
unsigned default execution, user-controlled PID and reused grants
remain prohibited.

## Proof gates

1. Native `windows-2025` source and Win32 module unit tests compile
   and pass on the exact PR head; fix real rustfmt or SDK errors.
2. The original D7D protected SCM service, native AppContainer Job and
   pinned offline Cargo+PE/PDB build actually reach a real LLDB stop,
   then run `--project-debug-module`. Record module base, preferred
   PC, build receipt hash, Running receipt and completed terminal.
3. Require failure on a foreign Job, wrong original PE SHA, wrong module
   base/size, changed process membership, missing PDB or receipt,
   stale DAP state and changed/replayed authorization. No fallback to a
   standalone test process or loosening the service/AppContainer rights.
4. Subsequent D4B real RIP/PDB test and installed application acceptance
   must pass separately before source-level debugger commands can run.

**Current result:** Source work committed; no completed exact-head
Windows kernel/LLDB CI results. Any CLI output or source-only test is
not equivalent to native live stop or signed installed acceptance.
