# Lu IDE DEV-03 — project-specific Build / Run / Test

**Scope:** Group D / DEV-03 of the November 7 Product Readiness and
Blocker-First Execution Ledger. This is distinct from Wave D's protected
supervisor. **Current implementation milestone:** DEV-03.1, source only.
**Branch:** ide/dev03-project-run-profile-discovery-20261108, stacked on
C7 SHA 07f0a889bd14c3d587e26678307a5c8d32f9fe6e.

## Why start with read-only project profiles?

The existing WO6 protected runtime accepts exactly three Cargo recipe
IDs: rust_check, rust_tests, lu_ide_host_tests. Every actual execution
must still freeze a source snapshot, validate source and toolchain pins,
obtain a fresh native OS approval, consume a one-use protected-supervisor
grant, and prove descendant cleanup and terminal journal evidence.

A manifest filename or a user-chosen script is not an approved native
adapter. In particular, blindly passing npm "test", arbitrary
pyproject instructions or CMake executable commands across the WebView
would bypass the deliberately separate reviewed-execution boundary.

### DEV-03.1 — implemented source

1. The native registry exposes a fixed, project-scoped
   ide_discover_project_run_profiles command. Only projectId crosses
   from the WebView; there is no executable, command, path, shell,
   argument, interpreter, manifest text or process permission parameter.
2. The host checks six fixed project profiles: Rust Cargo check/test,
   Python pyproject pytest, JS package.json test convention, CMake C/C++
   build, and the Lu IDE host's fixed Cargo tests. Each filename is a
   compiled-in constant and read through the existing registered cap-std
   source file capability, returning only a saved SHA or unavailable
   state, not source text. No manifest is evaluated or declared valid.
3. For test profiles, the host makes one bounded metadata-only listing
   of the conventional tests/ directory. It reports candidate filenames,
   at most 20 hints per language and at most 400 scanned entries.
   File counts and hints are explicitly partial; no project source code
   or package script is executed, and zero conventional candidates is
   **not** a "no tests" determination.
4. The Jobs UI exposes an explicit Inspect project profiles control,
   a bounded readable comparison, saved SHA and candidate test hints.
   Only the existing fixed Cargo recipe IDs can be selected into the
   old WO6 selector. The existing Run/Test button independently checks
   native resources and requires fresh OS approval every time.
   Python, JS and CMake profiles are visible but disabled for execution.
5. A closed-schema native capability and frontend response guard reject
   spoofed profile IDs, fabricated manifest paths/SHAs, arbitrary
   command strings, foreign projects, worker grants and invented results.
   Project switches invalidate older asynchronous discovery responses.
6. No modifications to the original WO6 build backend, D3 supervisor,
   OS containment, draft vault, project source files, or release pins.

New files:
- ide/host/src/project_run_profiles.rs
- ide/host/tests/project_run_profiles.rs (six native filesystem tests)
- ide/app/web/project_run_profiles.js
- ide/app/tests/project_run_profiles.test.mjs (nine frontend checks)

Updated native command registry, 48 fixed capability entries, 45 typed
schema definitions, Jobs screen, CSS, this documentation and project
handoff. Source checks: all nine frontend test bodies passed in a
development V8/Node-assert emulation against fetched exact GitHub source.
**Not** a Node/npm, Rust compiler, GitHub OS or installed GUI result.

## Remaining DEV-03 implementation

**DEV-03.2 — reviewed runnable project profiles.** Add separately
reviewed, exact native adapters for Python pytest, CMake/CTest,
JavaScript/TypeScript test runners and Rust target selection. The
native host, not project scripts/WebView, must resolve executable
identity, source snapshot, toolchain pins, fixed argument templates,
resource bounds and containment. A selected project may request an
explicit review but is not itself an execution grant. Do not enable
a generic npm script, Python import, CMake generator or shell eval.

**DEV-03.3 — genuine progress and streaming logs.** Connect
supervisor-owned bounded framed stdout/stderr updates and typed
status events to the current Jobs UI. Preserve prior output on errors.
No unbounded WebView log or misleading token-like "progress".
Late/stale messages must never cross project/job boundaries.

**DEV-03.4 — useful test discovery.** Run reviewed, bounded discovery
operations *inside* the isolated native worker for Rust, Python,
C/C++ and JS/TS; normalize stable test IDs and file/source locations;
distinguish discovery failures from zero found tests, track
selection/filtering as independently reviewed request.

**DEV-03.5 — durable receipts, cancellation and installed acceptance.**
Exact user project, source snapshot, toolchain, run plan, native result,
test summary, log/artifact SHA and independent OS kernel cleanup must
be preserved in the original supervisor journal. Real Windows/Linux
install, two launches, project edit/build/test, cancel, worker crash,
restart and uninstall. Avoid claiming production release acceptance
until fully pinned bundle and legal/shipping review finish.

## Acceptance boundary and parallel work

Dedicated exact-head GitHub Windows 2022 and Ubuntu 24.04 workflow:
.github/workflows/ide-dev03-project-profiles-native.yml. It executes
genuine Node regression, native Rust fixture and host suite, formatting,
locked Tauri compile, and a diff guard ensuring **no change to the
protected native worker** in this slice.

No installed runtime is newly enabled and no provider is newly
shipped. The existing Group A/B OS failures (real GTK chooser and
protected DAP) and Group C integration/installed artifact gates remain
open and will be followed up independently from source development.
The original product-readiness ledger on
ide/product-readiness-blocker-ledger-20261108 remains the authoritative
high-level list until branch consolidation.
