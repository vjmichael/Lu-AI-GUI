# LU-IPC-01D-R1 — Lu Protobuf over Windows named pipes and Linux Unix-domain sockets

**Status: isolated source-only adapter candidate, NOT installed or authenticated production IPC.**
Source parent: detached LU-IPC-01B contract candidate 62a9736f. Lu, IDE, CI-policy and mainline refs are untouched. No new CI/PR or OS installer change.

## Prepared in this source candidate (not runtime-qualified)

- One common Protobuf lu.workspace.v0.WorkspaceFrame codec and big-endian-u32 length prefix independent of OS. Maximum frame 256 KiB; length checked before allocation. Zero/oversized, partial, noncanonical, invalid-schema and incompatible protocol versions fail closed. Three-second timeout per framed I/O operation.
- Mutual challenge–response: host first checks OS-peer admission through an injected trusted PeerAdmission policy. Client and host exchange HMAC-SHA256 proofs over canonical Hello, a host-generated 32-byte nonce, and the host session/epoch. Domain separation prevents reusing client proof as a server proof. Keys are externally provisioned, never supplied through Hello or compiled into product defaults. A valid HMAC proves enrolled-secret possession, not consent or an effect grant.
- Host must provide secure CSPRNG nonces, unique session/epoch refs, and a replay/secret-rotation policy. This module deliberately does not choose predictable nonces or store credentials. Reusing nonces, stealing keys or using a permissive trust policy destroys the handshake guarantee.
- Linux adapter only binds under an existing real private directory owned by the expected UID with permissions 0700, refuses existing endpoints, sets 0600 socket permissions, and observes Linux kernel uid/gid/pid via Tokio peer_cred. A separate host enrollment policy still must verify actual process image/package, account, lifetime and profile; the digest in Hello remains untrusted.
- Windows adapter uses Tokio local named pipes in byte mode with FIRST_PIPE_INSTANCE, remote-client rejection, one instance and a strict 32-lowercase-hex Lu endpoint suffix. The client uses Tokio's default identification impersonation level. CRITICAL: a connected named pipe does not independently prove its client's Windows SID, token, package signature or process. Source returns WindowsUnverified, and the host handshake intentionally refuses it. The installed Windows OS token/ACL attestor remains a blocking task.
- Post-handshake session matching does not authorize any model/tool/effect. ConversationAuthority, native provider-owner verification, per-user-turn permissions, durable journal and cancellation/uncertain-outcome recovery must still be checked per call. No Docker, HTTP server, MCP or duplicate model runtime is introduced.

## Source tests added (not executed)

1. Bounded u32 framing, zero/oversized/truncated-frame refusal, round-trip Protobuf decoding.
2. Fixture-only mutual HMAC handshake and independent Python HMAC-SHA256 test vector.
3. Wrong-key, forged-host, session mismatch, second-Hello and unverified Windows peer refusal.
4. Linux actual socket peer credentials, 0700/0600 permissions and unsafe path/symlink/endpoint negatives.
5. Windows local, first-instance named-pipe construction and framed message exchange; protected handshake intentionally denied pending native OS attestation.
6. Original Tokio dependency upgraded only by enabling net, io-util and time features. Added mio/socket2 to Cargo.lock from the existing same-version Lu desktop lockfile. No new independent library.

## Required follow-up gates

- R1 preflight: exact source tree lock, no workflow changes, no existing lu.v1 protocol changes. Neither rustc nor installed native CI has qualified this source.
- R2: installed Windows named-pipe DACL, impersonated access-token, SID, executable/package attestor and user-profile isolation; Linux process/package proof and endpoint custody; secret storage, nonce/epoch uniqueness, replay/revoke.
- R3: integrate host sessions and original saved-user-turn grants with NativeToolRegistry, ConversationAuthority, durable journal and protected IDE worker; cancel, restart, failure and uncertain effect outcomes.
- R4: physical Windows/Linux, framing and backpressure p50/p95, disconnect/reconnect, stale endpoint recovery, installer/update/rollback, real single loaded model.

**Blocking security caveat:** This R1 does not enable successful Windows app authentication. A dedicated reviewed OS-native Win32 peer attestor is necessary. Do not circumvent Lu Core's deny-unsafe policy or treat pipe access or a claimed package hash as an authorization grant.

## R2 native Windows source integration (unqualified)

- The Windows FFI module in lu-win-worker-job creates a named-pipe SECURITY_ATTRIBUTES descriptor at creation through Tokio's create_with_security_attributes_raw API. Protected DACL permits LocalSystem and the Lu process owner's SID only, not the Windows default Everyone/Anonymous read DACL.
- The module captures GetNamedPipeClientProcessId, GetNamedPipeClientSessionId, OpenProcessToken/TokenUser SID and QueryFullProcessImageNameW from the connected client. Native handles and allocated security descriptors are freed with scoped ownership.
- WindowsPinnedEnrollment compares observed PID, user SID, session, executable path, module ID, role, manifest digest and HMAC enrollment key. The actual signed package/file digest remains independently host-verified BEFORE enrollment; path observation alone is not a code signature.
- Original Windows tests now require OS-observed identity, positive synthetic same-process handshake and forged-PID denial. Installed native tests are pending.

