# IDE-CORE-IPC-01B-R4 — Upstream Lu Core IPC and issuer audit

Reviewed 2026-11-21; **current reviewed Core source** draft PR #244 at `bfbffc35320c78e74d52b3918cd587356fffb3a3` (not IDE PR #236). Scope: **IDE only**; no Core changes, no CI dispatch, no release claim.

## Exact upstream custody

| Lu Core file | Git blob SHA-1 | Observed contract |
|---|---|---|
| `src-tauri/lu-core/proto/lu/workspace/v0/workspace.proto` | `747b7472ab7e4bdfd7d646dd629b2c45a9c63dd1` | `lu.workspace.v0`, major 1/minor 0, HealthRequest tag 25 / HealthReport tag 26, AuthChallenge 36 / AuthProof 37 |
| `src-tauri/lu-core/src/workspace_ipc_transport.rs` | `133198027db1834a0f0904bab2f1decb9d0f9456` | canonical HMAC domain, installer-owned Windows enrollment, `\\.\pipe\lu-ipc-` + exactly 32 lowercase hex digits |
| `src-tauri/lu-core/src/workspace_ipc_core_host.rs` | `912f6fdae583e54ce61beba5f23e637235d71b7e` | protected tool invocation and GPU lease dispatch, **not** installed HealthRequest dispatcher |
| `src-tauri/lu-win-worker-job/src/ipc_credential_broker.rs` | `6c30f57e8af701f308ff61215e97d20cdaa5adac` | LocalSystem-only installed, persistent package-bound 32-byte key; no public IDE lease service |
| `src-tauri/lu-win-worker-job/src/ipc_installer_enrollment.rs` | `fa96021eb79c2090568d3a58850d5e6a3e6b2f88` | fixed protected ProgramData `lu.ide.json` pins; **no key** in public record |
| `src-tauri/lu-core/src/workspace_ipc_scm.rs` | `1cc8e219363a055711df91efe97e288bb27ce495` | private SCM worker/control, **not** public Health IPC |

These identities were obtained by reviewing exact current Core GitHub tree/file contents through the user's connected GitHub repository. Do not infer they are approved production source or installed binaries.

## Proven wire agreement

* IDE's `readonly.proto` is a **read-only projection**, not the authoritative full Core schema. Both projects agree on `WorkspaceFrame` tags `1/2/3/4/5`, `Hello=10`, `Welcome=11`, `HealthRequest=25`, `HealthReport=26`, `ProtocolFailure=30`, `AuthChallenge=36`, `AuthProof=37`, `ModuleRole.WORKSPACE=1`, and `HealthReport(module_id=1,connected=2,status_code=3,epoch=4)`.
* Both use length-prefixed big-endian framing with 256-KiB maximum and `HMAC-SHA256` transcript `lu.workspace.client.v0\0 || canonical Hello frame || server nonce`, followed by `lu.workspace.server.v0\0 || Hello frame || server nonce || session_ref || unsigned Welcome`. The HMAC is **not** an OS peer/signer witness.
* Core's server negotiates `enabled_features=[]` for `lu.ide` (its only conditional feature today is `gpu-lease-v1` for `nvp.film`). R4 IDE rejects unexpected signed feature claims even after MAC verification; still only `HealthRequest` may be issued.
* **Windows public pipe**: `\\.\pipe\lu-ipc-` followed by 32 lowercase hex digits; the **private SCM pipe** `\\.\pipe\LuCoreIpcScmControlV1` is never available to IDE. A syntactically valid endpoint remains untrusted until OS/signer and Core HMAC checks have succeeded.

## Hard interoperability blocker — one-use lease is not a Core grant

The IDE's `lu.ide.v1.NativeHealthLeaseRequest/Reply` was created **on the IDE side**, as a proposed issuer-facing protocol. **Core does not currently implement, serve or recognize this issuer Protobuf.** More importantly, the current Windows Core server uses a durable, SYSTEM Credential Manager 32-byte package key for HMAC. Releasing this **persistent** key to an unprivileged IDE in a 60-second IDE lease does **not** make the Core-side key single-use. The Core handshake has no one-use `operation_ref`/issuer-burn binding, so an IDE-only journal cannot enforce one-use *at the server*. Do not implement a bridge by copying/exporting the SYSTEM key or passing the private SCM control pipe.

Core owner must choose and implement an **accepted Core-side** per-operation enrollment and consumption mechanism, with independent signed package/process provenance, single-use grant enforcement **at Core**, revocation/restart and actual Health dispatcher. This may use a Core-recognized derived/ephemeral credential, or a Core-owned one-use native broker transaction, but must be reviewed and accepted on the Core track. Until then, IDE's installed issuer positive path remains **BLOCKED**; deny rather than fabricate an endpoint or use a mock key as production evidence.

## Windows & Linux admission obligations

* Linux: actual installed Core gateway and independently attested issuer/service with verified Linux `SO_PEERCRED`, fixed protected endpoint, durable burn and receipt. A synthetic test issuer is **not** Lu Core.
* Windows: genuine signed IDE image and installer pins, OS-observed server process generation, process image and certificate, protected native enrollment from an accepted SYSTEM broker, original `\\.\pipe\lu-ipc-...` public gateway (not SCM), and independently observed HealthReport. No known accepted Windows issuer interface currently serves the IDE's proposed lease message.
* Never mark real Core interoperability, credential issuance or installed signed release PASS until Core issue #245 provides accepted corresponding source and evidence.

## Follow-up

Track Core/OS-owned missing endpoint and issuer on [issue #245](https://github.com/vjmichael/Lu-AI-GUI/issues/245). IDE PR #236 remains IDE-only; do not patch Core code here. Use one reviewed non-forced IDE source publication after existing CI is terminal, not a new CI batch per repair.

### Additive GPU schema change at current head

This source advance adds `WorkspaceFrame.GpuLeasePoll=41` and `GpuLeaseTransitionKind` for NVP. It does **not** change the IDE read-only `WorkspaceFrame.HealthRequest=25`, `HealthReport=26`, `AuthChallenge=36` or `AuthProof=37`. The IDE projection intentionally excludes all GPU messages; a Health client must not negotiate GPU capability. The reviewed Core `workspace_ipc_transport.rs` blob remains unchanged.
