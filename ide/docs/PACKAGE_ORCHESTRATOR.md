# IDE Package/Toolchain Orchestrator — design, 2026-11-04

## Goal
One graphical **master manager** for language runtimes, compilers, build systems, package managers and dependencies. It is a coordinator, not a replacement for ecosystem solvers or a system-wide package database. It must not install anything into system Python or OS global paths by default.

## Initial adapters
- Python: `uv` first; inspect `pyproject.toml` and `uv.lock`, create per-project `.venv`, preview `uv add/remove/sync/lock`, respect Python version markers.
- C/C++: choose `vcpkg` manifest mode (`vcpkg.json`) or `Conan 2` (`conanfile.py/txt`, profiles, lockfiles) per project; detect CMake/Meson/Ninja and system compiler (Clang/GCC/MSVC). C compilers and OS headers are toolchains, not ordinary project libraries.
- Rust: Cargo (`Cargo.toml`, `Cargo.lock`) and rustup toolchain selection.
- JS/TS: detect packageManager field/lockfiles; use selected npm/pnpm/Yarn, never silently rewrite a competing lockfile.
- Go: `go.mod`/`go.sum`, Go toolchain.
- Java/JVM: Maven or Gradle with wrapper verification; JDK toolchains.
- .NET: SDK selection via `global.json`, NuGet project lock/central management.
Add other ecosystems through constrained adapters, not free-form agent execution.

## Manager adapter contract
`detect(project_snapshot)-> confidence, manifests, lockfiles, manager_version`
`resolve(request)-> plan` (versions, transitive deps, sources, downloads, scripts/hooks, network, privileges, target locations, expected diffs)
`apply(approved_plan)-> job_id`
`verify(job_id)-> actual versions, hashes, provenance, changed manifests/locks, installed tree`
`rollback/reconcile(job_id)-> evidence and safe recovery where possible`.
Prefer typed arguments and pinned executable identity, not shell interpolation. A plan must expire/revalidate when source manifests or toolchain change.

## UX
Single Packages & Toolchains view; per-project environments; version/registry/source badges; available upgrades; license/CVE advisory review; reproducibility details; disk/network budget; approve/deny per action; manager diagnostics; "open native manager details" advanced view. Make manifest and lockfile changes explicit and diffable.

## Security
Untrusted package install is **code execution**, not merely data download. Install scripts, native builds, Git URLs, Python build backends, compiler plugins and environment discovery may execute code. No automatic install-on-open.
Two phases: (1) static metadata and registry fetch with isolated networking, (2) install/build in restricted worker with workspace-scoped write access, limited network and no secrets. Where a manager cannot resolve without executing code, mark preview as incomplete and require stronger isolation/approval.
Use project-local caches where practical; content hashes, pinned registries, signed provenance if available; commit lockfiles; SBOM/advisory integration; controlled upgrades; avoid running system package managers with elevation under Lu.

## Acceptance
Fixture matrix for Python uv, C/C++ vcpkg/Conan, Rust Cargo and JS/npm; detect without changing files; preview plan; deny install hooks; offline mode; corrupt lockfile; malicious source; blocked home/SSH key access; cancellation; interrupted install; cross-OS repeatability and file-system effect receipts.

## Research references
- https://docs.astral.sh/uv/guides/projects/
- https://learn.microsoft.com/en-us/vcpkg/consume/manifest-mode
- https://docs.conan.io/2/reference/tools/cmake/cmaketoolchain.html
- https://docs.npmjs.com/cli/commands/npm-ci/


## IDE-05.1 source implementation checkpoint (2026-11-09)

The first safe delivery slice is **read-only**, using the existing
`IdeWorkspaceRegistry`, registered project IDs and capability-scoped
`probe_project_resource`. It detects fixed Python uv / C++ vcpkg and Conan
manifests and lock evidence; reports SHA-256 revisions, missing/incomplete
or conflicting managers, and disallows all execution or network operations.
No native package-manager runtime was bundled and no `apply` action exists.
Refer to [IDE-05.1 implementation](IDE_DEV051_PACKAGE_MANAGER_DISCOVERY_20261109.md).

**Gate sequence:** verified static plan preview -> independently authorized
and protected manager worker -> independent result/restart reconciliation ->
consolidated native Windows/Linux and installed approval acceptance. This
source-only slice must not be credited as a functioning installer.


## IDE-05.2 operation preview checkpoint (2026-11-09)

