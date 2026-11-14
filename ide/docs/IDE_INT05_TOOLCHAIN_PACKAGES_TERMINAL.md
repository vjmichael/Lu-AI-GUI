# IDE-INT05 — Toolchain, packages and protected terminal

**Source status:** INT05-A/B/C integrated on `ide/int05-toolchains-packages-terminal-20261111`, stacked on the INT04 source branch. Native compiler, exact-head Windows/Linux, installed desktop, physical owner approval, PR/CI and regression gates intentionally deferred until consolidated backlog acceptance. **Not accepted for release.**

## INT05-A — Native toolchains / runtime readiness

- The Packages workspace now presents a fixed Rust/Cargo, pytest, Node tests, CMake and offline uv inventory from the existing native `ide_build_status` supervisor. Those native routines check installed pinned runtime bytes and native admission preconditions; no `PATH` lookup, untrusted manifest execution, new process or network discovery is added.
- UI validation rejects omitted/duplicate/unknown profile IDs and malformed readiness values. Runtime readiness is an observation, never a grant: every actual build/test or package Sync still requires an exact frozen project, native per-job approval, and the original OS custody/receipts. Global quarantined recovery or running jobs are separately identified.
- Versions not independently measured are marked **unverified**; no profile is advertised as installed, updated or automatically runnable merely from a manifest. Native Windows `uv_sync_offline` remains denied without actual AppContainer ACL acceptance.

## INT05-B — Packages and lockfiles

- Existing six-file read-only manager inspection and exact fingerprint preview/revalidation remain the single native package source. The workspace now correlates the manifest candidate with *independent* native runtime readiness: vcpkg/Conan remain preview-only; a matching uv candidate plus pinned runtime still requires fresh review and one-use OS approval and only supports offline locked Sync in an isolated private output area.
- No Add/Remove, general resolver, custom registries, update command, arbitrary package scripts, network install or automatic project promotion was authorized. Native private-output audit/discard and interrupted-worker reconciliation are retained; a passed environment build cannot be inferred from displayed text.

## INT05-C — Supervised terminal transcript

- The Jobs workspace now exposes the actual existing bounded 64-KiB/256-chunk Windows/Linux worker output as a **read-only protected terminal transcript** with stream identity and original job/plan/receipt correlation. A current job may show unverified text before the append-only journal entry is terminal; after restart a journal never reconstructs ephemeral output.
- The transcript is constrained by registered project, native active/last-output job identity, native journal, quarantine and recent in-memory outcome. Malformed streams, duplicates, oversized entries, foreign project IDs and missing journal identity fail closed. Escape/control and bidi override bytes are neutralized; output is rendered as React text, never HTML.
- This **is not an interactive shell**: no command input, child process launch, arbitrary executable, terminal credential handling or additional permissions are added. Users initiate one of the existing reviewed fixed Build/Test profiles in the Jobs workspace. Cancellable/recoverable process-tree control remains entirely native.

## Remaining engineering

1. INT05-D: decide and implement any genuinely interactive terminal through *new* native typed session broker and explicit per-command authority, detached from project-controlled scripts and secrets; default remains no shell. Do not grant `shell:allow-execute`.
2. INT05-E: complete offline language/toolchain availability across installed hosts, including version/provenance evidence, license closure and approved package source trust; do not fake native credentials or installation.
3. Consolidated exact-head compiler/front-end tests and Windows/Linux native package/terminal recovery matrix; Windows AppContainer/SCM denial/ACL tests, no-network and escaped-descendant probes; installed app focus/keyboard screen-reader checks; physical-human approval only at backlog closure.
4. Continue to IDE-INT06 source control/workspace integration independently of those acceptance gates. No CI/PR verification or merge triggered by INT05 source commit.

## INT05-D — Protected interactive typed terminal

- Added an optional **Terminal** navigation pane with explicitly opened native sessions and one-command-at-a-time, bounded textual input. Native `terminal_broker.rs` recognizes only `help`, `status`, `toolchains`, `packages`, exact `run <reviewed fixed profile>` commands and `cancel <exact job ID>` (or preflight). The Rust broker rejects shell syntax, executable names, environment expansions, scripts, flags, pipes, redirects and arbitrary shell arguments.
- A process-local project/session/sequence pair is consumed before each native operation; stale sessions, project switches, duplicate commands and session replay are denied. IDs are correlation only, **never credentials**. Rejected user-entered text is not appended to terminal history; history is React-local, bounded and cleared on project changes.
- Fixed `run` commands delegate **only** to `NativeBuildRuntime::run_profile`, which independently rehashes the selected saved manifest, freezes the source and requests fresh native OS consent before protected execution. Native absence fails closed. No PTY, child spawn, network, shell privilege or terminal execution path was added.
- Existing exact-job protected cancellation/reconciliation and ephemeral native stdout/stderr remain authoritative. The terminal cannot infer a success receipt or installed acceptance from text.
- Source-level Rust parser/replay tests and frontend contract tests added; not executed in this backlog phase.

## INT05-E — Installed toolchain provenance and availability

- Extended native `BuildStatus` with `cargoProvenance` and each pinned multi-language profile with `provenance`. Only **actual rehashed installed tree bytes matching the reviewed compile-time release pin** are published as verified; absent pins, mismatches and Windows uv ACL denial cannot be represented as installed availability.
- Only bundle SHA-256 and pin SHA-256 leave the host. No installed paths, launcher flags, secret environment or project-controlled version strings are returned. Frontend checks lower-case hex fingerprints, exact equality and fixed provenance state independently before claiming readiness.
- **Version, publisher authenticity, complete license closure, and installed Windows/Linux physical acceptance are still explicitly unverified.** A matching digest is not a vendor signature, software provenance attestation or one-use execution grant. Complete distribution review remains a future acceptance gate.
- Native invariant tests and JS regression contracts added; compilation, native Windows/Linux, recovery/custody, physical accessibility and installed acceptance are deferred by owner instruction.

## Next gates

- Complete full exact-head build, tests, native OS kernel isolation and original protected receipt/restart matrix at consolidated backlog closeout. Include terminal command injection, foreign project/session races and per-command OS refusal/cancel, Windows uv ACL denial, cross-project custody, corrupted toolchain pins and license closure.
- Do not merge or publish release packages before those gates. Continue to IDE-INT06 independently.
