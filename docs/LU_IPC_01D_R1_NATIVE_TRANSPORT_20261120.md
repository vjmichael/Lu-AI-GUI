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