## R2-R1 original native qualification and source repair

Original PR #244 first exact head 4cac21018f28301d17695bf71f575d57182b4679:
- Linux native test phase failed with rustc E0716 at workspace_ipc_transport_r1.rs:73: borrowed temporary peer + server identity across tokio::join; Windows failed same two places plus analogous Windows server identity temporary at line 253.
- Original lu.workspace.v0 protobuf suite passed 9/9 on BOTH actual runners before transport fixture compilation failed. No transport test executed on that SHA. Source gate passed, original REPO-01 failed its separate existing CI-policy #242 test.
- Prepared successor binds peer and server identity to locals before creating futures; adds an actual separate Windows PowerShell client connecting to the private named pipe. That fixture tests kernel-reported non-parent PID, received original Protobuf Hello, and refusal to turn a same-user connection into another process's enrollment.
- The separate Windows client fixture is an intentionally UNENROLLED process and is a negative identity test, not a signed IDE test. Successful synthetic same-process handshake still does not prove real IDE package signing, installed custody, or user-turn permission.
- No original negative result may be erased; exact new SHA requires fresh native acceptance on both platforms. Frozen Lu/IDE branches remain unchanged.

## R2-R1 exact source native result (immutable original attempt retained)

* Initial PR #244 SHA `4cac21018f28301d17695bf71f575d57182b4679`: native run 44876666456; Windows/Linux `workspace_ipc_v0` 9/9 passed per platform; `workspace_ipc_transport_r1` compile failed E0716 (temporary `linux_peer()` and `materials()` borrowed across Tokio join; Windows same plus analogous identity). Original REPO-01 run 44876666475 failed CI trigger policy.
* Repaired SHA `0186b08af3cd7682950ea726b0de5c3c695b5e8d`: native run 44877087334; **Linux** original Protobuf 9/9 and IPC 7/7 passed, including actual kernel UDS credentials and hostile frame/permission fixtures; **Windows** original Protobuf 9/9 and IPC 8/8 passed, including a separate PowerShell process on the real named pipe with kernel PID evidence and forged-enrollment refusal.
* On both platforms the independent third `lu-win-worker-job` crate check failed *before running tests*: its manually synthesized standalone `Cargo.lock` needed updating under `--locked`. This is a real reproducibility blocker, not a passed entire workflow. The specific successor removes Tokio's optional `bytes` dependency (not enabled by `net,macros`) from that standalone lock and places package records in deterministic order. **This successor's locked worker check has NOT been run.** Do not claim complete native qualification.
* Original REPO-01 run 44877087406 failed source-policy issue #242's nine remaining legacy push-trigger entries **and also** this task's own inappropriate automatic pull_request event on `lu-ipc01d-r2-native.yml`. This successor corrects **our** violation to `workflow_dispatch` plus `workflow_call` exclusively. The nine legacy triggers are managed separately by CI-POLICY-01 and remain unresolved in this lineage. Do not weaken existing REPO-01 tests.
* The manual/reusable-only workflow no longer starts new native CI on PR pushes. Do not assume final lock/trigger/documentation successor is native-qualified from earlier SHA: arrange an explicit one-SHA manual/reusable native acceptance through an approved workflow route after source-policy convergence. The PR remains draft and no merge to Lu or IDE is authorized.
* Windows named-pipe native tests prove owner-DACL, observed PID, OS user SID and session metadata under test conditions, **not actual signed Lu IDE DLC identity**. Token impersonation after completed client message, package signer+file hash custody, PID lifetime, adversarial unrelated-account tests, durable grants, session secret rotation and replay prevention are all still open.

## R2-R2 protected Windows message-token source candidate (native acceptance pending)

- The original first-pass R2-R2 source `4ea8783da26bfc37caab549499b63a9ef4b0ac68` passed original `lu.workspace.v0` 9/9 on Windows/Linux; UDS 7/7; Windows named pipes 8/8; worker-crate 14/14 Linux and 13/13 Windows. The standalone `--locked` failure is closed for that immutable source. Its REPO-01 CI-policy audit remains separately failed on nine legacy triggers.
- This *successor* requires a fully read and validated first Protobuf Hello before moving Windows pipe evidence beyond raw PID/session/SID/path. The Win32 FFI synchronously calls `ImpersonateNamedPipeClient`, opens and queries the request token at identification/impersonation level, and must return to the host's primary token even on error. Failure of `RevertToSelf` is process-fatal rather than silently retaining foreign credentials. No asynchronous suspension occurs during token impersonation.
- The request token's SID must match the connected client's process primary SID. Win32 process PID/session, image and `GetProcessTimes` creation FILETIME are observed before/after impersonation to refuse changed process generations. The host's pinned enrollment must include exact process creation identity. Raw `WindowsObserved` cannot enter any Protobuf challenge or grant; only a message-token-observed witness is admissible to the existing explicit HMAC policy.
- Tests now require the original same-process Windows roundtrip through the post-read message-token path; the original independent PowerShell process uses explicit identification-level token and remains an unregistered **negative enrollment** case; raw observations and forged PID must be denied. Original Linux and general Protobuf tests remain mandatory.
- **NOT YET ACCEPTED:** this successor has not been compiled on a native Windows runner and has not passed installed tests. Effective impersonation token observation is not Authenticode signer verification, signed installed package ownership, retained process-handle lifetime, user-turn grant or durable cancellation/replay protection. There is no automatic tool/model activation. Never call a synthetic test signer proof or live Lu IDE package attestation.

