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
