# DEV-04.5D — Source-line semantic preflight (stage 1)

**Status: DRAFT SOURCE IMPLEMENTATION.** Stacked on DEV-04.5C PR #176 (head at branch creation: `6fbfe44705019171d024cab1a94309f723281a5b`). This is a read-only preflight, not breakpoints, stepping, variable inspection, signed installed debugger acceptance or debug line-table certification.

## Implemented in first stage

- `ide/host/src/dap_session.rs` retains a bounded, native-only ephemeral `SourceLineCandidate` only after an independently correlated, successful, nonempty `stackTrace` response in the Stopped phase. No source candidate is fabricated for frames without a real local source path, virtual `sourceReference`, or missing coordinates. A new stack query, continued event, disconnect, termination or protocol quarantine invalidates the old candidate.
- `ide/host/src/dap_source_lines.rs` rejects unbounded, relative, traversal, control-character or remotely referenced source paths. It can compare a candidate against exactly one **native host-selected** relative source file under the reviewed project root, the independently supplied pinned original file SHA-256 and an actually present source line. Rejects missing/linked/oversized/non-UTF8/mutated source. This does not read another file guessed from adapter output and does not execute the project.
- The review explicitly sets `debug_symbol_line_table_verified=false`, `debugger_execution_authorized=false`, and `installed_app_accepted=false`. A DAP-reported source location plus matching source bytes alone does **not** prove the executable's DWARF/PDB line table maps that line to the stopped instruction. There is no source map service or WebView command.
- Host unit tests cover correlated vs missing/virtual/malformed frames, clearing an old candidate, exact source digest and valid line, traversal/forged source and file mutation, plus Unix symlink denial.

## CI technical gates

`.github/workflows/ide-dev04-5d-source-line.yml` independently checks the exact source commit and compiles/runs the real Rust host tests on Windows 2022 and Ubuntu 24.04 under Rust 1.88.0. Rustfmt and explicit default-denial contracts are required. At creation there are **no current-head results yet**; a queued job or passing source test is not native LLDB acceptance.

## Follow-up gates (do not quietly skip)

1. Obtain and repair **actual Windows/Linux Rust compiler/test/rustfmt** results at the current D head. Do not claim a pass based on source review.
2. Provide independently verified compiler DWARF line-table / PDB line-record evidence linking the exact protected build executable + symbols, stopping instruction address, exact source snapshot and source line. The read-only source candidate cannot substitute.
3. Only after B/C real kernel and signed-installed gates pass, add separately scoped `setBreakpoints`, step/continue, scopes/variables with strict no-expression behavior and explicit bounded state transitions; never trust arbitrary DAP adapter output or frame IDs for executable permissions.
4. Native OS Windows SCM/AppContainer and Linux delegated cgroup-v2 process custody, cancellation, crash recovery, replay denial, and installed UIA per-feature acceptance still required.
5. Preserve existing refusal of `evaluate`, `setExpression`, `runInTerminal`, arbitrary launch/attach, foreign paths, unapproved modules, auto-continue and unconfined debugger execution.

This branch does not change existing D3 per-run human authority, the known DEV-04.5B formatting failures or DEV-04.5C GTK chooser failure.