## WIN-SIGN-01 — Authenticode and exact installed image enrollment candidate

**Status: SOURCE CANDIDATE; NOT INSTALLED Lu IDE acceptance.** Base native
Win32/Ubuntu transport and message-token qualification at \`e6ea8eec\`
passed (Protobuf 9/9 per OS; Windows IPC 8/8; Linux IPC 7/7;
Windows worker 13/13; Linux worker 14/14). This successor has new compiler
and genuine OS tests and needs independent same-SHA native results.

* A new default-off, narrow \`lu-win-worker-job::ipc_signed_package\` FFI
  module calls \`WinVerifyTrust\` with \`WINTRUST_ACTION_GENERIC_VERIFY_V2\`,
  a held read-only image HANDLE and \`WTD_CHOICE_FILE\`. It refuses unsigned,
  untrusted, malformed, or catalog-only files. It copies the leaf code-signing
  certificate's encoded DER from the verified WinTrust provider chain before
  closing the provider state. It forbids network retrieval and has no shell,
  PowerShell or model invocation in production.
* Lu Core SHA-256 hashes that same pinned file HANDLE in bounded chunks and
  SHA-256 hashes the signer certificate DER. Both must independently match
  exact, nonzero trusted host/installer pins. Revocation is not freshly fetched
  in offline mode; exact image+signer pins are mandatory. The Protobuf
  \`claimed_package_sha256\` is only a claimed manifest ID, **not** the
  executable hash or signer proof.
* The host pins the previously OS-observed and supervised PID, creation
  FILETIME, session, owner SID and executable path. The new signed-only
  enrollment creates no default approval and refuses all raw metadata
  evidence before sending the first challenge byte. The formerly synthetic
  \`WindowsPinnedEnrollment\` refuses enrollment in non-debug release builds.
  A successful HMAC still authorizes NO model, project, tool or effect.
* The signed session retains the actual Win32 process HANDLE and opened image
  FILE handle; post-handshake frames recheck the process lifetime,
  post-message impersonation SID and exact observed process generation.
  Handle or signing failures are terminal; no unsigned fallback.
* Native Windows tests use a **Microsoft-signed PowerShell 7 pwsh.exe surrogate** to
  test a separate real process, independent host pre-enrollment, WinTrust,
  certificate/file fingerprint match, bounded Hello and HMAC proof over the
  original Protobuf challenge. This is a harmless controlled test fixture,
  **NOT** a signed or installed Lu IDE. The fixed fixture-only test HMAC key
  is never a product credential. Additional tests refuse unsigned files,
  unsigned same-owner processes and mismatched signer/image/PID pins.
* **Still blocked:** actual signed installed Lu IDE binary and production
  publisher certificate from the approved installer; protected installer
  record/key storage, ACL on installed image and canonical file identity
  across a malicious path replacement, independent vendor signer provenance
  and revocation policy; physically installed hostile-user tests,
  one-use operator permission, durable cancellation and replay-denial.
  No mainline merge or DLC activation is authorized by a signed OS surrogate.

## WIN-SIGN-01-R1 — exact-SHA native reconciliation and strictly scoped repair

Original run [44879933412](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44879933412) on `edbb824b61d5f78a322ed05f5575cbfd828b41ad` finished: independent tracked source 1,280 PASS; Ubuntu 24.04 Protobuf 9/9, UDS 7/7, worker 14/14 PASS; Windows original Protobuf 9/9 PASS, then transport test compilation failed genuine Rust **E0716** at `workspace_ipc_transport_r1.rs:509`, because `&hello()` was borrowed across `tokio::join!`. The Windows transport and worker steps did not run. This is NOT a native signing PASS and NOT a failed signed positive test; the compiler stopped before that execution.

R1 consolidated repair keeps the original signed-payload policy and native negatives, binds the Hello fixture to a scoped local value before awaiting and changes WinTrust's three `WTHelper...` signer-chain APIs to **system32-only dynamic export lookup** through `LoadLibraryExW(..., LOAD_LIBRARY_SEARCH_SYSTEM32)` and `GetProcAddress`. Microsoft's official Win32 documentation explicitly says these helpers have no associated import library; direct `#[link(name="wintrust")]` helper imports are not a reliable linker contract. `WinVerifyTrust` itself remains the documented imported API. Missing exports fail closed, the library handle is retained through `WTD_STATEACTION_CLOSE`, and no network retrieval, shell signer check or unsigned fallback is introduced.

The exact-successor source and locked original native tests must still pass Windows/Linux. The overall REPO-01 nine-legacy-trigger policy failure remains separately owned by CI-POLICY-01. There is no signed Lu IDE release artifact available, no pinned publisher/installer custody evidence, and no actual installed DLC positive acceptance; a signed `pwsh.exe` test surrogate can never substitute for these.

## WIN-SIGN-01-R3 — protected installer-owned DLC enrollment candidate

