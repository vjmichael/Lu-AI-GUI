# IDE-INT03 — Protected Build/Test Workspace Integration

**Status:** Implementation slice complete in source; protected native and installed application acceptance deferred. Continue build-first on the existing `ide/int01-editor-workspace-integration-20261111` implementation branch. The existing WO6 supervisor is authoritative; this group does not create another executor, permission layer, subprocess mechanism or uncontained fallback.

## INT03-A — Exact reviewed Build/Test profile ownership (source implemented)

- Read-only native project discovery already enumerated four Cargo profiles and three fixed Python/Node/CMake profiles, their saved-manifest SHA-256 and bounded conventional test filenames.
- The user must now select an **available** fixed profile before requesting execution. A new `protected_build_profiles.js` contract requires the exact current registered project, observed saved manifest SHA, supported fixed recipe and provider-specific **native offline runtime readiness**. This selection is not a grant.
- The new Tauri `ide_build_run_profile` path accepts only `projectId`, fixed `recipeId`, and the expected saved manifest SHA-256. Native `run_profile` validates the registered manifest, freezes a complete bounded snapshot and remeasures the manifest's identity inside its seal. It checks the saved manifest again after the **native** user approval dialog before consuming the original OS-supervised grant.
- No project-supplied command, script, test name, arbitrary environment, path, executable or runner argument becomes an execution capability; unsaved drafts are not silently substituted for saved source.

## INT03-B — Fixed offline adapters and native isolation (source integrated)

- Reuses the existing protected Rust Cargo `check/build/test` and IDE host-test routes.
- Python pytest, Node builtin tests and CMake/Ninja/CTest are enabled for selection only when the existing `native-multilang` protected runtime is present, pin-verified and reports a recipe-specific readiness receipt. The native OS worker still independently enforces immutable snapshot, toolchain hash, one-use human confirmation, bounded execution/output and process-tree custody.
- Status now distinguishes `cargoReady` from recipe-specific multilang readiness: a pinned Node runtime cannot falsely make the Cargo profile runnable. The generic package `uv_sync_offline` is excluded from Build/Test and remains behind its separate package-specific review gate.
- The existing durable supervisor denies any new Build/Test job while *any* original kernel job requires reconciliation; no other profile or project can be used to bypass its quarantine.

## INT03-C — Results, progress and read-only failure hints (source implemented)

- The original current-process result is bound to the selected project, recipe, frozen manifest, complete plan/source/toolchain SHA, native job ID, terminal receipt hash, phase, OS exit and reconciliation state. A zero-exit completed native job is not promoted to installed acceptance, task approval, per-test assertion proof or release authorization.
- Existing bounded stdout/stderr and native phase timeline now include the fixed feature-gated multilang recipes. New advisory hints recognize bounded compiler/test output excerpts, reject absolute/traversal/foreign source paths, and remain **non-navigable/unverified text**. This is intentional: a worker log cannot provide the per-file saved revision required for verified CodeMirror navigation.
- Displayed native artifact evidence stays read-only and separate from real OS cleanup and future debugger authorization. Source-level regression cases were added for provider readiness, stale manifest, receipt provenance, quarantine and hostile output; no execution claim is made from their presence.

## INT03-D — Cancellation and interrupted worker lifecycle (source integrated)

- Existing native cancellation, kernel process-tree verification, original D3 journal, explicit recovery and non-resumable restart are retained.
- The interrupted-worker cleanup path no longer requires a Cargo runtime to be installed when the original job was Python/Node/CMake: Windows can reuse the **same original independently pinned** protected SCM recovery broker from a verified native multilang bundle; Linux uses its original OS-owned interrupted-job reconciliation. Neither route automatically relaunches project code or assumes an orphan is dead.
- UI distinguishes `reconciliation required` from a valid completed receipt and blocks new work while a quarantined job remains.

## Deferred / open

- **Physical/compiled native acceptance:** Windows AppContainer/SCM and Linux delegated cgroup/Bubblewrap normal, failing, cancelled and crash/restart paths on the final exact head. No GitHub Actions or PR verification initiated during this build-first pass.
- **Release assets:** Reviewed offline Cargo, Python, Node, CMake/Ninja/CTest binaries and dependency closure, exact installer pins and actual installed-app entitlement. Without real approved resources, the UI correctly keeps a profile disabled.
- **Structured test evidence:** Per-test identities/assertion counts/framework-specific machine-readable results and verified source-linked failure navigation remain a possible subsequent INT03 task slice; the present bounded stdout/stderr hints explicitly do not assert these facts. The task evidence/acceptance register must not infer human completion from a successful native exit.
- **Physical UI acceptance:** Restart, keyboard/assistive presentation, progress/cancellation, original files and human consent. Signing and merges remain separately governed.

**Next implementation option:** INT03-E — source-verified structured test-case evidence and failure navigation, if desired, before proceeding to INT04 debugger integration. Existing prior DEV-04 project debugger work should be reused, not reimplemented.
