# Lu E1 — host-reviewed application admission before general execution

**Started:** November 7, 2026. **Branch:** `lu/e1-reviewed-app-admission-20261107`, stacked on the open M5 draft PR #101. M3 PR #96, M4 PR #98, and M5 PR #101 retain their exact native/model evidence requirements; deferred real-world Windows testing is reserved for the **pre-v1.0 release candidate**, per owner direction.

## Purpose and architectural boundary

M1–M5 established a source-backed ANSF/SQLite conversation, task, semantic/procedural memory and recovery candidate with native CI; model measurements and physical installed-app checks are separately pending. The next major execution-layer requirement is **reliable reuse of existing applications** rather than more specialized mini-apps or an unbounded shell. The existing `native_external_app_action` registers only read-only host implementations, and `native_background_process` already has host-owned reviewed executables and process lifecycle; those are not the same as a model-facing general application executor.

**E1-P01 establishes the first common binding:** a *host-reviewed application identity* (local file, exact digest, host-reviewed version label and narrow read-only tool ID) and a *current authenticated user task* must both pass before Lu even presents a read-only executable-availability observation. Merely finding an app, retrieving an old approval, or guessing the command from a document never authorizes application execution. This work explicitly **does not launch any program** or register new model tools.

## E1-P01 source implementation

`src-tauri/lu-core/src/native_app_admission.rs`:

- Checks host-owned application/tool/workspace/version identifiers and SHA-256 format. The model has no `command`, `argv`, `cwd`, path, URL or credential parameter in the preflight surface.
- Requires the live `ConversationAuthority::authorize_worker_call` check on the exact host-bound conversation/task/user-turn, tool effect and workspace. It honors the separate `ToolAuthorizer` veto. Only `ReadWorkspace` can pass.
- Verifies an absolute local ordinary executable, rejects symlinks/reparse points in **any** ancestor, requires an actual Linux ELF or Windows PE signature, refuses missing, empty and oversized files, and compares the actual complete SHA-256 to a previously reviewed host digest. It rejects changed file size or modified time during the observation.
- Returns a bounded structured native observation: app ID, host-provided version label, tool ID, observed byte length, format, SHA-256. It explicitly records **`process_started=false` and `functional_output_independently_verified=false`**. It never returns file content or opens credentials.
- The version label is **not proof** that the app reported this version at runtime. A valid file SHA is **not** OS file-handle custody during process launch. A later executor must separately obtain OS file identity, rehash/reopen at spawn, enforce containment, journal effects, honor cancellation/revocation and independently verify outputs.

Tests use the *actual native Rust test executable* as the genuine local PE/ELF fixture (not a mock file), plus wrong hashes, bogus format, missing binaries, changed workspace/effect, model-impersonated turn, host veto and task revocation. They assert no process launch or semantic validation has been represented as a success. This is a Linux/Windows Core candidate, not a physical installed Lu acceptance result.

## Follow-on task group — E1-P02 through E1-P05

1. **E1-P02 — app availability and version catalogue.** Host-selected trusted external app records, with optional explicit profile scope, resource/capability requirement, real version probe where safe, independent path/OS file identity and availability-on-demand; do not advertise an unavailable application to the model.
2. **E1-P03 — contained execution with narrow typed actions.** Reuse the existing `BackgroundProcessSupervisor`, `ConversationAuthority`, host resource broker and `EffectJournal`. Add operations only after a *separately approved* effect class and exact resource/argument contract can be verified. No ambient shell or model-selected binary/CLI. Enforce process-tree cancellation, output limits, OS containment, network/credential denial and single owner on startup.
3. **E1-P04 — independent outcome oracles.** Compose tested application-specific parsers/readers/validators with typed receipts. A zero-byte XLSX, mismatched document cell, wrong model answer, invalid side effect or uncertain journal is not a verified success.
4. **E1-P05 — same-task benchmark and integration.** Compare direct model knowledge, existing safe utilities and an *actual* authorized external app action against disposable representative user tasks with K/I/A/F attribution; measure success, source/recovery, latency, repeat rediscovery, memory cost, and unauthorized effects. Headless Linux/Windows CI first; **real-world Windows WebView/AMD Vulkan and signed installer tests deferred to near-v1.0**.