**SOURCE CANDIDATE — not real installed Lu IDE acceptance.** The R2 native exact-SHA record passed Windows Protobuf 9/9, IPC 10/10, worker 15/15, and Linux Protobuf 9/9, IPC 7/7, worker 14/14; the Windows Authenticode positive was a signed Microsoft PowerShell 7 surrogate, not Lu IDE.

R3 installer-staging script: `scripts/lu-ipc-r3-stage-installed-ide.ps1` defaults to read-only preflight, requires an existing genuinely signed Lu IDE executable under Program Files, independent approved SHA-256 pins for its entire file and signer certificate DER, an approved package digest, absence of reparse-point or user-writable ancestry, and valid embedded Authenticode. It never self-signs, derives approval pins from the installed executable, grants effects, or writes a credential. Applying requires elevated installer context, and fails rather than overwriting an existing record.

Protected record: fixed `ProgramData\Lu\IPC\enrollments\lu.ide.json`. Every Lu-owned folder/file must have protected SYSTEM/Administrators-only DACL and owner. The installer writes exactly schema, module_id=lu.ide, role=Workspace, installed_executable_path and file/signer/package SHA-256 pins. The HMAC key is NEVER stored here. No silent upgrade, rollback or user-writable location.

Native FFI `ipc_installer_enrollment.rs` opens and checks fixed protected ProgramData path and all Lu-owned ancestors, rejecting reparse points, wrong owner, inherited or non-SYSTEM/BA permissions, and files over 4 KiB. Lu Core strictly rejects malformed/extra JSON fields, noncanonical/zero hashes and unknown module or role. It captures the separately host-supervised process PID, generation, SID, session and path, then obtains a nonzero session key *separately* from the trusted host credential broker. Only `windows::accept_installer_owned_lu_ide` can use this production path; synthetic caller-built pins are debug-only. It preserves existing WinTrust, signer + executable SHA-256, impersonation-token, HMAC and retained process/image lease gates. This is not user-turn authorization.

Windows native qualification additionally runs an adversarial unsigned/unapproved executable preflight and confirms no record is installed. Independent genuine installed positive acceptance still requires an approved signed Lu IDE executable, real protected installer-owned service and credential broker, exact verified publisher, privileged deployment receipts, physical Windows host, cross-account denial, tamper, revocation, cancellation and restart. The current NSIS dev preview installs an unsigned `lu-desktop.exe` under LocalAppData and cannot provide this. No signed IDE release artifact exists in this repo, and no signing key is in scope. Native R3 tests may pass while INSTALLED ACCEPTANCE MUST STAY HOLD.

### R3-R1 — native failure reconciliation and prepared correction

Original unchanged source run #44882690054 at `ec11356fc85ff298fb95ca4fba8310a69a24d2f5`: source lock 1,282 PASS; Ubuntu Protobuf 9/9, IPC 7/7, worker 14/14 PASS; Windows Protobuf 9/9, IPC 10/10, worker/Win32 16/16 PASS. **Windows-only installed enrollment negative PowerShell step FAILED prior to execution**, due a genuine ParserError at `lu-ipc-r3-stage-installed-ide.ps1:128`: line break in Get-ItemProperty argument expression. This is a script syntax fault, not a signer, ACL, or Windows Rust native failure. The overall REPO-01 CI-policy audit continues failing due to separately owned legacy automatic triggers.

R3-R1 successor repairs only that multiline PowerShell expression, preserving the existing fail-closed script and native signer policy. The Windows native workflow also adds an explicit core library test for malformed/extra protected records so a compiler-only library check cannot be mistaken for passing strict parsing. No actual signed Lu IDE package, installer custody, OS secret broker, or positive installed acceptance has been provisioned; keep installed release HOLD and PR DRAFT.

## WIN-SIGN-01-R4 — LocalSystem credential custody and approved signed installer gate

R4 source implementation adds native Windows credential lookup, actual protected enrollment integration and preflight for a separately approved signed installer. It is not signed Lu IDE installed positive acceptance.

Native broker: `src-tauri/lu-win-worker-job/src/ipc_credential_broker.rs` reads only the exact `LuCore/IPC/IDE/v1/<64 lowercase package SHA-256>` Windows Credential Manager CRED_TYPE_GENERIC record. The host must have the OS-verified LocalSystem SID S-1-5-18. It requires a local-machine persistence marker and an exact nonzero 32-byte secret; no user-profile/roaming/fallback lookup, default secret, CredWrite, key in a JSON record, model-origin or caller-supplied production key. Provisioning and protected IDE-side distribution are still separate, unimplemented privileged-service tasks.

Lu Core release admission now takes only a host-supervised PID and server identity. It reads the protected installer-owned record, obtains the exact package-bound credential from the native SYSTEM broker and then performs the existing WinTrust signature checks, SHA-256 pin matching, process/SID/session/token authentication and HMAC Protobuf handshake. It retains a digest of the protected record and credential; each received post-handshake frame must survive independent process lifetime, record-integrity and OS credential lookup checks. Revocation or key rotation refuses further frames, but is not certificate online-revocation freshness or a durable user-turn effect grant.

