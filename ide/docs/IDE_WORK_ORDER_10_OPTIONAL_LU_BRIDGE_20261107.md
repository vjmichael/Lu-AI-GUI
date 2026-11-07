# IDE Work Order 10 — Optional authenticated Lu Core bridge

**REVISED USER DECISION — WO10 IS POST–LU CORE v1.0 (November 7, 2026).** Do not resume WO10 interface design or implementation **before Lu Core reaches v1.0** and the user explicitly directs resumption. The present bridge/Protobuf/IPC prototypes are exploratory, not the selected integration architecture; keep PR #76 draft/unmerged and exclude it from installers. Lu IDE continues standalone.

**Architecture decision intentionally deferred:** Evaluate Lu Core v1.0 alongside Cognitum, NVP, and the full DLC Workspaces and DLC CE ecosystem before choosing a shared integration framework. Consider whether logic/reasoning pathways belong inside Lu Core and how core tools, workspaces and CE capabilities are admitted, discovered, supervised, cancelled and given durable context. Compare native in-process interfaces where appropriate, local AF_UNIX sockets on Linux (with corresponding supported Windows transport), Protobuf-encoded IPC, and API/RPC approaches rather than committing now. **AF_UNIX is a transport; Protobuf is a serialization format; an API is an interface contract, so these are not necessarily mutually exclusive.** Retain host ownership of files, permissions and process execution. No cross-workspace memory or tool authority is implied by attachment.

**Reopening gates:** (1) Lu Core v1.0 reached and its architecture stable; (2) audit actual Cognitum/NVP/IDE and other DLC workshop/CE requirements; (3) select/test a cross-platform contract and native security/worker lifecycle; (4) obtain an explicit user instruction to resume WO10. None is satisfied merely by code on the deferred branch.

**USER DIRECTIVE — DEFERRED (November 7, 2026):** Suspend all Lu Core ↔ Lu IDE interface and integration work until explicitly resumed. This includes IPC/peer pairing, model sharing or inference handoff, coding chat, proposal delivery, and Lu-related connection UIs. Work already committed on this experimental branch is **incomplete and unverified**, must stay in a draft PR, and must **not** be merged, shipped, enabled, or treated as WO10 acceptance. Keep Lu IDE standalone. Resume other, non-interface work only on explicit instruction. WO9-F and earlier release/installed acceptance gates remain independent and open.

**Started:** November 7, 2026 (UTC). **Branch:** `ide/wo10-optional-lu-bridge-20261107` stacked on WO9 draft PR #75.

**Status:** Contract and offline security gates implemented for native CI. NO live connection, peer pairing, inference, coding chat, or automatic project access has been accepted. This is the tenth and final planned IDE work order. All WO4–WO9 independent OS acceptance remains open where not proven.

## Objective

Allow the standalone Lu IDE to use an already-running Lu Core model **when independently authorized** to help reason about selected code and propose edits. A Lu Core model must not become a source of IDE filesystem, Git, worker, or administrative authority.

IDE continues to launch, edit and run approved native tools without Lu. Lu runs separately; the IDE neither embeds Lu Core nor links a core Rust library. No Docker and no extra required manual installer.

## Work breakdown

| Task | Deliverable and acceptance | State |
| --- | --- | --- |
| **WO10-A Protocol** | Independent versioned Protobuf `BridgeEnvelope`, canonical wire validation, bounds, optional protocol negotiation, project/thread IDs, proposals as data, no authority-bearing paths/tokens. Host compiled and tested Windows/Linux. | **Source implemented**, native exact-head CI pending |
| **WO10-B Peer + human security** | Native OS-verified peer identity **AND** explicit project/turn consent; challenge nonce; one-use replay/sequence; advertised capabilities not grants; revocation; cross-project denial; default off. | **Host policy FSM implemented as isolated/test-only primitive**. Real peer-proof constructors intentionally absent until transport verified |
| **WO10-C Live local transport** | Actual Windows kernel named-pipe peer identity / Linux AF_UNIX peer credential inspection; protected per-user endpoint, bounded framing, mutual authentication, disconnect/reconnect; independent Lu Core handshake acceptance. No blind localhost auto-discovery. | **OPEN — not enabled** |
| **WO10-D Coding chat + model handoff** | IDE-owned per-project threads, isolated memory, selected context only, inference via authenticated Lu Core with independent quotas/cancellation, no native-chat transcript import. | **OPEN — no model request/response connected** |
| **WO10-E Reviewable changes** | Bridge proposal passes strict `ProposalEnvelope` schema and matching project/task, then **separate** `IdeProposalInbox::import` and human review; staging/saving/building remain the IDE's existing independently granted actions. | Data-only bridge policy schema/admission path implemented; actual peer-to-inbox delivery **OPEN** |
| **WO10-F Installed and adversarial acceptance** | Independent real Windows/Linux installed IDE+Lu native peer identity, wrong-user and fake-peer tests, replay/oversize/project isolation, per-turn revocation, model outages, tool grant separation, crash cleanup and accurate release evidence. | **OPEN; no connected app evidence exists** |