## Acceptance and dependencies

- **Current submission:** E1-P01 preflight source plus native tests; source lock and CI to be checked at the exact submitted head. No source-only pass claim.
- **Kept open:** M3, M4, M5 genuine local GGUF measurements, native source-specific CI where queued, and pre-v1.0 physical Windows/NSIS/Vulkan tests. The Windows deferral is a *scheduling decision*, not proof of installed acceptance.
- **Do not grant:** arbitrary shell/process/API/GUI automation, network, code execution, new root access or historic approval reuse through the existence of this E1-P01 module.


## E1-P02 — reviewed application catalogue, current binary and version evidence

**Implementation submitted, native CI pending.** This P02 slice creates a production Lu Core host-only \`HostReviewedApplicationCatalog\` in \`src-tauri/lu-core/src/native_app_catalog.rs\`. It does not auto-register any app, execute a version command, accept PATH as a trustworthy executable, install packages, or activate a model tool by itself.

### Host-owned catalogue

- The trusted host enrolls an exact app ID, reviewed release version label, workspace, native read-only tool ID, absolute installed executable, and SHA-256 of the reviewed release's binary. Limits: at most 64 enrolled apps / 64 exposed tools per discovery call. Duplicate app IDs or tool IDs and invalid host records are refused.
- On every \`discover_for_current_task\` call, Lu requires the current persisted user-turn authority, exact tool descriptor, read-only effect, matching workspace and independent host policy. Only reviewed tools actually exposed by the trusted host are candidates. Unregistered tools, denied tasks and missing/stale binaries do **not** appear in the result list; the module deliberately does not leak denied filesystem details to a model.
- The preflight reopens the actual ordinary local executable, validates ELF/PE bytes, hashes the whole binary, checks the file size/time against the opened handle, obtains real OS handle identity (Linux/Unix device+inode; Windows existing safe \`lu-win-worker-job::file_guard::opened_file_identity\` bridge, volume+128-bit ID), and reopens the path to check that the identity did not change during the preflight. A file replacement with new bytes, changed digest or swapped path cannot be advertised as the reviewed release.
- \`delist_reviewed\` removes an entry from the host catalogue; this is **not** a substitute for revoking any existing user task. Discovery rechecks the live authority every time, so revoked grants cannot reappear through cached inventory.
- Observations are bounded data: app ID, native tool, reviewed release version, checked digest, file format, byte count and OS identity token, plus explicit \`process_started=false\` and \`output_verified=false\`. No file path or command line is exposed by discovery.

### What “version verified” means at P02

The returned version is a **host-reviewed label cryptographically bound to the on-disk executable digest**. If binary bytes differ from the host's per-version SHA-256, the candidate is unavailable; a newer binary must be separately reviewed and re-enrolled. The result explicitly states \`version_basis=host_reviewed_version_bound_to_matching_binary_sha256\` and \`independently_queried_runtime_version=false\`.

It does **not** claim that \`--version\`, a PE VERSIONINFO resource, a publisher signature, a package manager, a child process, or a live application API actually reported this version. Safe independent runtime/product version interrogation, where required, belongs to the separately approved contained E1-P03 adapter. Merely matching the reviewed label does not prove real application functionality or install readiness.

### Verification matrix and release boundary

- CI: genuine Linux and Windows native Rust Core tests must pass at the **final exact head**, including P01 fixed Rust test type/structure, positive catalog discovery, expired/revoked/forged/host-veto denial, duplicate and changed-byte identities, stale digest, unsupported format and Windows/Unix OS identity.
- Source code alone is not passing test evidence. No new OS privilege, shell, browser access, arbitrary app controller, model-selected executable or effectful tool is registered.
- **Physical installed Windows/AMD Vulkan/NSIS testing remains deferred until the Lu v1.0 release-candidate phase** by owner decision. GitHub-hosted native Windows test results are not a claim that the owner's installed Lu has run.
