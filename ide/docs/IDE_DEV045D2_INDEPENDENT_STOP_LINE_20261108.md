# DEV-04.5D2 — Independent stopped-PC-to-DWARF/PDB line semantic witness

**Status: DRAFT. Not independently accepted against a genuine protected stopped debuggee yet.**
Stacked on DEV-04.5D1 PR #179 and upstream DEV-04.5C PR #176. All new CI must run on the **exact head** of this draft. The stable IDE and default unsigned installer remain nonexecuting.

## Implemented

1. Native DAP `stackTrace` capture includes a bounded optional `instructionPointerReference` parsed as a nonzero 64-bit hex address. This is only an **adapter-claimed** stopped PC, never an OS/kernel identity or debugger permission. The native protected Linux/Windows DAP session offers read-only ephemeral access, and a fresh stack query or continuation invalidates the previous source candidate.
2. `ide/packaging/verify_dev045d2_line_oracle.py` is a **CI-only** independent LLVM tool, not a Tauri command or an application launcher. It separately hashes the reviewed source and executable (plus PE/PDB), verifies caller-supplied exact debug target digest and source-coordinate constraints, and asks a measured independent `llvm-symbolizer` for the line at the specified instruction address after an explicit image relocation slide. The exact source file and line MUST match the independent reader, not merely the DAP report.
3. Real compiler acceptance fixture `test_verify_dev045d2_line_oracle.py`: on Ubuntu compile an actual non-PIE ELF with Clang `-g`, get a real symbol address via `nm`, and compare genuine LLVM DWARF source-line decoding. On Windows build actual `clang-cl /Zi` + `lld-link /DEBUG` PE/PDB with no C runtime, extract the real PE image-base+entry RVA, and compare LLVM CodeView/PDB line decoding. Tamper tests reject wrong DAP line, source path, PC, source or binary modification, oracle binary mismatch, missing PDB, traversal and linked files.
4. New exact-head Windows 2022 / Ubuntu 24.04 test workflow `ide-dev04-5d2-source-symbol-oracle.yml` plus genuine Rust source compilation matrix. No arbitrary LLDB attach/launch, source expression, breakpoint, stepping, variables, debuggee executable invocation or general shell execution API was added to Lu IDE.

## Distinguish five evidence categories

- **Static binary+source SHA:** independently rehashed bytes.
- **Compiler line semantics:** independent LLVM symbolizer maps an address to the same file+line.
- **Stopped instruction identity:** requires the real **native protected LLDB** stop and its stack frame instruction address, with exact plan/receipt/job, not a fabricated or static fixture PC.
- **ASLR / loaded image origin:** requires the real kernel-observed module-load bias from that same protected process, not an arbitrary `runtimeSlide` argument.
- **Installed acceptance:** requires signed, offline, policy-gated real Windows/Linux installed app/UI, cancellation, restart, process-tree cleanup and uninstall evidence.

**The current independent tool can validate only the first two**. Its output expressly sets `stoppedPcKernelAttested=false`, `runtimeSlideKernelAttested=false`, `breakpointsSteppingVariablesEnabled=false` and `installedAppAccepted=false`. Its user-supplied `targetBindingSha256` is a provenance label, not a verified one-shot grant. The llvm tool path/hash are CI operator-observed test-only bytes; the production signing/redistribution pin remains separate.

## Acceptance gates before breakpoints or stepping

1. Native Windows/Linux CI compiles and passes the real compiler fixtures and hostile tests at this exact branch head. Fix actual failures; do not conflate queued/cancelled with pass.
2. Capture a real protected **stopped PC and frame** from LLDB-DAP, independently identify exact OS process+module via Linux cgroup-v2 + kernel `/proc` memory mapping or Windows SCM/Job + kernel module base, rehash its original executable/PDB and read-only symbolizer, and prove the loaded address+line **matches original pinned source**. Do not treat DAP's own module name/base as independent OS evidence.
3. Independently verify the source instruction/address record against the actual `rust_build` terminal journal, immutable target digest, one-use debugger grant, kernel cleanup and post-restart replay denial.
4. Only after explicit B/C protected OS and signed-installed acceptance, create separately host-authorized `setBreakpoints`, bounded step/resume and safe read-only variables/scopes. Keep `evaluate`, `setExpression`, `runInTerminal` and arbitrary attach/launch denied.
5. Do not broaden the worker's OS filesystem/network/process rights to make a diagnostic test green. Produce an OS-authenticated cross-platform witness or leave the feature disabled.
