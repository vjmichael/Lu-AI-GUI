# DEV-02.6B — Whole original native LSP resources and protected protocol acceptance

**Status at authoring (8 Nov 2026): Source staged on draft PR #141;
exact-head Windows/Linux native CI and independently protected worker evidence OPEN.**
Stacked on DEV-02.6A PR #140, DEV-02.5 PR #138 and DEV-02.4 PR #134.

## Resource-closure correction

The previous original-archive reviewer only extracted the main executable.
That was sufficient for identity research, but not a complete native language
server payload. In particular clangd 23.1.0 may require same-directory DLLs
and Clang resources under lib/clang/ to run semantic analysis on real code.

The new stage_complete_native_lsp_candidate.py:

- Uses the fixed **existing official original-archive SHA-256 pins** for
  clangd 23.1.0, rust-analyzer 2026-11-02 and Ruff 0.17.4 on both x86-64 OSs.
- Checks original compressed-archive SHA before inspecting any release content.
  Rejects absolute, traversal, duplicate, case-aliased, symlink, special or
  nonportable archive members, and unexpected cross-root paths. No extractall.
- Copies **every original regular archive file**, with SHA256/size identity,
  including native library siblings and original Clang resource directories.
  It refuses any archive that cannot fit the protected Windows worker's
  500 files / 512MiB or Linux worker's 1000 files / 1GiB per-bundle quota.
- Preserves all original release resource paths relative to a single verified
  release root and stages each main program into bin/<original filename>,
  matching the real Lu protected worker launch convention.
- Adds four original immutable Git-blob-pinned upstream license texts without
  modifying their bytes; stages only to disposable runner output.
- Verifies every staged candidate file's size and SHA256 independently in
  GitHub Actions; uploads only the JSON manifest, NOT the binaries, original
  archives or temporary local resource directory.
- Handles empty upstream nonexecutable resource files as genuine files while
  rejecting an empty raw .gz executable or changed decompressed length.

The candidate report guarantees entire original archive file-byte retention
or fails. It does **NOT** yet guarantee dynamic loader resolution, ABI
compatibility on a clean machine, copyright/transitive redistribution approval,
protected execution, or installed Lu IDE UI. These independent flags remain
false. It is not safe to flip the production releaseApproved flag.

## Real protected-worker integration

New Rust test fixture:
ide/supervisor/examples/e2_native_multilang_protected_acceptance.rs

For each of the three native reviewed binaries, it uses the real
start_windows_lsp (signed, pinned restricted SCM broker, AppContainer, Job
Object and native profile) or start_linux_lsp (root-owned Bubblewrap, exact
cgroup-v2 kernel membership and process limits) **without an alternate
launcher**. It requires an externally reviewed exact full-tree SHA256 and
refuses self-measurement as approval.

Each protected run uses a newly generated synthetic source file, sealed D3
snapshot, fixed recipe, fsynced supervisor journal and isolated one-use test
authorization. It measures real initialization, nonempty diagnostic
notifications for invalid source and, for clangd/rust-analyzer, hover,
same-file definition and completion from the actual provider. Ruff is
intentionally diagnostics only. It verifies source not mutated, kills and
reaps the process tree, and reopens its durable journal to verify normal and
cancelled terminal state without reconciliation.

The fixture's synthetic test authorization is NOT genuine OS dialog/user
consent, installed GTK/UIAutomation evidence, or production tool approval.
User-facing installed acceptance therefore remains independent.

The manual workflow
.github/workflows/ide-dev02-protected-multilang-real-protocol.yml requires
**real independently enrolled self-hosted protected Windows and Linux
runners**: Windows owner-signed LocalService broker with exact reviewed
SHA and valid Authenticode, correctly installed restricted broker/ACLs;
Linux root-owned Bwrap and reviewed resource roots with delegated cgroup-v2
CPU/memory/PID controllers and the runner in lu-ide-host. Both require
independent whole-tree pinned hashes for all three providers.

The hosted CI
.github/workflows/ide-dev02-full-native-resource-candidate.yml separately
compiles the real protected fixture API on Windows/Linux but never executes
it on uncontained shared runners. A green hosted compilation is not a kernel
containment/protocol pass. The manual OS jobs must really execute and produce
normal and cancellation evidence at the exact head SHA.

## Still-blocking acceptance gates

1. Run exact-head native Windows/Linux whole-release resource extraction
   and file inventory, repair genuine archive-layout/quota defects and
   compare 02.6A real dependency/version receipts.
2. Review dynamic DLL/ELF imports, loader search paths, clangd resource-dir,
   entire original notice/dependency graph and real clean-machine ABI.
3. Independently provision the two protected native runners with original
   immutable bundle trees, reviewed full SHA pins, signed Windows broker,
   root-owned Linux bundle and actual delegated cgroup.
4. Execute **real** normal and cancellation three-provider LSP protocol
   in those OS workers. Require completed status, real semantic response,
   process reaping and durable recovery. No fake/stub successes.
5. Genuine installed Windows NSIS/Linux Debian GUI tests, OS-owned picker/
   consent, diagnostics/hover/completion/definition, restart and uninstall.
6. Deferred publisher identity, signing/legal source and binary distribution
   review remain separate; none is accepted by this candidate engineering.

All production gates remain fail-closed.
