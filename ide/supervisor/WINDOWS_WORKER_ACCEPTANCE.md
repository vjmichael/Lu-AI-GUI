# Lu IDE Wave D7 — Windows native worker integration

**State:** Draft implementation, gated off by default. Native Windows CI and hostile installed acceptance are not yet established. The standalone IDE and Lu Core must not import this worker as an executable desktop command.

## Source and authority

- `ide/supervisor/src/windows_worker.rs` is compiled only on Windows with the opt-in `windows-worker` Cargo feature; default build has no native Windows launcher and no Tauri/WebView `job.run`.
- The native-only `Supervisor` accepts a fixed `JobPlan` with registered project, exact Git HEAD, immutable source tree SHA-256, Cargo manifest SHA-256, trusted offline bundle SHA-256, fixed recipe and resource budgets.
- `record_human_approval` produces a fresh session-local, one-use grant that cannot be serialized, reused, or created by historical IDE D1 review notes.
- D7 consumes the grant through `consume_grant_after_attestation` after staged source+toolchain verification and kernel Job configuration. The child is still suspended when the Running receipt is fsynced.

## Verified immutable input and toolchain

- A previously published `PublishedInput` must satisfy `verify_input`; its identity must exactly match the plan. The broker recopy-verifies that snapshot into an exclusive, per-job AppContainer LocalState workspace.
- A **native host-selected** offline toolchain bundle must have `bin/cargo.exe`, `bin/rustc.exe`, `lib`, and `cargo-home`. The broker bounds files and bytes, rejects Windows reparse points and invalid names, copies only regular files, and checks both source/copy tree SHA-256 against the planned toolchain digest.
- The bundle must already include all registry/cache/sysroot/linker dependencies necessary for the requested Cargo recipe. No rustup download, PATH search, shell, network fallback, user-selected command, or ambient credentials are allowed. Offline Cargo recipes are `rust_check`, `rust_tests`, `lu_ide_host_tests`.
- **Open integrity gate:** handle-relative directory traversal and concurrent same-user path-swap acceptance are still required. A digest alone does not establish a signed installer-origin trust chain or protect against hostile same-account mutation.

## Windows kernel execution

- A fresh zero-network-capability AppContainer profile is created per job. Fixed Cargo and source inputs are staged inside the profile-owned LocalState.
- Process creation uses `SECURITY_CAPABILITIES` and `PROC_THREAD_ATTRIBUTE_JOB_LIST` in a two-entry `STARTUPINFOEXW` attribute list. The kernel attaches the child to the unique named Job **during CreateProcessW**, eliminating the post-spawn/pre-Job-assignment parent-crash orphan window.
- No handles are inherited. The suspended child's primary token is checked with `TokenIsAppContainer`, and its `TokenAppContainerSid` is compared against the expected profile SID before `ResumeThread`.
- The Job has kill-on-close, fixed 50% hard CPU-rate control, plan memory and active-process caps, and no permitted breakaway. A missing OS API or quota denies launch. The bootstrap environment is explicit and limited to needed platform/Cargo variables, including `LOCALAPPDATA`, `SystemRoot`, `CARGO_HOME`, `CARGO_TARGET_DIR`, `RUSTC`, `PATH` to sealed tool binaries, and `TEMP/TMP`. No parent secret variables are forwarded.
- The D6 fixed probe previously failed on ERROR_ENVVAR_NOT_FOUND (203). The newly added `LOCALAPPDATA` fix is **not yet independently accepted** by native CI.

## Cancellation, output, and receipts

- Native `WindowsCancellation` or watchdog invokes the existing supervisor `cancel_running_with`, which **fsyncs CancelRequested before kernel TerminateJobObject**, then requires zero active Job processes before Cancelled can be recorded.
- If admission, token verification, wait or kill outcome is uncertain, `native_quarantine_after_error` immediately quarantines the active supervisor, tries to durably append RecoveryRequired, and denies further jobs. Any journal I/O failure must not become a success.
- After a normal child exit, all descendants must be reaped or terminated. Only bounded, no-reparse outputs are copied from AppContainer output into a separate native-profile publisher directory. Source output and published copy are SHA-256 rechecked; `native_finish_with_output` links the published SHA-256 into the hash-chained durable terminal receipt. No cancelled/partial outputs are published as successful results.
- The UI cannot forge a completion, output digest, OS kill receipt or authorization grant.

## Crash recovery limits

- On restart, the replayed journal quarantines AdmissionPending, Running, CancelRequested and RecoveryRequired jobs.
- `reconcile_windows_interrupted` NEVER relaunches Cargo. It derives the exact name of the existing OS Job from native ID + recorded plan SHA-256, opens it with query/terminate rights, calls native termination, checks active membership zero, then records Cancelled.
- If the named Job is missing after crash/OS restart, there is **no surviving OS handle to attest cleanup**. The worker remains quarantined/uncertain instead of falsely reporting cancellation. A durable independent broker/witness mechanism is still required to fully close this case.
- The worker is not installed in Lu IDE; a physically installed Windows app build/run/cancel/restart exercise and Windows-native hostile filesystem/network proofs are open.

## Acceptance gates before activation

1. Exact-head Windows `cargo check --features windows-worker --all-targets`, native Rust tests, `rustfmt --check`, and the existing standalone IDE regression gates.
2. D6 fixed AppContainer launch+host-file+live socket denied-access acceptance on native Windows, including cross-volume `LOCALAPPDATA` behavior.
3. A real offline Cargo check/test using a fully pinned Windows toolchain/cache and SDK bundle inside D7 AppContainer, enforcing Job CPU/memory/PID caps and no network/host data.
4. Concurrent reparse/junction/rename races against both staging and output publication, process-tree descendant and kill injection, crash at every spawn/receipt boundary, plus independently witnessed post-crash orphan cleanup.
5. Installed Windows desktop acceptance; only then consider designing a narrowly scoped human-visible Run action and Lu Core bridge.

**No claim of full Wave D completion from a source commit or a green compile-only workflow.**
