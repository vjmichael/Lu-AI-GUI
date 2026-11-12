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
