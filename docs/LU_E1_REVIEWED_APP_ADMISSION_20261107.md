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


## E1-P03 — contained, reviewed ONE-SHOT worker execution

**Implementation candidate** on separate stacked branch \`lu/e1-p03-contained-app-worker-20261107\`, based on E1-P02 PR #103. E1-P02's standalone Windows/Linux CI should continue without being replaced by this work. Owner’s physical installed-Windows acceptance is still deferred until closer to Lu v1.0; hosted Windows/Linux genuine native process tests remain required immediately.

### First real execution contract (bounded deliberately)

\`src-tauri/lu-core/src/native_app_execution.rs\` is a **host-only execution broker** for a single exact, audited operation: the first-party \`lu-e1-version-worker\` with \`--lu-e1-version-one-shot\`. Host registration provides the exact executable file, expected SHA-256 and reviewed version, as introduced by P01/P02. The model supplies **NO executable name, argv, working directory, environment, PID, network address, filesystem target, or verification result.** A separate current \`ExecuteCode\` capability is required; the read-only availability discovery grant is explicitly insufficient.

Before launch, the broker checks the current trusted user-turn/task/workspace and independent host policy, reopens and hashes the actual PE/ELF executable and its OS identity, then durably writes \`PREPARED\` → \`EXECUTING\` to \`EffectJournal\`. A duplicate or \`UNCERTAIN\` call ID is denied rather than retried. It delegates the actual process to Lu’s existing \`BackgroundProcessSupervisor\`, which owns the child, clears the environment, closes stdin, constrains stdout to the small native progress protocol, discards stderr, uses a fixed executable-parent directory and a 15-second host deadline.

- **Windows:** the existing suspended-launch/Job Object implementation owns the process tree and receives an explicit **256 MiB job memory cap** for this one-shot mode.
- **Linux:** existing process group and \`PR_SET_PDEATHSIG\` host-death binding are used; this mode receives a **256 MiB RLIMIT_AS**. These protections do NOT independently prove full network namespace isolation or absence of all filesystem effects in arbitrary third-party binaries; arbitrary external application execution remains disabled.
- **Revocation:** a synchronous user-task-revocation listener fences launch with the same mutex, then cancels the exact owned worker and conservatively marks the effect \`UNCERTAIN\`. A host-originated cancellation also checks the exact original conversation/task identity, not a model-provided PID.
- **Completion:** supervisor proves process exit and a bounded \`P\`/\`D\` protocol receipt. Lu’s independent task store persists the source receipt, and the broker compares it against the **host-known literal one-shot version-marker digest**. Only then can the *specific process action* reach \`APPLIED\`; receipt JSON explicitly says \`independentApplicationOutputVerified=false\`. A malformed/truncated receipt, timeout, failed worker, aborted launch or cancellation stays \`UNCERTAIN\`, never automatic retry.
- **Crash/restart:** only the exclusively restarted host may reconcile SQLite \`RUNNING\` to \`INTERRUPTED\` and journal \`EXECUTING\` to \`UNCERTAIN\`. A current worker must never be misclassified as crashed just because a model loses context.

### What E1-P03 actually executes

Two genuine native Rust example binaries are built from source, not fake file descriptors or monkeypatched OS calls:

1. \`e1_reviewed_version_worker\` emits the strict, bounded completion marker without accessing a document, shell, network or credentials.
2. \`e1_reviewed_slow_worker\` sleeps before the same marker so a real running OS process can be cancelled and reaped by Lu’s process supervisor.

The dedicated \`src-tauri/lu-core/tests/e1_reviewed_execution.rs\` checks a launched real worker and durable post-restart journal, the actual revoked process kill/reap, wrong-origin cancellation, host veto, changed binary digest, duplicate ID denial and interrupted startup reconciliation. Tests depending on external built examples are **explicitly ignored in ordinary Cargo tests** and are **explicitly run using \`--ignored\` in a dedicated real Windows/Linux GitHub matrix**, which builds and supplies the binaries by absolute path. An ordinary core unit-test pass must never be counted as these OS tests.

### Remaining boundaries before general-purpose app operations

E1-P03 is **an initial safe reviewed process execution primitive**, *not* arbitrary CLI/script/general third-party application execution or a user-installed-app result. Full E1-P03 completion still requires a separately audited typed/model-facing host registration, OS resource and network/credential policies appropriate to each external application, argument-level resource broker, and tested cancellation/timeout for each allowed operation. E1-P04 adds independent semantic output validation (e.g. expected worksheet cells rather than a process-exit or digest marker), and E1-P05 compares outcomes against direct-model and existing utility baselines. No new model-facing \`ExecuteCode\` tool has been automatically registered.


## E1-P03 cancellation/recovery closeout review — November 7, 2026

**No actual GitHub native runner result has been obtained yet** for E1-P03 commit \`149d888eb1f3d3246964173f53cf115aebfe8c15\`. At inspection, the dedicated Windows/Linux run \`42441697036\` remains QUEUED at frozen-source, and Desktop CI \`42441697004\` / source identity \`42441696902\` are queued. Do not mark cancellation/OS descendant containment/journal acceptance complete from code review.

### Identified real code-level cancellation race, repaired in next commit

The initial \`ConversationAuthority\` revocation listener removed the active broker entry *before* verifying \`BackgroundProcessSupervisor::cancel_owned\` had actually terminated and reaped the owned process. Similarly, \`cancel_current_task\` left the journal conservatively \`UNCERTAIN\` but discarded no failed-cancellation custody and could later publish a normal \`APPLIED\` receipt for the same completed worker. This was a genuine fail-closedness gap discovered by source inspection, **not a fabricated CI failure**.

The broker now:
- Marks its active entry \`revoke_pending=true\` and marks the journal \`UNCERTAIN\` **before** attempting OS cancellation.
- Removes the entry immediately only when \`cancel_owned\` returns success after OS process reaping. If cancellation cannot be confirmed, the broker **retains** the entry and returns \`WorkerCancellationUncertain\`.
- A subsequently observed terminal task status can clear retained tracking through \`poll\`, but a \`revoke_pending\` entry is never eligible for an \`APPLIED\` publication, even if a late valid protocol digest is present.
- A real native fast-worker regression deliberately lets the underlying supervisor reap the process before the E1 broker receives the completion. Task revocation then sees no owned child to kill, must retain custody until broker reconciliation, and must preserve the journal as \`UNCERTAIN\`.
- Another real native fixture deliberately exceeds the fixed 15-second runtime deadline; the test requires \`TimedOut\`, owned worker absence, no verified result, and \`UNCERTAIN\` durable journal with replay denied. These are appended to the dedicated Windows/Linux \`--ignored\` actual-worker test matrix, not counted through ordinary unit tests.

### Evidence gates

**Required:** on the final exact commit, frozen source passes, Linux+Windows core compile succeeds, the two pre-existing OS-native fast and slow tests pass, late-revocation and deadline-expiry native regressions pass, real OS worker/process-tree reaping is witnessed by supervisor APIs, durable SQLite journal remains \`APPLIED\` only for the successful fixed protocol worker and \`UNCERTAIN\` for aborted/missing/ambiguous operations, and historical call IDs do not execute twice. The full dedicated matrix must have real job logs and uploaded evidence. Do not close E1-P03 before that.

Real user's installed Windows/NSIS/Vulkan acceptance is independently deferred until near Lu v1.0; hosted Windows 2025 CI is still required now.


## E1-P04 — independent output/semantic verification candidate

**Branch:** \`lu/e1-p04-independent-outcome-oracles-20261107\`, stacked on the still-draft E1-P03 PR #105. Physical owner's Windows installed-app trial remains deferred near Lu v1.0; Linux/Windows native source and actual opened-file tests must run now.

### New production native oracle

\`src-tauri/lu-core/src/native_app_output_oracle.rs\` implements the first **host-only** independently checked outcome contract. It does not run any shell or application, does not register a model-facing tool by default, and never accepts a model-selected file path.

- The trusted host supplies an **already-opened, independently admitted file handle** and the exact user-approved expectation (original user turn, P03 process call ID, workspace, revision, ordered key/value rows and expected SHA-256). Model claims are not approval. A current and separate \`ReadWorkspace\` grant for \`lu.e1.output.verify\`, plus host policy, is verified under the existing bounded \`ConversationAuthority::with_authorized_worker_effect\` so synchronous revocation cannot race the verified SQLite receipt commit.
- Before touching result content the oracle requires the genuine persisted P03 journal effect in \`APPLIED\` and a strictly decoded 4 KiB-or-less worker receipt with the exact known P03 operation/call ID and **independentApplicationOutputVerified=false**. It also requires the real \`TaskStore\` task \`COMPLETED\` in the same workspace and the genuine scanner proof of the reviewed, host-known P03 version-marker SHA-256 and byte count. Model text cannot promote its own answer to a receipt.
- Reads up to 128 KiB from the **opened** ordinary file, restores caller's original stream position, obtains OS opened-file identity (Unix dev/inode; Win32 volume/native file ID through Lu's existing safe FFI bridge), compares size, OS identity and modified time across the read, and computes actual full-file SHA-256. The host must separately enforce workspace membership and resource permissions when granting that handle.
- Parses a strict, schema-named JSON report \`lu.e1.structured.report.v1\` (task ID, revision, and bounded ordered \`{key,value}\` rows). Unknown or duplicate fields, duplicate row keys, wrong values/revision, missing or binary bytes, hash mismatch, or overlong content are **not** successful outcomes. Actual semantics must match the immutable human-approved expected rows exactly. This does not verify arbitrary image, PDF, XLSX spreadsheet formula, audio/video or graphical layout semantics; those will use format-specific independent oracles reusing Lu's existing readers.
- Writes a small durable receipt to its **separate** \`lu_e1_output_verifications\` SQLite table: process call ID, workspace, original user-turn ID, observed SHA, byte count and opened file ID. No file text, untrusted path, API credential, executable command, transcript or model output is copied into memory SQLite. Same-bytes rechecks are idempotent; a different file/digest for an existing process ID is refused as \`ConflictingReceipt\`. After crash/restart, a saved receipt can be found but cannot mean a re-opened artifact is still valid; each new approval must reverify current bytes.
- Reports three distinct truths: \`exact_semantics_verified=true\` for the opened file snapshot, \`artifact_producer_independently_attributed=false\`, and \`external_application_output_verified=false\`. A matching JSON file that the host opened is **not proof** that P03's worker produced it. Do not promote these to true until an OS-granted and durable worker-to-artifact handoff is proven.

### Native evidence plan

New E1-P04 native tests open actual local files, verify approved key/value semantics and real OS file identity, persist and reopen SQLite, and reject forged P03 receipt fields, stale model user-turn claims, host-policy veto, revoked authority, wrong workspace, changed row/revision, duplicate/unknown fields, and mutated artifacts after acceptance. The tests **explicitly construct a P03 journal fixture** rather than pretending to have executed the P03 OS worker. The genuine real-worker tests remain in E1-P03's own dedicated workflow. The E1-P04 matrix runs source lock + native oracle and existing artifact parser tests on hosted Linux and Windows. Never claim installed Windows or genuine worker-generated artifact evidence from these fixture-based semantic tests.

### Pending gates

1. E1-P04 final exact-head Windows/Linux native test results, repair any concrete compiler or regression failures.
2. A real supervised P03 worker producing an artifact into a host-owned, capability-scoped temporary output handle, with independent ownership/producer attribution and no path override or partial-file publication. The current first-party version worker has only a marker, not an artifact.
3. More independent validators for actual requested formats (CSV/XLSX cell values, DOCX content/structure, PDF rendered outputs, image/video/audio constraints) with host-supplied expected outcomes. Use existing parser modules rather than treating mere file presence or format as task success.
4. E1-P05 comparative real task benchmark with independent K/I/A/F evidence and measured latency, failures and unauthorized effects. Physical installed Lu/Windows GPU testing remains near v1.0 by owner decision.

**Acceptance status: source candidate only; source inspection is not native CI proof or a general third-party application acceptance.**


## E1-P04B — genuine owned worker stdout to independent Office artifact custody

**Source candidate** stacked on E1-P04 draft PR #106; full Linux/Windows native and real-worker CI must independently pass at the exact final commit. This closes the *implementation gap* for the reviewed first-party test path; it is not general arbitrary third-party binary execution and is not physical installed-Windows acceptance.

\`native_background_process\` extends the existing host-owned worker stdout protocol with exactly one optional bounded \`A<TAB>kind<TAB>bytes<TAB>sha256\` binary segment after normal \`P\`/\`D\` progress. It is admitted **only** for host-selected \`--lu-e1-artifact-one-shot\` with \`docx\` or \`xlsx\` fixed argv, verified executable SHA, a 15s supervisor deadline, 256 MiB OS memory budget and existing Job Object/process-group ownership. It requires full bytes, correct digest and EOF with no trailing junk. Unrecognized artifact bytes, altered hashes, wrong type, truncated stream or unexpected binary mode fail the supervisor proof. Private bytes are held in a one-use, exact workspace-scoped \`CapturedArtifact\`; memory is scrubbed on drop and never copied into ANSF/SQLite or a model prompt.

New \`native_app_artifact_custody.rs\` is a host-only action broker: separate original-user-turn \`ExecuteCode\` and \`WriteWorkspace\` grants, exact reviewed application identity, fixed no-model-args protocol, independent host policy, and durable execute/write intent journal. The worker never has or chooses an output path. Only after actual native supervisor reaping and progress proof does the broker take the private child-pipe bytes, check the complete SHA, parse the **actual OOXML** using existing independent \`native_artifact_inspect\`, \`native_reader_docx\` or \`native_reader_office\`, and require **exact approved document text or worksheet cell values**. It then publishes create-new only through the trusted host \`cap_std::fs::Dir\` using the existing atomic-write primitive and an unpredictable host-selected stage ID, reopens the new file to verify exact bytes and records distinct durable process and publish receipts.

The truthful custody outcome is: **produced by the reviewed first-party worker's host-owned stdout pipe, child reaped, independent content verified, file published to the host's exact approved directory**. This does **not** establish a general third-party renderer or office application wrote the file, nor that a complex feature-film edit or Office formula calculation is correct. The initial fixed document and workbook are deterministic samples; user-controlled arbitrary document contents and cross-app output handles need further typed adapters and isolation before exposure.

The end-to-end native matrix actually compiles a reviewed DOCX/XLSX producer, a real slow worker for process cancellation and a forged-content worker which sends correct transport SHA but invalid OOXML. It runs real Linux/Windows OS subprocess tests using \`--ignored\`, verifying producer pipe custody, independent semantic validation, exact create-new publication, receipts surviving SQLite restart, format disagreement, malformed OOXML, wrong-user-origin cancellation, revocation, invalid old call ID, and wrong workspace. Ordinary Cargo tests alone **do not** count these OS integration cases.

Open review items: actual exact-head compiler/native test receipts; narrow typed app-adapter integration into the Lu desktop host, separate full external-app sandbox/resource policy, and general document/spreadsheet authorship beyond fixed reviewed test fixtures. Owner's physical Windows test remains scheduled near v1.0.
