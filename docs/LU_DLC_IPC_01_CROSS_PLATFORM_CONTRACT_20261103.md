# DLC-IPC-01 — common Lu Workspace integration contract (Windows / Linux)

**Date:** 2026-11-03. **Development branch:** `lu/dlc-interop-01-cross-platform-contract-20261103`, forked from the active WRITE-01E development revision `2a9a1355`. **Release:** source-only, unshipped, not merged. The accepted `0.1.0-alpha.2` installer is unchanged.

## Problem and two integration modes

NVP's independent external-host alpha probe did **not** establish a usable Lu inference service. Lu's accepted alpha only exposes Tauri-internal `lu_status`, `lu_load_model`, `lu_generate` and related commands. Its internal llama.cpp loopback API key and port are *private Lu process state*. The experimental jan_alt AF_UNIX Protobuf `ToolCall/ToolReply` transport is a separate native-tool adapter, not a shipped inference server, and Windows peer/ACL verification was explicitly incomplete. See [NVP PR #33](https://github.com/vjmichael/nvp_offline_film/pull/33) and [Lu PR #6](https://github.com/vjmichael/Lu-AI-GUI/pull/6).

* **Lu-hosted DLC Workspace:** IDE, Film/NVP, Research and Writing are registered as trusted `ModuleKind::Workspace` modules. `WorkspaceHost` owns instance lifecycle and checks active instance/module ownership; normal `ConversationAuthority`, `PolicyEngine`, grants and effect journal still govern operations. A workspace never gets authority from its manifest, a discovered tool, or its UI.
* **External process / independent application:** NVP or a third-party application can optionally *request* an authenticated, Lu-host-approved provider session via a dedicated local transport. Lu owns the GGUF/MMProj, model worker, resource guardian, user identity, data scope, generation ordering and cancellation. A peer's OS identity alone does not give it a user grant.

These are two adapters to **one semantic contract**, not two proprietary versions of inference. No Docker, MCP server, guessed TCP port or shared llama API credential is required.

## Transport is NOT authority

| Layer | Linux | Windows | Invariant |
| --- | --- | --- | --- |
| Local carrier (future) | `AF_UNIX` stream, private per-profile runtime directory, peer credentials | Named pipe with per-profile security descriptor, OS client identity verification | A path, name, PID or socket access is not an authorization grant |
| Framing | u32 big-endian length + bounded Protobuf `DlcFrame` | Identical u32 big-endian length + bounded Protobuf `DlcFrame` | One canonical message per packet; reject truncation, trailing bytes, version drift, no JSON/unknown-field fallback |
| Instance enrollment | Trusted Lu host approves a registered module/active instance | Same | Separate explicit user/project/conversation scope and revocable lease |
| Inference | Lu-owned `InferenceHost` / scheduler | Same | No remote load/unload, private engine port/key never exported |
| Worker lifetime | Existing Linux supervisor/cgroup where claimed | Correct Windows Job + suspended/race-free launch must be proven | No uncontained fallback; never claim process safety based on frame validation |

The Windows Job Object source issue is independent and blocking: [Lu issue #7](https://github.com/vjmichael/Lu-AI-GUI/issues/7). The independent alpha source has referenced `Job::assign_and_resume_suspended` absent from the checked-in library. Other development snapshots use unsuspended `spawn` then `assign`, which is **not** proof of a contained-before-execution launch. Do not paper over the discrepancy by weakening the containment gate. Reconcile exact alpha source/artifact provenance first.

## Stage implemented here — real checked-in Rust contract, not a live listener

* Added `DlcCapability`, `DlcHello`, `DlcFrame` messages in the canonical `lu.v1` Protobuf schema, reusing the existing validated `InferenceStatusRequest`, `InferenceGenerateRequest`, `InferenceCancelRequest` and `InferenceEvent` messages. There is intentionally **no DLC model-load/unload operation**.
* Added `src-tauri/lu-core/src/dlc_interop.rs` with canonical version-1 bounded decode, exact u32 big-endian length-prefixed packets (one frame per packet), strict truncated/extra-byte/unknown/duplicated-field refusal and validated typed inference bodies. Invalid version, references, capabilities, image references, prompt or history are rejected.
* Added non-serializable host-held `DlcAdmission` gate: exact module, workspace and Lu session IDs; host-approved capability subset, one-time hello; reject cross-session traffic, unauthorized status/inference/images, request and generation replay, concurrent generation, cancellation of a generation the instance does not own, and post-revocation calls. A cancel request is not terminal proof; only host completion ends an active generation. The `host_bind_workspace` path now reads the **real** `WorkspaceHost` active module/instance ownership at bind and every request; suspended, closed, or wrong-owner workspaces are denied. An external app path still requires separate verified OS peer and approval wiring.
* Added Linux and Windows native Rust CI jobs exercising this module and existing `WorkspaceHost` / `inference_wire` contracts. **A source-level test cannot substitute for OS peer authentication, a GUI approval, a real GGUF run, or a compiled installed app.**

**Important trust boundary:** `DlcAdmission::host_bind_trusted` is an API solely for a trusted host adapter, not a public IPC command. Its constructor does not authenticate a peer, obtain user consent, verify active `WorkspaceHost` ownership, grant a model, or execute generation. Those are explicit integration gates. No socket/pipe server is opened by this change; all existing Lu inference commands remain internal.

## Required gates before NVP integration may claim operational support

1. **DLC-IPC-01 (this branch):** wire schema, version/capability checks, host-held deterministic admission, denial/replay tests on both OSes, exact source lock + CI.
2. **DLC-IPC-02 — native carrier:** Linux `AF_UNIX` must prove runtime directory ownership/mode, `SO_PEERCRED` / peer PID identity where supported, unlink/symlink/replacement refusal, restart cleanup and hostile same-machine-user tests. Windows named pipe must prove DACL binding to the intended user/profile, client impersonation/identity verification, pipe-instance spoofing denial, reconnect and cancellation; no pretending that the POSIX socket ACL model applies on Win32.
3. **DLC-IPC-03 — trusted Lu owner binding:** native UI opt-in (OFF by default); explicit module/instance registration and provenance, project/conversation identity, a revocable approval lease, bounded prompt/output and resource budget; bind each generation to a real persisted user request and to Lu's model scheduler. Host must enforce `WorkspaceHost` owner and `PolicyEngine` at time of use, not just at handshake. No keys/paths returned to DLC. Multiple concurrent DLCs cannot secretly acquire multiple large models or exhaust the machine.
4. **DLC-IPC-04 — NVP real client:** expose a Film provider through Lu **only after** the host is live. NVP cannot infer progress from a sample, unload Lu's model, assume ownership of memory/VRAM, or treat prior observed status as current. Support streaming, cancellation, host restart/lease expiry and model/provider handoff while preserving film provenance and artifact custody.
5. **DLC-IPC-05 — independent end-to-end proof:** native Linux and Windows test suites with a real small GGUF, text generation, image+MMProj, concurrent UI chat and NVP rendering, memory pressure/kill/restart, two independent user profiles, malicious callers, cancel/reconnect, upgrade and uninstall. Windows resource guardian cannot be accepted until [issue #7](https://github.com/vjmichael/Lu-AI-GUI/issues/7) is closed with actual safe worker launch and cleanup tests.

## Failure and ownership rules

* Missing peer proof, user approval, compatible version, live instance, exact session, or host resource admission => **DENY**, never auto-enable a listener or fall back to loopback HTTP.
* Only Lu launches/unloads the server and owns model memory. A DLC may request cancellation of its *own* active generation only; server completion is determined by Lu's terminal event.
* DLC outputs and film files stay in the respective user-approved project directories. Native tools, model calls and file writes remain separate effects with their own approvals, durable receipts and recovery; no authority is inherited from a read-only status view.
* Recovered leases are observation-only pending an independently authenticated fresh owner grant. No automatic continuation of uncertain external effects.
* Windows and Linux must pass the **same semantic conformance suite** plus their own OS identity/lifetime adversarial suites. A green Linux run does not sign off Windows.

## Test evidence ledger

| Gate | Current evidence | Status |
| --- | --- | --- |
| Protobuf schema + deterministic Lu Core admission | Initial frozen revision passed Windows/Linux CI run 41784832110; ownership-check continuation requires fresh exact-revision run | **INITIAL PASS; LATEST PENDING** |
| Linux live authenticated AF_UNIX inference | No live Lu carrier wired here | **NOT IMPLEMENTED** |
| Windows native pipe / checked-in safe suspended worker | Missing host adapter; separate source blocker issue #7 | **BLOCKED** |
| NVP live Lu provider with GGUF | NVP PR #33 only has source probe and read-only status boundary | **NOT IMPLEMENTED** |
| Accepted Windows `0.1.0-alpha.2` | Retained as immutable historic acceptance | **NOT RETESTED** |
| Full external local-source convergence (REPO-01) | Source lock covers GitHub branch, not the unpublished historic local snapshot | **OPEN** |

**Next step:** inspect exact Linux/Windows CI and repair only verified contract issues, then implement `DLC-IPC-02` platform-carrier proof on a subsequent separate development commit. Do not merge until verified or relabel this source contract as an operational external inference service.
