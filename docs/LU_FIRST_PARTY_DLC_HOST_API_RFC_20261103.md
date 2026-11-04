# LU-DLC-API-00 — First-party DLC Host Services RFC

**Date:** 2026-11-03  
**Status:** Proposed canonical platform contract for Lu Core; not implemented or shipped in the accepted \`0.1.0-alpha.2\` Windows desktop.  
**Tracking:** [Lu #8 — first-party bidirectional DLC Host API](https://github.com/vjmichael/Lu-AI-GUI/issues/8)  
**Existing Windows source integration defect:** [Lu #7 — missing suspended Job assignment method](https://github.com/vjmichael/Lu-AI-GUI/issues/7)

## Decision

Lu is a **platform**, not just a chat UI and not a hidden model server that each DLC must rediscover. Lu Core will offer **one generic first-party DLC Host API** that serves both:

1. **Core Extensions (CEs)** such as Cognitum: declare independently reviewed, typed native capabilities; the trusted Lu host chooses eligible capabilities, invokes the correct CE worker and verifies its evidence/results. A Cognitum CE is optional and remains separate from Lu Core.
2. **Workspaces** such as NVP Film, IDE, Research and Writing: distinct applications/UIs that **consume Lu host services** directly (with creator approval), including model access, generation/streaming, multimodal references, memory/ANSF, artifacts, scheduler, resources and cancellation. Workspaces may also use installed CEs and expose their own approved workflows, but they do not need to impersonate a human chat/model-generated tool call merely to request inference.

**Read-only model status is not the final API.** NVP B5 proved a narrow authenticated AF_UNIX status protocol with a simulated host, not a functioning Lu service or model-control interface. It remains a test fixture to reconcile/migrate, never the canonical API by default.

All first-party DLCs shall use the same *logical capability/permission/model-session contract*, whether accessed inside the trusted Lu process or from a separately running native application. **Internal vs IPC is a transport choice, not a different authorization regime.**

## Terminology and trust distinction

| Property | Meaning | Does it grant an action? |
| --- | --- | --- |
| First-party package provenance | Lu-vetted publisher/build hash/signed distribution | No. Eligible for trusted installation/enrollment |
| Installed | Package is present and signature/source verified | No |
| Enabled | User or administrative policy permits this specific DLC to participate | No |
| Discovered | A service/capability is known to this host and version-compatible | No |
| Authorized | Host-bound user/creator/project/session/turn/effect/resource approval is active | **Only for the precise requested operation** |
| Running | OS-managed worker/process and active leases are supervised | No additional rights |
| Completed | Durable terminal receipt and independently checked effects/results | No future rights |

First-party identity permits the host to rely on a **vetted component implementation** but does *not* confer unbounded file, network, GPU, memory, conversation or tool privileges. A model, WebView, workspace manifest, socket path or Protobuf reference is never an authority issuer.

### Common logical roles

- **Host-service consumer**: NVP and IDE call Lu to discover models, request a model session/lease, issue an inference request, subscribe to stream/status events, cancel their own request, read allowed memory/artifacts and release **their own lease**. They do not directly call llama.cpp private localhost endpoints or acquire Lu engine API keys.
- **Capability provider**: Cognitum implements \`calc.rational\`, C14 causal, etc. Lu invokes a registered, currently authorized, exact typed Protobuf operation, checks output evidence, records the host route and effect receipt, and renders/provides the result. Pure computation explicitly declares **no workspace read/write effects**; it must not masquerade as \`ReadWorkspace\`.
- **Host orchestrator**: Lu owns model/provider admission, resource guardian, permissions, capability registry, model-facing schema selection, ANSF ownership, worker launch, journal, cancellation and recovery. It can route between NVP/IDE/Cognitum only with explicit bounded caller grants; no transitive rights.

A Workspace and a CE can coexist in a single DLC package, but their *roles, manifests and grants remain distinct*.

## A. Service catalog and version negotiation

The versioned contract shall distinguish \`host_protocol_major/minor\`, \`capability_schema_version\`, \`module_manifest_version\` and \`domain_operation_version\`. Do not equate Cognitum semantic version \`1.3.0\` with \`lu.v1.ToolCall\` or with a DLC host ABI. Required services and optional features are negotiated explicitly, with deterministic refusal of unknown mandatory versions/capabilities.

**Initial Lu-owned services** (proposed names, not exposed in shipping code):

| Service ID | Direction | Behavior / effects |
| --- | --- | --- |
| \`lu.dlc.catalog.list\` | Workspace → Lu | List only installed/enabled/compatible visible services and reviewed operation contracts; discovery is inert |
| \`lu.model.list\`, \`lu.model.status\` | Workspace → Lu | Host-owned model IDs, state and measured telemetry only; no paths, keys or fabricated progress |
| \`lu.model.admission.estimate\` | Workspace → Lu | Estimate host RAM/commit/VRAM peak and report safe/unsafe/unknown against active workloads |
| \`lu.model.session.acquire\` | Workspace → Lu | Request **host-authorized** model lease/session and suggested context/profile; Lu decides load/reuse and returns opaque refs only |
| \`lu.model.session.release\` | Workspace → Lu | Release only the caller's approved lease. **Not** unconditional model unload |
| \`lu.inference.start\`, \`lu.inference.stream\` | Workspace → Lu; Lu → Workspace | Text + admitted multimodal refs and history, bounded token streaming, ordered sequence numbers, terminal success/failure/cancel receipts |
| \`lu.inference.cancel\`, \`lu.inference.recover\` | Workspace → Lu | Scoped cancellation of caller-owned generation and explicit reconnect/replay from journaled sequence; never kill foreign work |
| \`lu.resources.snapshot\`, \`lu.resources.reserve\` | Workspace → Lu | Report genuinely measured usage and host-approved resource ticket; no arbitrary GPU/OS quota claims |
| \`lu.ansf.query\` | Workspace → Lu | Approved scoped memory retrieval, provenance and redacted references; no direct ANSF DB access |
| \`lu.artifact.resolve\`, \`lu.artifact.commit\` | Workspace ↔ Lu | Brokered, content-addressed, project-scoped artifact operations, with atomic edit/effect journal and explicit grants |
| \`lu.ce.capabilities\`, \`lu.ce.invoke\` | Lu → CE | Reviewed provider capability discovery and exact type-checked CE invocation/results. Manifest alone never runs a tool |
| \`lu.host.activity\`, \`lu.host.cancel\` | Bi-directional events / caller-owned cancellation | Host event IDs, progress if measured, durable effect/recovery state, only the caller's active task |

The final ABI may split asynchronous event subscription, backpressure/credit and large-asset streaming into subchannels; a single giant generic JSON/RPC command is **not** authorized by this RFC. Domain payloads are strictly typed Protobuf and matched to registered type URLs; unknown fields/required capabilities reject or are version-negotiated.

## B. Flow for first-party NVP / IDE Workspaces

1. User opens the Film or IDE Workspace. Lu verifies installed first-party package identity, enabled state and selected project. A trusted desktop action grants a bounded project/session operation set (duration, workload and memory budget), possibly with per-high-risk-action confirmation.
2. Workspace negotiates protocol/version and requests \`model.list/status\`. Lu may reveal a host-safe model catalog, never a model filesystem path or underlying loopback API key.
3. Workspace calls \`model.admission.estimate\` then \`model.session.acquire\` with an approved model **reference**, expected context/modality and resource hints. Lu's guardian measures headroom and arbitrates concurrent Lu sessions and externally declared Blender/encoding reservations. Unsure or unsafe => refuse/queue; no silent backend switch or automatic overcommit.
4. Lu returns a **host-issued session and lease** with model provenance and effective settings. A second Workspace may reuse the same loaded engine if supported, but receives a separate, independently authorized session/queue. NVP cannot stop an IDE-owned generation.
5. Workspace issues \`inference.start\` with typed conversation context, generator profile and pinned image refs. Lu emits ordered \`chunk\`/state/usage events with correlation IDs and terminal receipts; backpressure and sequence resume are handled by host. JSON translation occurs only at Lu's actual external provider boundary.
6. Workspace can \`inference.cancel\` its own generation. On dropout, reconnect requires host-owned identity and \`inference.recover\`, not blind resubmission. Uncertain non-idempotent effects remain \`UNCERTAIN\`.
7. Workspace releases its own session/lease. Lu determines whether a model remains used by other sessions or can be unloaded. An observation or a revoked/closed DLC is **not proof of model unload**.

## C. Flow for Cognitum provider CEs

1. A signed, installed, enabled Cognitum Core Extension proposes a reviewed set of typed operations via its **inert** \`cognitum.v1.CoreExtensionHandoff\` manifest. Its operation schemas and source hash are pinned and matched to Lu's host registry, tool effects and capability catalog.
2. Lu decides whether the exact \`calc.rational\`/other operation is available/healthy and relevant, and injects only permitted small model-facing schemas for the active saved human turn. Discovery is not an execution grant. A Workspace may separately ask the host to invoke a reviewed CE.
3. Lu's host authorization checks original saved user turn or trusted UI operation, project, CE identity, exact typed operation, resource budget, effect-free or brokered-effects classification, current revocation, idempotency and priority.
4. Lu invokes Cognitum with canonical Protobuf bytes via a supervised, constrained worker or an explicitly safe in-process first-party service. Cognitum returns domain evidence and uncertainty/result assurance. Lu independently verifies actual output and provenance, then journals result and safely exposes it to the requesting Workspace/model.
5. Revoking or disabling Cognitum drains/terminates its own work without terminating NVP, destroying the Lu model or corrupting ANSF.

Existing \`lu.v1.ToolCall/ToolReply\` and \`InferenceFrame\` remain internal legacy/core contracts to adapt **explicitly**, not grounds to leak engine permission onto generic sockets.

## D. Transport, OS and deployment profiles

**In-process first-party Workspace SDK:** Direct Tauri/Rust host-call service facade, same validated method set and authorization checks, no AF_UNIX required. Useful for Film Workspace when integrated inside Lu.

**Separate first-party app / worker:** Authenticated local IPC to a Lu-owned listener/worker through exactly the same host facade. Native AF_UNIX with platform-specific endpoint/peer checks where proven; fall back to Windows Named Pipes if native Windows AF_UNIX cannot satisfy peer ACL/reparse/identity requirements. Linux profile-owned private directory, symlink/inode checks and peer identity. Never use a default public TCP server. No Docker, Jan or required MCP.

**Different architectures are possible:** NVP can continue standalone with its independently managed render pipeline and optional other model provider when Lu is absent; a dedicated Lu Film DLC can use the same logical host calls and authoring state. Neither path may duplicate model ownership or infer permission from mere connectivity.

## E. Authorization, durable recovery and resource custody

- Auth grants originate only in Lu's trusted host (real user/creator action, verified profile/project/turn), scoped by exact operation, model session, resource/artifact, effect, time, and method version. First-party signatures identify packages, not user consent. The Lu host rechecks before dispatch and at sensitive effects; revocation is durable and effective for future requests.
- Every request has host-issued opaque task/session refs, correlation ID and idempotency key. Event streams are monotonic and bounded, and terminal status is separately recorded. One-time transport challenges prevent replay but **do not replace** operation-level host authorization.
- Worker restart/host death: no simulated success from socket closure; use journal PREPARED/APPLIED/UNCERTAIN/VERIFIED recovery states. Cancellation is a request, not proof of termination. Windows Job Objects / Linux cgroups need independent real-process cleanup evidence.
- The resource guardian must account for **physical RAM, commit, VRAM and peak inference allocations** separately. NVP owns its Blender workers, but can expose measured resource commitments to Lu's broker; missing trustworthy cross-app telemetry denies unsafe automatic coexistence rather than fabricating free memory.
- Host-owned model processes are never exposed to DLCs as arbitrary executable paths, process-kill IDs, private ports or API keys. Release-own-lease is *not* force-unload.
- ANSF references and artifact handles have explicit project/profile sensitivity scopes; a returned model text or Cognitum HDC candidate can never become a capability grant.

## F. Release acceptance (do not infer from partial CI)

**DLC-API-00:** Document/schema/version negotiation, Cognitum CE + NVP Workspace fixtures reviewed; first-party identity and effect matrix frozen.  
**DLC-API-01:** Lu Rust host service facade passes **real in-process** capability/status and denial tests.  
**DLC-API-02:** Separate native Windows and Linux IPC host+client tests (real Lu implementation, not merely NVP Python HMAC test-host), user/project/peer binding and OS controls; secure fallbacks documented.  
**DLC-API-03:** Real pinned GGUF/MMProj session acquisition, text/multimodal streaming, cancellation and lease isolation with two simultaneous Workspaces; no cross-session unload.  
**DLC-API-04:** Cognitum \`calc.rational\` verified end-to-end with no invented ReadWorkspace effect, typed result evidence and independent re-verification.  
**DLC-API-05:** NVP Film → Lu actual inference plus concurrent Blender resource contention/admission, no desktop freeze; optional IDE sample.  
**DLC-API-06:** Stop/restart/revocation/host death, bad version/frames, corrupted journal, duplicate request and worker escape tests. Native Windows/Linux CI, then physical installed Windows acceptance.  
**DLC-API-07:** SDK/examples, migration/compatibility policy, signed distribution tests and explicit version promotion. First-party APIs may evolve independently of core v1.0 if the release scope remains deferred.

**Important:** The already accepted Windows \`0.1.0-alpha.2\` is a **functional baseline**, not evidence that this DLC ABI is implemented. [Lu #7](https://github.com/vjmichael/Lu-AI-GUI/issues/7) independently blocks checked-in Windows source compilation until safe suspended/Job assignment is reconciled; do not fix it via unsuspended launch.

## Cross-repository alignment

- **Cognitum:** \`docs/LU_FIRST_PARTY_DLC_PROVIDER_HOST_CONTRACT_20261103.md\` maps native CE producer paths, pure computation, verified evidence, HDC/causal component gating and no Lu modifications as part of Cognitum work. Preserve \`docs/lu-dlc-ce-addenda.md\` as a proposal register.
- **NVP:** \`docs/NVP_FIRST_PARTY_LU_DLC_WORKSPACE_CONTRACT_20261103.md\` maps the Film Workspace consumer, standalone-app IPC and in-Lu host SDK, Blender resource custody and NVP B5 status-only prototype migration.
- **Lu:** This document is the proposed normative platform-level design. Lu #8 is the implementation backlog; it is NOT approved deployment or an implemented endpoint until separate host/CI/physical release gates pass.