Signed-installer preflight `scripts/lu-ipc-r4-verify-signed-installer.ps1`: an independently approved exact installer SHA-256 and embedded Authenticode signer certificate SHA-256 must match a real signed Windows installer, as must the separately approved installed Lu IDE executable, signer and package digest checked by the existing R3 script. It never builds, signs, self-approves, executes, trusts or downloads a binary. Default verify-only; elevated -Apply delegates to the already protected, no-overwrite R3 record writer. CI creates an unsigned binary with a *correct* exact file SHA-256 and verifies its signer check refuses it without enrollment.

Unwaivable installed acceptance still needs: genuine independently signed Lu IDE installer and executable under protected Program Files; authoritative approved publisher/manifest file pins; a trusted SYSTEM broker service with credential creation/distribution, bounded one-use operator approval and authenticated IDE-side HMAC custody; physical installed positive and tamper, wrong signer/account, revocation, replay, crash, restart and rollback negatives; connection of signed IPC frames to ConversationAuthority and EffectJournal. The existing unsigned Lu desktop NSIS preview, test surrogate and unprovisioned runner cannot satisfy this.

Keep PR #244 draft/unmerged and CI-POLICY-01's legacy-trigger failure separate from native R4 qualification.

### WIN-SIGN-01-R4-R1 — exact-source security hardening and physical admission hold

Two source-level issues in the first R4 candidate require correction before any installed acceptance:

1. Credential Manager's effective logon identity must not depend on a transient named-pipe client impersonation. The native credential reader now checks `OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, ...)` immediately before examining the SYSTEM process and calling CredReadW. Only ERROR_NO_TOKEN (no impersonation) permits the SYSTEM credential lookup. Any available impersonation token or unknown query error is fail-closed. The negative regression uses `ImpersonateSelf(SecurityImpersonation)` and `RevertToSelf`, independently exercising a real native thread token. See Microsoft OpenThreadToken and ImpersonateSelf Win32 contracts.
2. The signed installer verifier requires `-PackageManifestPath`, a canonical real protected file inside the approved Program Files installation root. It checks *every* path component against junction/reparse points, non-installer ownership and user-writeable ACEs and checks exact bytes SHA-256 against the externally approved manifest digest. A signed trusted installed `pwsh.exe` is permitted in **negative test only** to exercise the preflight before a user-writable manifest is refused; it is never installed as Lu IDE. A second test uses an unsigned executable with a correct file hash to prove that a digest alone is insufficient. The verifier defaults to non-mutating preflight and does not sign, enroll or modify Windows trust.
3. Lu Core uses `#![deny(unsafe_code)]`. R4's temporary key-copy volatile clearing has been moved into the isolated, reviewed Windows FFI module, rather than weakening Lu Core's unsafe-code gate. Native clear uses volatile stores and a compiler fence as a best-effort local mitigation, with no claim of erasing all OS/compiler copies.

R4-R1 is limited to the existing manual/reusable dedicated IPC native task group, with one exact source lock and one nonforced successor after the original CI batch is terminal. No automatic CI trigger proliferation, no downgrade of signer requirements, no unsigned enrollment or credential fallback.

**Production acceptance remains HOLD** until a separately approved genuine signed Lu IDE executable and installer, publisher and manifest pins, protected Windows service and credential distribution, authentic installed positive Core↔IDE proof, revocation/restart/rollback negatives and ConversationAuthority / EffectJournal per-turn host integration are provided and independently tested. No signed release artifact or private publisher signing key is assumed.

## LU-IPC-01D-R4-R2 — Lu Core-only authenticated session and durable turn integration

**Scope and ownership:** This task implements Lu Core's server/host integration ONLY. The separate IDE conversation owns the IDE executable, installer, signature/release packaging and IDE-side Protobuf IPC client. This Core task does not build or integrate an IDE. Its production Windows entry requires a genuine installed signed DLC witness from the existing protected Core-side native transport; synthetic signed Microsoft PowerShell test fixtures do not carry an installer revocation witness and cannot enter Core effect admission.

New `workspace_ipc_core_host.rs` is an intentionally closed Core-side boundary. `CoreDlcHostGate::from_installed_windows(&SignedWindowsSession)` refuses signed-but-not-installer-enrolled debug surrogate sessions and checks native process/record/credential custody. The only production receive method calls `SignedWindowsSession::read_verified_frame` (current message-token OS identity, signed image and credential/revocation witness, host-minted session). No public method accepts a raw JSON/Protobuf frame as authenticated evidence. Linux has only private source/test scaffolding until independent OS/package attestation is accepted; UDS transport or UID alone grants no effects.

Per-turn authorization: the trusted Lu host supplies immutable `BoundCallOrigin`, execution scope, policy, and a generated-Protobuf semantic input validator. Incoming `OriginRefs` are compared EXACTLY to the host-owned original USER task, not used to create a task, grant or scope. Declared `CapabilityInvoke` IDs must resolve to Lu Core's native registered descriptor at v0 version 1; type URL and actual encoded input require exact host semantic verification; unresolved object locators and unknown versions refuse. The host rejects expired or excessively long deadlines, stale/foreign session refs and handshake frames. `ConversationAuthority.authorize_worker_call` and `dispatch_bound_durable` BOTH recheck live user-turn task, current tool effects, workspace scope and independently enforced host policy.

