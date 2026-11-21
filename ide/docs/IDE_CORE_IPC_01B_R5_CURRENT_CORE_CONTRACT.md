# IDE-CORE-IPC-01B-R5 — Current Core contract and installed-admission gate

Review date: 2026-11-21. **Scope: Lu IDE only. This is NOT a release approval.**

## Exact source custody

- IDE original draft PR #236: `f1f0ed4bc1e740d2a9f85615b988b779a46651db`.
- Lu Core draft PR #244 reviewed head: `35529d1672a6b77b6f877049ca501d82123fde7f`.
- Core `workspace.proto`: Git blob `747b7472ab7e4bdfd7d646dd629b2c45a9c63dd1`.
- Core `workspace_ipc_transport.rs`: Git blob `133198027db1834a0f0904bab2f1decb9d0f9456`.
- Core `ipc_credential_broker.rs`: Git blob `6c30f57e8af701f308ff61215e97d20cdaa5adac`.
- Core `ipc_installer_enrollment.rs`: Git blob `fa96021eb79c2090568d3a58850d5e6a3e6b2f88`.
- **Follow-up during qualification:** Core PR #244 moved to `1afb058e09dddb86e1d5ce1726ae8a167c920aca`. The same `workspace.proto`, `workspace_ipc_transport.rs`, `workspace_ipc_core_host.rs`, SYSTEM credential broker and installed-enrollment blobs remain byte-identical at that head; issue #245 remains OPEN. No new Core-owned per-operation issuer or public Health dispatcher source was observed.

## Wire compatibility verified by source review, not by installed testing

The IDE read-only projection matches `lu.workspace.v0` wire major/minor `1.0` for `WorkspaceFrame`, `Hello`, `AuthChallenge`, `AuthProof`, `Welcome`, and fields `25=HealthRequest`, `26=HealthReport`. Core and IDE use length-prefixed canonical Protobuf and domain-separated RFC2104 HMAC-SHA256. `lu.ide` has no negotiated capabilities; unexpected features are refused. `HealthReport` must have module `lu.core`, status code and nonzero host epoch. The private `\\.\pipe\LuCoreIpcScmControlV1` must never be treated as the workspace Health endpoint. This is a review of structural compatibility; no Core source is copied into IDE.

## Core producer missing — mandatory refusal

Core Windows currently admits the signed Lu IDE process only through an installer-owned protected record and unimpersonated SYSTEM-held **persistent**, package-bound credential. Core's `accept_installer_owned_lu_ide` does not implement the IDE-proposed `lu.ide.v1.NativeHealthLeaseRequest/Reply` per-operation producer, nor consume the IDE journal operation ID before issuing an ephemeral enrollment. No installed Core-owned public `HealthRequest -> HealthReport` dispatch loop, protected independent Linux signer/issuer service, or independently signed Windows installed issuer has been accepted under issue #245.

**Do not export, reuse or relabel the Core SYSTEM persistent credential as a one-use IDE credential.** A client-only journal burn does not prevent a second Core session and is not server-enforced replay denial. Do not install a test responder as the production Core endpoint or connect to the private SCM control pipe.

## R5 IDE-side defensive correction

A properly authenticated `HealthReport(connected=false)` remains a diagnostic status and must never produce a *positive observed* result in the IDE native menu or durable journal. The IDE now records the terminal state **refused** instead of **observed** for this response. The authenticated report can still be inspected internally, but grants no authority and is not a successful connected observation. The new focused unit test covers `connected=false` and `connected=true` classification.

## Required independent Core handoff before R5 can be completed

1. Accepted Core source SHA and an independent protected issuer producing a *one-operation* Health credential from an installed signed package, with OS process identity/publisher pins, issuer-side fsynced operation burn, expiry, restart replay refusal, cancellation and revocation. **Do not assume the IDE-proposed Proto is an accepted Core API.**
2. Authenticated, actually running Core-owned public Linux UDS and Windows named-pipe Health-only endpoints returning real Core epoch/status from the live Core host. Exact endpoint discovery and source/package hashes must be independently provided.
3. Real separate installed IDE and Core processes; positive and negative Linux and Windows peer/signer, stale credentials, reused operation ID, forged Welcome, malformed frame, socket substitution, lost reply, restart and installed power-loss evidence.
4. Separate source/OS acceptance receipts. Unsigned disposable Linux Tauri, mock issuer, generated HMAC replies, and Windows syntactic pipe checks do **not** satisfy installed acceptance.

## Status

- IDE-only structural compatibility and default-deny regression: **PASS** (see local receipt).
- Core one-use producer: **BLOCKED**, missing/uncertified.
- Installed Core Health handler: **BLOCKED**, missing/uncertified.
- Windows signed IPC acceptance: **NOT RUN**.
- Independently installed Linux IPC acceptance: **NOT RUN**.

No Core source edit, GitHub CI dispatch, forced publication or IDE source push is part of this task.
