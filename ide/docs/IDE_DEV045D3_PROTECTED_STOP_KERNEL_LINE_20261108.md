# DEV-04.5D3 — Genuine protected Linux stopped-PC / kernel image / journal lineage

**Status: SOURCE IMPLEMENTED — LIVE NATIVE OS ACCEPTANCE NOT YET VERIFIED.**
This task stacks on DEV-04.5D2 PR #180 (and #179, #176, #174).
No right to run arbitrary binaries, attach debugger, request a terminal,
step, evaluate expressions, modify breakpoints or inspect variables is added.

## Actual defect repaired

The Linux protected debugger previously built a launch path below
`/debug-target/...` **without binding the sealed project target into its
Bubblewrap debugger mount namespace**. A code block referencing
`project_target` had mistakenly been inserted into `start_linux_lsp`,
where no such variable exists. DEV-04.5D3 moves this exact native-only,
read-only bind into `start_linux_dap_inner`, after immutable target
rehashing and before kernel child admission. No uncontained fallback.

## New native-only stop witness

`ide/supervisor/src/native_kernel_pc.rs` independently checks:

1. A real, job-ID-specific delegated cgroup-v2 directory created by the
   protected worker, and bounded actual `cgroup.procs` membership (no
   caller-supplied PID).
2. One unambiguously matching child whose `/proc/PID/exe` device, inode,
   length and independent SHA-256 match the exact privately staged Cargo
   executable; no symlinks or foreign executable substitutions.
3. The same child is in a kernel **ptrace tracing stop**, not merely
   SIGSTOP or an adapter `stopped` JSON response.
4. The adapter's previously correlated, bounded stopped instruction address
   falls inside this child's actual executable `r-x` mapping in
   `/proc/PID/maps`, independently matched on device/inode.
5. The actual load bias is derived from the executable's ELF64 program
   headers and the kernel RX mapping's file offset; ET_EXEC must have
   zero relocation, ET_DYN must match its observed relocation. A fabricated
   DAP module base, relocation argument or arbitrary address is not used.
6. PID lifetime start ticks, traced stop and cgroup membership are
   revalidated after the inspection. Ambiguous/missing PID, contradictory
   map, changed executable, escaped descendant or denied proc access fail
   closed.

The `LinuxDapSession` retains its own native cgroup location, plan digest,
job ID and sealed input snapshot. Its new native method cross-checks the
real `Running` debugger journal and earlier `Authorized` one-use grant,
the original `Completed` Cargo build's **fsynced terminal receipt hash**,
the target binding, source head/manifest/tree hashes and a strictly
`/workspace/...` DAP source coordinate resolved against that exact
immutable native snapshot. All data stay native-only.

A separately measured root-owned LLVM 18 symbolizer, pinned into the
**CI's native executable at build time**, must then decode that
OS-relocated stopped address to the **same original source file and line**
as the correlated LLDB frame. The new test-only method requires source
and binary digests, ELF/ASLR, path and actual compiler line equality. It
does not enable any DAP command. The distributed release path has no
source-line symbolizer authority without a separately reviewed,
installer-pinned runtime; debug builds only.

The existing real delegated Linux `rust_build` → `dap_lldb_project`
fixture is changed to demand this entire witness *before* closing/cancelling
its native debugger, then require a distinct fsynced Completed/Cancelled
terminal receipt preserving the exact Running receipt and original Cargo
Completed receipt. Reopening the journal must still deny either old grant.

## Native CI requirements and limitations

The existing `ide-wo6-native-os-acceptance.yml` Ubuntu cgroup-v2
project-debugger path now installs LLVM 18 and compiles the exact digest
of its independent symbolizer into the debug-only test fixture. Its real
native `--project-debug` and `--project-debug-cancel` cases demand
`DEV045D3_LINUX_CGROUP_REAL_STOP_ELF_DWARF=PASS` and
`DEV045D3_LINUX_REAL_STOP_LINE_TERMINAL_JOURNALS=PASS`.
A separate source/unit compilation matrix checks Linux ELF slide logic
and keeps Windows Job/SCM behavior unchanged.

**Not established merely by commits:** Windows native Job/SCM loaded
module base + PE/PDB live-stop witness, signed installed-app acceptance,
live protected cgroup result on this exact candidate SHA, and any
step/breakpoint/variables authority. Existing B/C real native gates,
GTK chooser, signing/legal requirements and correct Rust formatting remain
open until actual logs and artifacts pass. The original static fixture
ptrace/SIGTRAP trials are independent partial evidence, not proof of
the Lu protected-session path.

If LLDB stops outside the reviewed module, lacks a real source location,
the kernel denies /proc inspection, or the PDB/DWARF oracle disagrees,
the correct outcome is **DENIED / NOT ACCEPTED**, never automatic
continue, arbitrary attach, release-right expansion, or a synthetic pass.