Durable call identity is SHA-256 of domain-separated, length-framed host-owned original conversation, turn, task, scope, authenticated module and remote operation ref. It is intentionally NOT keyed to a transient IPC session/epoch so an interrupted effect cannot become a new call simply by reconnecting. The existing full-transaction SQLite `EffectJournal` and `DurableDispatcher` own PREPARED -> EXECUTING -> APPLIED or UNCERTAIN transitions and receipt replay. On cancellation/drop of a future after it alone obtained EXECUTING, the new `ExecutingEffectGuard` conservatively marks UNCERTAIN; a duplicate or retry cannot run the same effect again. On host process restart, exclusive-owner custody must first prove prior execution stopped before `EffectJournal::reconcile_interrupted()` is called. A claimed CancelAcknowledged is never OS worker termination proof.

Bounded admission: a host-owned session gate refuses a fifth simultaneous invoke (max four), closes new admissions on the original task revocation, and refuses further submissions after explicit session close. Closing a gate does NOT itself kill/reap an in-flight OS worker; the native supervisor remains responsible for cancellation and kernel custody. A session is pinned to one original Lu Core user task for this bounded initial lane, with no automatic cross-task elevation. Raw DLC output is never an independent receipt and cannot grant an effect.

Native tests in the existing manual/reusable task-group cover per-turn origin and exact scope, semantic typed-input refusal, untrusted object locator refusal, host policy denial, durable idempotent replay across actual SQLite reopen, revoke-closes-session, at-most-four in-flight slots, cancellation-to-UNCERTAIN, restart denial, and operation-ref stability across authentication epochs. Windows also retains its original installed signer, protected Credential Manager, native peer and script denial checks. No automatic CI job fanout or per-task run stacking.

**Release gate still HOLD:** Source-level Core-host qualification is not the same as an actual deployed Core host service with OS-backed exclusive process custody, installed signed DLC, protected credential provisioning, and end-to-end physical installed Core↔DLC positive. Those remain separate release/physical acceptance. The IDE conversation supplies IDE-specific artifacts. No pretend IDE installation or Core-side IDE implementation is claimed.

### R4-R2-R1 — reconcile first Core host native batch and Windows test fixture cleanup

Exact original R4-R2 run #44914609639 at `b17e03516502502691151517f327a41394b2c371` is terminal. Frozen source integrity **PASS (1,285 tracked)**, Ubuntu native job **PASS** (original Protobuf 9/9, UDS IPC 7/7, Core host module six new tests 6/6 including cancellation-to-UNCERTAIN/reopen, native worker 14/14). Windows original Protobuf and named-pipe transport suites **PASS**, and new Core host unit compilation **PASS**. However Windows new Core host step **FAIL** 5/6 due test fixture teardown `fs::remove_dir_all` at `workspace_ipc_core_host.rs:428`: Win32 sharing violation 32 immediately after closing the reopened SQLite WAL database. This is *not* a failure of the committed durable replay assertion; the failure occurred after it. Following steps did not execute and cannot be counted as original Windows PASS.

R4-R2-R1 successor preserves the original true SQLite journal reopen and committed receipt/idempotency assertions, changes only temporary test-directory cleanup to a bounded best-effort retry (Windows WAL/AV sharing races are not authorization or journal-fidelity evidence), and retains the same six regression tests. All other Core/protobuf/native/credential/installer source remains byte-identical. Exactly one nonforced update after the original CI batch is terminal and one succeeding native batch; no stacked CI or automatic workflow trigger edits.

Release remains HOLD for a physically installed, signed DLC/Core system, privileged credential provisioner and IDE-side work owned by a separate conversation. Core-side verified native fixture acceptance alone is not live installed integration.


## R4-R3 — Host-owned DLC cancellation/custody and protected key provisioning (source candidate)

- Lu Core's installed Windows gate now requires a concrete original Job-owned worker and checks its kernel-observed PID against the signed named-pipe lease. A suspended child is assigned to Lu's private kill-on-close Job before resume; failed assignment kills/reaps. This is not an IDE installer implementation.
- Revocation closes the gate, requests Job termination and verifies descendant-zero plus child reaping. Active durable dispatch futures observe closure and are dropped; an EXECUTING effect becomes UNCERTAIN rather than being silently replayed. Gates remain closed after revocation.
- The native SYSTEM-only broker can create a new 32-byte random package-key credential, rejects an already installed credential (no silent rotation), and revokes the exact package target. Interactive users, impersonating threads and caller-supplied secret bytes cannot provision through this API. The privileged installer must serialize provisioning and independently approve the exact signed package before invoking it.
- Tests added for in-flight revocation, uncertain journal preservation after reopen, and irrevocable gate cancellation. **These are source tests, not executed receipts.**
- Still BLOCKED: genuine signed installer/publisher provisioning and protected IDE-side secret enrollment; real installed Windows DLL/package identity and Job launch/credential service end-to-end tests; Linux delegated-cgroup child custody and package admission; exclusive prior-owner termination and single-writer startup recovery before broad `EffectJournal::reconcile_interrupted`; CI exact-SHA and physical Windows acceptance. No positive production admission or restart recovery may be inferred without those independent witnesses.

## LU-IPC-01D-R4-R3-R1 — Single-owner IPC journal source preflight (not native-qualified)

