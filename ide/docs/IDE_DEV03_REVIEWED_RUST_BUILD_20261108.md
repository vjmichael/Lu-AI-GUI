# Lu IDE DEV-03.2A — Reviewed Rust Cargo Build

Date: November 8, 2026.
Branch: ide/dev03-reviewed-rust-build-20261108.
Parent DEV-03.1: 80486012e817de2f6c64892695e66ebb99292286.

## Implemented

The original standalone WO6 protected Cargo worker now recognizes
one additional fixed recipe: **rust_build**.

Existing recipes are rust_check, rust_tests, lu_ide_host_tests.
The new recipe invokes precisely the native equivalent of
cargo build --locked --offline --manifest-path Cargo.toml.

The following boundaries remain authoritative and unchanged:

- The WebView may submit a fixed recipe ID and active project ID only;
  never an executable path, script, source content, argument list, shell,
  grant, environment or arbitrary working directory.
- The native host obtains the registered source root and verifies clean
  Git HEAD. The original immutable source snapshot copies and hashes
  the saved manifest and exact tree; untracked generated target files
  are excluded under the existing snapshot policy.
- The existing release-pinned native toolchain must be installed.
  Each run receives an independent native human OS confirmation, then
  the existing one-use protected supervisor authorization.
- Linux runs fixed Cargo build inside the original cgroup-v2/Bubblewrap
  no-network worker with pre-exec cgroup membership and bounded
  process/output/resources. Windows uses original AppContainer+Job,
  authenticated restricted SCM broker, pinned Cargo and staged source.
- Native cancellation and OS process-tree cleanup, one-job exclusivity,
  supervisor receipt journal and recovery rules remain unchanged.
- No new general terminal, CI privilege, startup execution, npm script
  or package installation is introduced by this recipe.

Updated source:
- supervisor/src/lib.rs: exact recipe plan allowlist + hostile plan test
- supervisor/src/snapshot.rs: rust_build exact manifest + frozen-tree test
- supervisor/src/linux_worker.rs: fixed Cargo build command
- supervisor/src/windows_worker.rs: fixed Cargo build command
- app/src-tauri/src/build_backend.rs: fourth fixed native recipe
- host/src/execution_intents.rs: bounded, nonexecuting build intent
- host/src/project_run_profiles.rs: seventh fixed discoverable profile
- app/web/project_run_profiles.js, app/web/main.jsx and matching tests:
  user-visible Build selection without grant
- host/src/capability_schemas.json: closed enum, seven-profile maximum

Python pytest, JS/TS tests, and C++/CMake are still view-only in Jobs.
There is no native execution adapter for those languages yet.

## Test status

The seven additional static cross-layer regression cases plus the
nine profile tests all passed in a V8 test harness with Node-assert
emulation against exact fetched GitHub source. This is a development
sanity check, not native execution acceptance.

The dedicated exact-head Windows 2022/Ubuntu 24.04 workflow is:
.github/workflows/ide-dev03-reviewed-rust-build-native.yml.
It runs genuine Node, Rust host and protected supervisor tests plus
opt-in native-build Tauri compilation, with no hidden allowance for
broker/containment source changes.

Open gates:
1. Inspect real exact-head native jobs, repair actual failures and
   rerun same final SHA.
2. Native protected Cargo Build normal/cancel/worker-crash tests on
   each OS, with actual executable/artifact hashes, bounded stdout,
   source immutability and kernel descendant termination proof.
3. Actual installed NSIS/GTK Build/Cancel/Restart/Uninstall acceptance;
   run results must not be inferred from source tests.
4. DEV-03.2B: separately reviewed, source- and toolchain-pinned native
   Python pytest, CMake/CTest and JavaScript/TypeScript worker adapters,
   without running arbitrary project manifest scripts.
5. DEV-03.3 streaming logs, DEV-03.4 real test discovery and
   DEV-03.5 durable installed pipeline acceptance remain open.

Group A/B native GTK and protected debugging failures, and Group C
candidate/packaging integration remain separately open. No signing,
redistribution or full daily-driver release approval is inferred.