## Implemented source

- `ide/proto/lu/ide/v1/bridge.proto`: version 1. Six typed envelope bodies: Hello, HelloAck, ProjectContextOffer, ProposedChangeOffer, Failure, Goodbye; bounded nonce and capability enumeration; transport does not carry filesystem roots, shell commands or reusable grants. Separate from `workspace.proto` (existing local IDE UI lifecycle).
- `ide/host/build.rs`: compile both Protobuf messages through existing vendored `protoc`.
- `ide/host/src/bridge_wire.rs`: strict canonical Protobuf serialization, replay-safe header structure, version/session/sequence/nonce rules, bounded context and proposal frame limits, allowed capability values and IDs; hostile unknown field/duplicate/invalid-length tests.
- `ide/host/src/bridge_policy.rs`: OS peer and separately user-authorized project scopes represented by private unforgeable host types; **no production OS peer proof constructor yet**. Test-only pairing proves wrong-peer, cross-project, wrong-turn, duplicated sequence and revoked-session refusal. Strictly parse the inner `ProposalEnvelope` and match project/task before acknowledging any data proposal; proposal handling does not import, write, execute or save.
- `ide/app/src-tauri/src/lib.rs` + `ide/app/web/main.jsx`: **read-only** `ide_bridge_status` and an explicit Lu connection view. State is truthful `not_connected`, disabled, not paired, no OS peer verified. No automatic socket connection, no connect command, no copied chat transcript. The existing app remains standalone.
- `ide/host/src/capabilities.rs` / `capability_schemas.json`: one bounded read-only status operation with typed schema; capability discovery `connected_to_lu=false`; frontend exact-allowlist updated. Discovery never authorizes any effect.
- `.github/workflows/ide-wo10-optional-lu-bridge.yml`: Windows and Linux canonical Protobuf/security FSM tests and frontend checks. Source-only status must never be reported as installed or connected Lu acceptance.

## Safety gates and platform constraints

1. **User decision to proceed to WO10 is authorization to develop**, not authorization to expose a running local IPC listener or import source files into Lu. Live integration will require explicit human opt-in and separately authenticated local peers.
2. Existing source/worker guarantees must hold: all Lu-generated text, Protobuf fields and project code are untrusted; IDE host validates real project ID, current source revision and independently granted effects; OS workers preserve WO6 protected containment.
3. No incoming Protobuf message may carry an absolute host path, shell command, delegated tool grant, credentials, or shared native Lu memory database. A task/correlation ID is not a credential.
4. A HELLO message claims an identity; it never proves the identity. Actual kernel peer credentials, signing/publisher policy and human consent are not implemented merely by compiling this schema or exercising a unit-test peer.
5. A coding chat thread ID remains separate from a native Lu thread, project catalog entry, supervisor job ID, or model worker ID; no automatic transcript/context merging.
6. **WO10 release remains default deny** until native authenticated Windows and Linux installed applications successfully perform model inference, selected context review and proposal import under independently verified security/OS test evidence.

## Current verification disposition

No runtime connection or signed release is claimed. WO10-A/B native code and the read-only UI were committed for exact-head CI; task C onward remains work. WO9-F’s failed/pending native installed CI and upstream WO6–WO8 signed protected acceptance are carried as separate blockers, not assumed closed by starting WO10.