- `ExclusiveIpcEffectJournal::acquire` requests SQLite EXCLUSIVE+WAL ownership before any effect frame. A second Lu Core process must fail boundedly; the OS releases the lock when its process exits. The Core Windows production receive path now requires this exclusive journal wrapper instead of an arbitrary raw `EffectJournal`.
- Startup INSPECTS actual durable EXECUTING and UNCERTAIN rows and refuses all new DLC effects when either is present. It does NOT misclassify them as FAILED, pretend an installed package has been authenticated, or automatically replay them.
- Once the external OS/SCM/delegated-cgroup supervisor has independently proven previous process-tree shutdown, a separate reviewed reconciliation operation must classify remaining EXECUTING as UNCERTAIN and record operator custody. This OS-to-journal proof is NOT implemented by an exclusive database lock. Accordingly cold restart after pending effects remains fail-closed, not production-recovered.
- Added source tests for two competing SQLite owners, released-owner reacquisition, blocked EXECUTING restart, preserved UNCERTAIN state, zero-package credential refusal and non-SYSTEM write/delete refusal. These tests still require actual compiled Windows/Linux runs. Credential rotation/re-enrollment remains installer-broker work; no key is delivered through IPC.
- Core-owned cross-DLC dependencies received: [#245](https://github.com/vjmichael/Lu-AI-GUI/issues/245) requests an authenticated health-only Core endpoint/HealthReport for IDE; [#246](https://github.com/vjmichael/Lu-AI-GUI/issues/246) requests scoped NVP ObjectRef resolution plus explicit multi-effect execution/write mapping. Neither a raw NVP ObjectRef nor a repeated effect declaration is permission. Track #245 under Core IPC service/health; #246 under Core resolver/effect-policy. Do not edit or duplicate the IDE/NVP clients. Both remain OPEN and NOT accepted by R4-R3-R1.

## R4-R3-R1 exact-session retirement source follow-up

- Added `SignedWindowsSession::installed_package_sha256()` from the protected native revocation witness (not the Hello claim). Lu Core `CoreDlcHostGate::revoke_installed_package` checks exact session binding, closes admission, requests SYSTEM Credential Manager deletion and *independently* attempts original Job termination/kernel-empty/reaping. It reports failure if either action is unproven; revoking a debug/uninstalled session is refused. It cannot be used as an arbitrary caller-selected key deletion API.
- This is a SOURCE-ONLY API, not proof that a dedicated SYSTEM broker service, a genuinely signed installer, protected IDE-side key enrollment, or a post-reboot OS worker-custody reconciliation path is installed.

## R4-R3-R2 exact-source qualification candidate

- Source review confirmed that the SYSTEM credential provisioner reopened a key and verified only its shape, not that it was the same freshly generated 32-byte key. It now compares actual OS-reopened bytes against the freshly created key before reporting success, with no early-exit comparison and volatile erasure of both temporary copies. A mismatch refuses success and requires operator/supervisor reconciliation (it does not blindly delete a credential possibly written by a different SYSTEM owner). The installer still requires an external protected provisioning serialization lock; this code is not a replacement for installed enrollment.
- The preexisting manually dispatchable/reusable-only cross-platform workflow now executes `cargo check --locked --lib` for Lu Core and the native worker crate, exercises `workspace_ipc_core_host` and `ipc_exclusive_owner_tests` explicitly, and retains both original Protobuf transport suites and the full native FFI/credential tests. **No new automatic push/PR trigger was added.**
- This patch is a prepared SOURCE candidate until published and independently compiled on actual Linux/Windows runners. A REPO-01 source-lock pass is neither Rust compilation nor installed OS credential/custody acceptance.
- **Authoritative reboot recovery remains blocked** until a separately installed OS supervisor retains or independently verifies the original native worker custody. An exclusive SQLite lock only proves that no previous SQLite writer currently holds that DB, not that all OS descendants have exited. Do not clear startup EXECUTING/UNCERTAIN or replay a call on that basis.

- Restart reconciliation now exposes a bounded (maximum 1,024) **read-only exact-call worklist** from the exclusive journal: call ID, tool ID, canonical fingerprint and unresolved state only. It omits raw model/input/output bytes, credentials and receipt material. Malformed or oversized recovery sets fail closed; this is not an OS kernel-empty witness, not an approval and cannot itself clear `NeedsCustody`. This prepares the eventual installed supervisor service to join trustworthy original worker custody with the exact persisted interrupted calls.


## LU-IPC-01D-R4-R3-R3 — Exact-call original Job custody / physical Windows acceptance gap

**Source candidate, not installed release acceptance.** Lu Core now persists an exact signed-session
worker binding BEFORE entering durable execution: call ID, input fingerprint, admitted module,
session reference, Core epoch, host-observed original Job worker PID, and
OS-observed worker creation FILETIME. Incoming DLC frames,
saved plan state, and user-owned SQLite files cannot create the native kernel-empty proof.
The native DLC-client Job must independently observe ActiveProcesses=0 and reap its child.
Only then, after active IPC dispatches drain, can the gate mint an in-process immutable
client-kernel-stop witness for the exact original calls it admitted. This client
Job does NOT automatically contain every Core native tool executor.

The exclusive journal joins *every* startup EXECUTING/UNCERTAIN entry against the original
supervised call and the native observed witness in ONE SQLite transaction. Missing, duplicate,
extra, foreign-session, wrong epoch, wrong worker PID, wrong worker creation time,
wrong fingerprint or non-supervised entries fail the entire transaction. All
reviewed EXECUTING effects become UNCERTAIN, with a separate forensic custody
table; **no tool effect is marked APPLIED/FAILED or re-executed**.
Critically, this is ONLY DLC *client* termination custody. The journal retains
NeedsExecutorCustody and **continues to refuse all new execution admissions**
until a separate protected service proves the actual Core tool executor trees
also ended; a signed DLC client Job is not such a service.

**Critical limitation:** this witness is only constructible while the host still owns
the original DLC-client native Job handle. After unclean Core death, neither
SQLite EXCLUSIVE ownership nor a stored custody row nor a PID liveness probe
is valid evidence of remaining Core tool executor descendants having stopped.
Such recovery stays fail-closed. A separately installed SCM/SYSTEM supervisor
must retain and verify custody for the ORIGINAL DLC client and each Core
effect-execution worker across a Core process crash, then supply exact-session,
exact-call OS-attested receipts. The broker also needs approved enrollment,
credential rotation/revocation and a real installed security boundary.
No such privileged service or physical installed receipt is present here.

### Existing native batch / physical acceptance requirements

- Existing manually dispatchable/reusable grouped IPC workflow now explicitly executes the
  cross-platform ipc_supervisor_exact_reconciliation_tests matrix, including all-or-nothing
  exact-call, PID, process creation time, epoch, fingerprint and stale-receipt
  negative tests. Windows also runs the
  immutable signed-peer bind test and genuine Job/child kernel-empty test. Original source lock,
  Protobuf transport, trusted host/turn, installer refusal and credential tests stay intact.
- Native CI on hosted Windows does **not** satisfy physical installed Windows acceptance.
  Obtain a signed, approved installed Lu Core executable/SCM service and a separately installed
  IDE, independently pin executable/publisher/manifest and protected credential enrollment,
  then verify the original named-pipe OS peer, owner/service identity, worker creation epoch
  and actual signed session. Do not use a synthetic HMAC or signed PowerShell surrogate.
- In a physical Windows test, terminate Core with an effect executing while the installed
  **independent** supervisor retains original Job custody. Verify that it observes the full
  assigned descendant tree reach kernel-empty, emits exact previous-epoch worker receipts
  under SYSTEM control, and the next Core independently authenticates their provenance.
  Refuse missing/tampered/wrong-call/older-epoch/duplicate/stale receipts, key rotation,
  revocation, session replay and an interrupted power cycle. Check original effect remains
  UNCERTAIN and new-user-turn authorization is independently required before new work.
- Capture approved local operator and OS receipt references with exact installed binary,
  publisher, service PID/creation identity, backend/source SHA and kernel custody evidence.
  **No physical installed execution or SCM proof is claimed here.**
- Known independent release blocker: CI policy #242;
  follow-on Core interop requests: IDE health #245, NVP ObjectRef/effect policy #246.

## LU-IPC-01D-R4-R3-R3-R1 — SCM identity bootstrap, not installed executor recovery

- Native fixed-name `LuCoreIpcSupervisor` read-only service entry point is now buildable (Windows SCM dispatcher + LocalSystem nonimpersonating requirement, stop/shutdown notifications). It has **NO arbitrary process launch, runtime tool executor control plane, or credential issue/rotate endpoint**. It may be signed/installed only by a separately reviewed approved installer; never infer this occurred from source or hosted CI.
- Native `ipc_scm_supervisor` verifies the actual Service Control Manager's dedicated RUNNING service, original process PID/creation FILETIME, Session 0, primary LocalSystem SID and held signed image. `workspace_ipc_scm` independently checks exact installed binary SHA-256 and embedded signer certificate DER SHA-256 against externally authorized pins and rechecks service/process generation. A service named by a caller, a bare SYSTEM process, an image path, self-asserted signature or user-selected PID is not an admission.
- The production `CoreDlcHostGate::receive_authorized_windows` now explicitly requires the pinned live SCM witness as an input and checks its lifetime around message dispatch. It continues to deny non-READ effects and refuses any restart needing native executor custody. A RUNNING SCM service is **NOT** a proof that its workers or descendants were stopped.
- Added read-only physical-Windows `scripts/lu-ipc-r5-installed-scm-preflight.ps1` that checks installed service registration, LocalSystem owner identity, stable live process epoch, protected installer-owned file ancestry, exact signed image hash and signer approval. It never registers the service, changes permissions, provisions credentials, or emits an executor Job-empty receipt.
- The original grouped native workflow now compiles the SCM service binary, tests SCM status/refusal paths and verifies the unapproved/unsigned SCM preflight refusal on hosted Windows. **These tests are distinct from physical installed Windows SCM/credential/cancellation and restart acceptance.**
- Remaining: the installed SYSTEM service must ACTUALLY own protected client and Core native-executor Job handles across Core crashes, launch only independently signed/pinned workers, keep scoped call ownership, produce independently authenticated kernel-empty service-origin receipts, implement one-use credential enroll/rotation/revoke and prove physical Windows restart/power-loss refusal and recovery. No read-only SCM identity or SQLite journal record can stand in for that evidence.