A read-only, revision-bound static operation preview is implemented after
IDE-05.1 discovery. One typed detected manager (`uv`, `vcpkg`, `conan2`)
and one fixed Add/Remove/Sync intent are bound to **all** native manifest
and lock SHA-256 observations. A deterministic preview digest is tied to
manager, operation, requested exact literal package/version, ordered
source evidence and risk classifications. The host independently reopens
all evidence and recomputes the preview before showing a verification
receipt. Any changed input, operation or manager forces fresh review.
A `vcpkg.json` data-only parser also rejects malformed files and obvious
duplicate Add / missing Remove intents.

**This is an intended declaration projection, not actual resolution or a
manifest/lockfile diff.** No package manager or network is called.
`readyForApproval=false` and `authorizationIssued=false` remain
enforced even when native evidence matches. See
[IDE-05.2](IDE_DEV052_PACKAGE_OPERATION_PREVIEW_20261109.md). The later
protected worker must repeat checks under an OS-owned one-use grant,
independently verify output and reject untrusted scripts/hooks.


## IDE-05.3 protected offline uv Sync pilot (November 9, 2026)

A new, **opt-in** `native-package-uv` feature connects the revision-bound
uv Sync review to **the existing** protected WO6 runtime. No generic
package execution API is exposed: only a fresh native OS dialog and
one-use supervisor grant after exact source/preview/runtime checks can
admit the fixed `uv_sync_offline` JobPlan. Linux uses an unnetworked
Bubblewrap/cgroup worker with `/workspace` read-only and a private
`/output` virtual environment; Windows explicitly denies the recipe
until immutable staged-source ACLs are independently proven.
Add/Remove, Conan, vcpkg and networked package management still require
new reviewed adapters and OS evidence. A private worker output is not
an automatically applied package environment. See
[IDE-05.3](IDE_DEV053_PROTECTED_PACKAGE_ADMISSION_20261109.md).

Next: IDE-05.4 independent installed-output checking, license and
source provenance, effect diff, fail-closed rollback/recovery, then
one consolidated Windows/Linux and installed validation group.


## IDE-05.4 local artifact evidence and durable private rollback (2026-11-09)

The fixed uv Sync worker now has a second independent, **read-only**
output verifier: full bounded tree hashes, installed PEP 376 RECORD
sha256/size checks, exact `uv.lock` name/version match, actual hashed
PEP 639 license files and verified original source/lock effects.
Its pre-admission frozen lock check denies Git/path/editable/direct
sources and missing registry wheel hashes. A registry URL or RECORD
claim does NOT prove a wheel's upstream origin; all project-promote
and actual wheel-archive provenance fields remain **false**.

A separate hash-chained, fsynced `package_prepared ->
audit_local_complete|audit_blocked -> discard_prepared ->
discard_verified` lifecycle records unresolved work BEFORE OS
launch, survives restart and supports fresh human-approved private-only
output disposal after independent D3 terminal/cgroup/Job evidence.
Incomplete cleanup remains pending, never automatically resumed.
Windows package execution is still denied; no user project environment
is installed. This is source-complete pending native acceptance.
See [IDE-05.4](IDE_DEV054_PACKAGE_OUTPUT_PROVENANCE_RECOVERY_20261109.md).


## IDE-05.5 consolidated native group gate (2026-11-09)

The acceptance matrix is authored at
[IDE-05.5](IDE_DEV055_CONSOLIDATED_NATIVE_ACCEPTANCE_20261109.md).
It has genuine Windows/Linux Cargo/Tauri/Node compile/test jobs and a
real Linux delegated-cgroup, unnetworked Bubblewrap, root-owned
candidate uv binary **negative/cancel** job. The runner must actually
execute before any native result is credited. This is NOT a real
successful wheel installation, wheel-origin proof or human consent.
Windows package execution remains fail-closed until staged ACL evidence;
physical installed-app approval is a separate human gate. Legacy
per-commit supervisor micro-workflow push triggers are disabled;
PR/manual triggers remain. Queue remains blocked.


## IDE-05.6 Task 5 — independent upstream runtime wheel intake (source stage)

See [IDE-05.6](IDE_DEV056_UPSTREAM_APPROVAL_ACCEPTANCE_20261109.md).
The fixed Linux CI uv runtime now needs independently observed upstream
release-metadata archive hash, wheel-extracted executable and license bytes
before protected execution. No CI package approval, Windows ACL bypass,
upstream publisher signature or physical installed-app acceptance is implied.
