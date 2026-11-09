# IDE-05.3 — Protected package admission (offline uv Sync only)

**Source checkpoint — November 9, 2026.** Work is on
`ide/dev05-1-package-discovery-20261109`, stacked on IDE-05.2
`6c8e9eb8fb0f925a2c3cba44637871097657a354`.
The implementation is NOT native Windows/Linux verified, physically accepted,
or release enabled. The original DEV-04.6D debugger acceptance remains open.

## Chosen narrow first executor, not a general package manager

A package install is untrusted code execution. The established native WO6
build/test supervisor already owns input sealing, one-use D3 grants, Linux
delegated cgroup-v2/Bubblewrap, Windows SCM/AppContainer and durable restart
quarantine. IDE-05.3 **reuses that host/OS-worker path** rather than creating
a new subprocess launcher or granting a preview itself.

The sole newly admitted recipe is **`uv_sync_offline`**: Python uv
`Sync` with a valid `pyproject.toml` + `uv.lock`, clean exact Git HEAD
and both IDE-05.2 source-bound preview digests. The native host derives
`PackagePreviewRequest {manager: uv, operation: sync}` itself: it does not
accept package names, flags, arbitrary paths or executables from the renderer.
`Add`, `Remove`, vcpkg, Conan, online resolution, registry queries,
arbitrary dependency builds and project-local environment deployment
**remain unavailable**.

### Host-to-worker authorization chain

1. The new fixed Tauri command
   `ide_package_run_reviewed_uv(projectId, expectedPreviewSha256,
   expectedEvidenceFingerprint)` exists only as a functional dispatcher
   when `native-package-uv` is explicitly compiled. Default no-feature
   apps return `IDE053_PROTECTED_PACKAGE_RUNTIME_NOT_APPROVED`.
2. The trusted host reconstructs the hardcoded uv Sync request, reopens the
   OS-registered project and rehashes **all six fixed file observations**.
   Conflicts, symlinks, lock edits or source mismatch fail closed.
3. The host then enforces exact clean Git HEAD, sealed and verified
   `publish_input` source snapshot, and exact approved hash of a
   separate immutable **uv + Python + original license texts** runtime
   bundle. Missing release-provisioned runtime SHA/source/service results
   in denial. The first-party runtime scanner rejects untrusted
   symlinks/reparse and non-root-owned Linux bundle bytes.
4. The host **separately hashes the exact private staged**
   `pyproject.toml` and `uv.lock` bytes after snapshot publication and
   refuses any disagreement with the freshly reviewed source. The
   lockfile read uses no-follow handles and file-size/mtime guards;
   this blocks a source swap between public review and worker staging.
5. The D3 `JobPlan` uses fixed `recipe_id=uv_sync_offline`,
   `effect_class=package_uv_sync_offline:<preview-sha256>`,
   the source HEAD/tree, manifest SHA and runtime SHA; it has a stricter
   180-second runtime, eight-process cap, 512 MiB memory and 128 KiB log.
   The supervisor rejects malformed or different review SHA, elevated
   limits, unrelated grants and any debugger permission field. The
   preview/evidence string is **part of the exact durable JobPlan hash**;
   it is not itself a grant.
6. A separately labelled **OS-native 45-second, cancel-aware human
   dialog** presents the reviewed operation, original native source,
   full frozen input SHA, preview SHA and pinned uv toolchain SHA.
   The callback is inside the trusted Rust host; never WebView/LLM data.
   A refused, timed-out, cancelled or stale review cannot create a grant.
   The native host **revalidates the complete preview again after the
   operator decision**. Only then does it call
   `record_human_approval` for this prepared Job. D3's original in-memory
   one-use grant is consumed by the OS supervisor after real containment.
7. On Linux, the **existing** cgroup worker applies pre-exec
   membership/no-new-privs, readback of CPU/memory/PID limits and
   independent kernel child membership; Bubblewrap unshares all including
   networking, `--ro-bind`s the sealed source at `/workspace` and
   the installer-pinned runtime at `/runtime`, and `--bind`s a **private**
   output at `/output` (no write to original project). The fixed uv
   arguments are `sync --offline --locked --no-build
   --no-install-workspace --no-dev --no-config --no-python-downloads`,
   with a pinned interpreter and `UV_PROJECT_ENVIRONMENT=/output/.venv`,
   `UV_CACHE_DIR=/output/uv-cache`, `UV_PYTHON_DOWNLOADS=never`.
   No shell, arbitrary registry, manager option or project executable path
   is injected. Offline cache misses are **expected denied/failed jobs**
   until reviewed wheel data is available in a trusted bundle.
8. **Windows remains explicitly blocked from package execution**:
   the existing AppContainer provides zero network capabilities and OS
   Job/SCM custody, but its staged `LocalState/workspace` is not
   yet independently proven read-only against that package SID. The
   new uv recipe is rejected `NoContainment` **before OS profile,
   native approval, grant consumption or launch**. To enable it later,
   stage and attest immutable read-only ACLs for the workspace and
   runtime while leaving only `output` writable, test child-side
   denied source rewrites and prove SCM crash/cleanup on native Windows.
   This is NOT waived by the source-sealing hash.
9. Existing protected worker output capture, bounded artifact hashing,
   cancellation (explicit `CancelRequested`, kernel tree kill, terminal
   journal), recovery quarantine on service/host crash, and separate
   OS-only `ide_build_reconcile` are **reused without a second launcher**.
   If an approval is declined before process launch, the original D3
   supervisor writes a `Cancelled` pre-start terminal receipt; it must
   not label running/uncertain children as harmlessly cancelled.
10. Only private output is returned as native receipt evidence.
   The native supervisor uses a **separate exact-effect package
   receipt reviewer**, leaving the original Cargo-only receipt reviewer
   unchanged. `installedAppAccepted=false`; an exit-zero contained process and
   output tree digest are **not** proof of usable packages in the
   registered project, signed installer, resolved license/advisory closure,
   absence of malicious wheel content or production readiness.

### User interface

The existing Packages & Workflows screen, only after IDE-05.2 exact
revalidation, displays the separately feature-gated **offline uv Sync**
button and its runtime readiness. It passes only opaque project ID and
expected review/evidence hashes. It shows native Job ID, original
terminal receipt and private output digest, plus explicit cancellation
and OS-recovery buttons wired to the existing build supervisor.
No historical review button is an approval, and other manager actions
remain preview-only.

### Acceptance and development status

Source integration authored; two supervisor Rust tests, one native host
Rust test and seven source-boundary Node tests added. Existing IDE-05.1
and IDE-05.2 native tests remain in the final group.

**Not run/accepted:** native Windows/Linux Rust compilation;
Linux runtime with exact approved uv/Python/LICENSES bundle;
real offline Sync of a bounded wheelhouse; wrong pin, malicious sdist,
editable backend, local path or external URL dependencies; live
cancellation before/after human consent; forced worker/service crash;
reopened original durable journal; malicious symlink/mtime/output
quota; installed Tauri UI, real human dialog, readonly Windows ACL,
package destination verification or signed release.

The owner rule remains **one consolidated native gate at the end of
IDE-05's task group, not per-file CI dispatches**. The task can proceed
to IDE-05.4 independent output, provenance and rollback/reconciliation
without claiming source changes are native acceptance.

Official uv semantics used in this narrow executor:
https://docs.astral.sh/uv/concepts/projects/sync/
and https://docs.astral.sh/uv/concepts/projects/config/
with CLI reference https://docs.astral.sh/uv/reference/cli/.
