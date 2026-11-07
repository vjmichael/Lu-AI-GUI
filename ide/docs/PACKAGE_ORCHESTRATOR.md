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
