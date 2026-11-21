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
