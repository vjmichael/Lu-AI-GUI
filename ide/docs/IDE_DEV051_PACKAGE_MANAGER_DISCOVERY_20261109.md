# IDE-05.1 — Read-only package and toolchain manager discovery

**Source checkpoint: November 9, 2026.** Workstream is stacked on DEV-04.6D
`2daaeea504f26394a02ef491b575569a8db1ffc4` on
`ide/dev05-1-package-discovery-20261109`. It is not a native Windows/Linux
acceptance pass or a permitted package installation.

## Goal and scope

IDE-05 is the standalone Lu IDE's *manager of managers*, **not** a new package
resolver, installer, programming-language runtime or autonomous application.
The initial implementation detects the presence and saved-byte revision of
fixed Python uv / C++ vcpkg / Conan manifests and locks in a user-selected
registered project. It deliberately does **not** run package-manager processes,
resolve dependencies, inspect versions via PATH, read credentials, contact
package registries, create virtual environments, install packages, modify
files or authorize downstream execution.

The host exposes `IdeWorkspaceRegistry::inspect_package_managers(project_id)`
from `ide/host/src/package_manager.rs`. The Tauri
`ide_inspect_package_managers(project_id)` command accepts only the opaque
catalog ID, not a path, manager name, command, package requirement, script or
authorization token. It runs in `spawn_blocking`, reuses the already
capability-scoped `source_capability` and
`probe_project_resource`, and hashes a fixed allowlist:

| Ecosystem / intended native manager | Manifest/lock observations | Interpretation |
|---|---|---|
| Python / uv | `pyproject.toml`, `uv.lock` | Both independently hashed: *candidate needs review*; pyproject alone does not establish uv choice; orphan lock blocks |
| C/C++ / vcpkg | `vcpkg.json` | Independently hashed file: *candidate needs review*, never executable permission |
| C/C++ / Conan 2 adapter | `conanfile.py`, `conanfile.txt`, `conan.lock` | Exactly one independently hashed recipe descriptor: *candidate needs review*; original Conan major version is unproven; Python descriptor is untrusted code that is never evaluated |

The host detects simultaneous vcpkg and Conan candidates, two Conan recipe
descriptors, orphan Conan or uv locks, symlink/non-regular files, oversized
or changed evidence, and unreadable native probes. It returns explicit
blockers rather than treating a failed read as absence. The resulting SHA-256
fingerprint covers the project ID, ordered filenames, observation states,
SHA-256 content digests, sizes, and candidate states. **This fingerprint is
not an authority token.** Each file is independently sampled, so the response
declares `snapshotConsistency=per_file_sha_not_atomic_resolve_requires_reinspection`.
The future Resolve operation must re-inspect all files before issuing a plan.

The existing Packages & Workflows UI has an explicit "Inspect project
package managers" action. It displays candidate states, blockers and exact
manifest SHA values. It rejects any backend result claiming network,
process, file modification, dependency resolution, package apply, or execution
authorization. All such result flags are hard-coded **false** in the host.
The UI has no Install, Upgrade, Remove or Run Package Script action.

## Implemented source tests / open acceptance

Five native Rust tests (added, not yet executed) cover empty projects, uv
lockfile detection and digest changes, conflicting C++ managers, malicious
Conan Python source not executed, a pure conanfile.txt classification, and
Unix symlink-denial. Four Node source-boundary tests (added, not yet executed)
check fixed filename allowlist, capability-scoped probing, Tauri command
scope and UI no-authority handling.

Acceptance gates for this full IDE-05 task group are **not** complete:
native Windows/Linux Rust compilation, actual filesystem/capability tests,
frontend build, hostile symlink/reparse race cases, >64 MiB lockfiles,
parallel file mutation, multi-project isolation, installed read-only GUI
discovery, and installation-worker security must be observed independently.
No per-commit GitHub CI was dispatched for IDE-05.1; use one consolidated
group gate after the forthcoming IDE-05 suborders are integrated.

## Next tasks

**IDE-05.2 — Verified static plan preview:** Recheck exact manager choice,
original manifest/lock revisions, deterministic lockfile and declared sources,
offline/no-network plan feasibility, script/hook risk and explicit expected
manifest/lock changes. The planner must not invoke any manager or silently
select among conflicts. It must distinguish data-only plans from
runtime-dependent resolution.

**IDE-05.3 — Native approved package worker:** Only after genuine Windows
SCM/AppContainer and Linux delegated-cgroup security gates pass, connect
reviewed fixed manager adapters with a separate native consent, no inherited
credentials, explicit registry/network controls, project-scoped writes,
bounded output, cancellation and durable process-tree cleanup. Never adopt
the D6 debugger consent or execution grant.

**IDE-05.4 — Independent result verification / recovery:** Hash actual
installed artifact tree and changed lockfiles, compare against plan, preserve
old lockfile and app journal receipts, fail closed on hooks, crash,
unexpected network or changed snapshot, and implement a separately reviewed
reconciliation path.

**IDE-05.5 — Installed native acceptance:** Real Windows/Linux protected
fixed-manager fixture matrix; authentic packages, no sys-global writes,
symlink/race, credential isolation, cancellation/restart, and a truthful
negative-case audit. Defer expanded JS/Go/JVM/.NET adapters until uv and C++
first-party worker evidence passes.

**DEV-04.6D stays OPEN** on its separate debugger workstream for authentic
Windows native compiler/LLDB/human approval/cancel/restart and release gates.
Starting IDE-05 does not imply those debugger gates passed.
