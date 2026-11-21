# LU-IPC-01A/B/C — Shared Lu Core ↔ DLC protocol v0 (source proposal)

**Status:** source contract + structural Rust codec only. NO external listener, OS peer authentication, trusted SDK, IDE integration, CI/native compile, installer or model sharing acceptance. Transport still undecided.

**Owners:** Lu Core is the authoritative host for the selected model and memory/ANSF, inference scheduler, native registry, per-turn user/task authority, journals, cancellation and recovery. IDE, NVP, Cognitum and third parties own their domain state, UIs and independently approved effects.

## Reused current code (as inspected at \`ebbfc4ae\`)

- \`lu-core/proto/lu/v1/core.proto\`: existing internal \`ToolCall\`, \`ToolReply\`, \`InferenceFrame\`, typed tools, diagnostics; unchanged by v0 contract.
- \`registry.rs\`, \`model.rs\`, \`contracts.rs\`: existing typed native capability registry, model-facing mapping and effect declarations.
- \`conversation_authority.rs\`, \`durable.rs\`, \`journal.rs\`, \`agent_tool_loop.rs\`: existing original-user bound approval, dispatch and durable recovery semantics.
- \`inference_bridge.rs\`, \`generation_queue.rs\`, \`inference_host.rs\`: Core inference/cancellation/streaming and single model residency owner.
- \`ide/proto/lu/ide/v1/workspace.proto\`: IDE-local project selection/list/focus contract, not the Core IPC to replace.
- Cognitum \`cognitum.v1.CoreExtensionHandoff\`: currently non-authorizing CE manifest proposal and typed domain schema.
- NVP must supply production-bible, scene, actor, camera, timeline, render and asset schema URLs through its own domain protocol, not copy whole USD scenes or frames into generic IPC.
- Historical \`docs/contracts/lu-dlc-host-v1.proposed.proto\` on closed PR #11 was unregistered design; useful for resource/model lifecycle vocabulary, **not** an already shipped runtime. Old 2026-11-03 AF_UNIX-only parking-lot document is superseded by issue #240's transport-open direction.

## Contract rules

1. \`lu.workspace.v0\` is a **draft schema version**; the initial wire header advertises \`protocol_major=1\`, \`protocol_minor=0\`. Incompatible majors and unknown minor versions fail closed; forward minors require an explicit reviewed migration. No false automatic compatibility.
2. \`Hello\` package/module/role/digest are **untrusted claims**, never OS peer proof. The eventual native transport adapter must bind process, OS account/profile, installed package identity, signed provenance where available and host-minted session lifetime *before* dispatching anything.
3. Protobuf is the internal envelope, **not the permission system**. No message can mint a grant, tool, OS resource, trusted identity, executable path or model secret. Lu binds \`OriginRefs\` to canonical ANSF user turn, workspace, task, operation and the original permission manager; domain hosts separately verify effects.
4. Discovery/registration declarations are inert. The runtime adapter must map registered types to **existing** native tool codecs and host capability registry, with approved schema/effect pairs. Exactly one active per capability ID/version/scope, revisioned health/detach and negative tests. Do not create a second generic tool registry.
5. \`EFFECT_CLASS_PURE_COMPUTE\` is reserved to represent Cognitum honestly. Existing \`ToolEffect\` has no pure variant, so the adapter must refuse this capability until a reviewed pure-effect bridge is implemented. Never masquerade a solver as \`ReadWorkspace\`.
6. \`CoreInferenceRequest\` contains an original request and opaque context references, NOT an engine path, model load command, API key or worker privilege. Only Lu Core resolves admitted context and selects the user's current model; one model lifecycle remains authoritative.
7. \`ObjectRef\` is a bounded metadata locator (owner/ref/hash/size/expiry), not a bearer capability. Resolve under fresh caller/workspace/turn/read rights and path-safe owner policy. Large USD, files, frames, film and mathematical tensors travel by independently admitted references; arbitrary external pointers are denied.
8. Cancellation acknowledgement means the signal was recorded, **not** that an effect was undone or the worker stopped. Durable \`TerminalState\` includes UNCERTAIN; recover via original operation id and OS/domain oracle before retry. Never replay effects blindly on reconnection.
9. Streams have monotonic sequence per operation, bounded 64-KiB typed payload, 256-KiB frame, explicit 1–64 event credits. A slow client must cause bounded backpressure/controlled interruption, never unbounded queueing.
10. NVP \`ResourceTransitionRequest\` is cooperative only; Core may defer/deny an unload/reload while existing chats/leases are active. Report measured RAM/VRAM only with separate genuine OS/backend evidence. Cognitum mathematical results need effect/assurance provenance in registered output schemas. IDE domain effects remain protected by its native supervisor.
11. \`FocusWorkspace\` uses opaque host-resolved workspace refs; OS owner must actually verify launch/focus. App-disconnected UI must distinguish unavailable, deferred, cancelled and uncertain. Core may later outlive a chat window, but this source contract does not alter process lifetime.
12. This source intentionally does not pick AF_UNIX, named pipe, shared memory, gRPC, TCP or MCP; after native identity and threat model are qualified, measure transport on installed Windows and Linux. Host side manages framing, verified peer/session identities, rate limits, deadlines and stream capacity.

## Concrete interoperability map

| Participant | Role | Offers | Consumes |
|---|---|---|---|
| Lu Core | authoritative AI host | single selected model, common typed registry, ANSF/journal, grants, scheduler, model events, cooperative resource handling | approved domain capabilities under original user/task grants |
| Lu IDE | Workspace | project picker/trees, revisioned diff, protected build/test/debug, local Git/gh review, verified results | Core model reasoning + shared context with explicit project scope |
| Cognitum | Core Extension/provider | typed HDC/KAN/SSM/solver operations with mathematical uncertainty and source proof | optional Core scheduling, user-authorized data references |
| NVP | Workspace + provider | production bible, typed scene/Take data, review, native worker/render operations | Core model, optional math providers, cooperative RAM/VRAM release/reload |
| Future/third party | untrusted until separately enrolled | only signed/registered exact scoped capabilities | least-privilege discovery and host-approved invocations |

## Native acceptance plan (not executed)

- **Gate A:** protoc/prost schema compilation, Rust roundtrips, strict malformed/unknown/duplicate bytes, incompatible versions, bounds, no cross-chat claims, fake package/app identity denial by later OS adapter. Existing \`lu.v1\` wire unchanged.
- **Gate B:** first real installed Windows Lu ↔ IDE process. OS peer identity, package and account, per-turn grant, protected IDE worker, exact source/diff review, cancel, revoked grant, replay, crash, uncertain outcome and OS receipt verified independently.
- **Gate C:** Cognitum pure computation or NVP typed provider exercises the same envelope without a second tool/model registry. Default-off/disabled/revoked behavior, malformed resource claims, object reference ownership and unsupported schema rejected.
- **Gate D:** installed same-hardware model sharing, streaming p50/p95 and memory/VRAM contention; real release/upgrade/disconnect/rollback. No physical acceptance from source tests.
- **Policy:** one grouped exact-SHA acceptance when the shared CI queue is idle, and no changes to frozen Lu/IDE branches, existing PRs, workflow triggers or mainline until separately reviewed.

**Next:** LU-IPC-01B-R2/01C: independently native-compile this v0 codec, repair genuine Rust/protoc issues, then implement a host-owned registration/peer-admission broker over a transport only after the OS identity/threat model. No rollout claims.

## Shared-model lifecycle clarification

The v0 schema also contains `SharedModelStatusRequest` / `SharedModelStatusReport` and `ModelLeaseRequest` / `ModelLeaseOutcome` / `ModelLeaseRelease`. They represent Core-observed status and scoped cooperative reservations, not DLC-owned model loading. Model leases are host-issued references, not transferable authority. A denied or deferred lease must not expose an active lease ID; reported RAM/VRAM remains optional without real observation. Each native adapter must reconcile existing chats and competing DLC leases before modifying GGUF residency.
