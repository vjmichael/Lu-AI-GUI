# ADR-0003 — Lu-to-IDE coding proposal boundary

**Status:** Proposed interface contract; **not implemented as an authenticated bridge**  
**Date:** 2026-11-05  
**Scope:** Standalone Lu IDE, optional future Lu model service integration. The stable Lu Desktop and Lu Core remain unchanged.

## Decision

Lu IDE is a standalone source-development environment. Lu may eventually connect as a separately running **coding collaborator**, but a remote or model-originated text proposal must **never acquire IDE filesystem or command-execution permissions merely by being received**.

The R3.4 proposal envelope is a first local **data-only validation model**, not a network or IPC protocol. Future Lu↔IDE messages must use a version-negotiated, authenticated Protobuf transport, separate from the existing local editor/Tauri commands.

## Roles and scopes

| Role | May read | May propose | May perform effects |
| --- | --- | --- | --- |
| Human IDE editor | Files in explicitly opened project | Human edits and reviews | Explicit bounded save via native host |
| Untrusted proposal payload | Nothing beyond fields passed into its validation | Data only; no trusted origin claim | **None** |
| Authenticated Lu session (future) | Specifically granted project content and selected diagnostics only | Task-specific, versioned source change proposals | **None directly** |
| IDE policy/authorization host | Scope-limited project catalog and capabilities | Validates proposal and provenance | Effects only on independent, valid human/task grant |
| Isolated IDE task worker (future) | Broker-granted inputs | Build/test results, diffs and diagnostics | Only OS-contained, auditable, cancellable effects |

## Handshake and task lifecycle (future)

1. Authenticate the local peer using a verified, OS-appropriate channel. Transport identity is not a user approval.
2. Negotiate the coding bridge ABI major/minor version, capability set, payload and resource limits.
3. Create an IDE-owned task ID with selected workspace/project ID, selected conversation context and **read-only** file/diagnostic scopes.
4. Send a bounded **context manifest**, not ambient filesystem paths, secrets, or full unrelated chat histories.
5. Lu returns a **proposal**, including task ID, untrusted explanation, target relative paths, original SHA-256 revisions, and bounded proposed text. The IDE independently validates all fields.
6. Display a complete review of every file. On incomplete/truncated review, the IDE must refuse reviewed application.
7. Present a **separate authorization** with each exact change hash and requested effect. The IDE, never the model, creates the authorization record. A simple model-provided `approved:true` is discarded/rejected.
8. After approval, IDE revalidates source revisions, identity/roots and policy, applies with its existing native filesystem broker, journals results, and returns structured receipts. If any file changes, halt rather than silently overwrite.
9. Build/test proposals are separate operations. No process invocation until Stage 2 Linux/Windows containment, quotas and cancellation pass.
10. Task cancellation and application restart must be durable and must not unintentionally cancel unrelated native Lu or NVP work.

## Explicit non-goals for R3.4

- No Lu model connection, execution worker, or automatic edit application.
- No raw shell execution or package install, even when requested by a proposal.
- No merging IDE Coding Chat threads with Lu's native chat or NVP film-project conversations.
- No implicit permission derived from a claimed producer string, origin metadata or repository contents.
- No use of one resident model to share a task's approvals with another client.

## Current implementation mapping

- `ide/host/src/proposal.rs`: local strict JSON schema-v1 envelope validator and **read-only** multi-file preview. The `claimedProducer` field is explicitly untrusted.
- `ide/host/src/review.rs`: bounded multi-hunk review and source revision validation.
- `ide/host/src/editor.rs`: existing explicit human save endpoint; **not invoked by the envelope**.
- `ide/app/web/main.jsx`: human-owned review and save UI.
- `ide/docs/IDE_07_R34_MULTIHUNK_PROPOSALS.md`: actual milestone status and tests.
- `ide/docs/ADR_0002_WORKSPACE_CHAT_BOUNDARIES.md`: separate chat/thread identity per application/workspace.

## Required future authorization tests

Reject: unknown Protobuf major versions, unauthenticated peers, expired/replayed task grants, repository prompt injection claiming system priority, changed source after approval, cross-project request ID substitution, inappropriate background effects, unapproved multi-file edits, tampered proposal hashes, malformed paths, and attempts to execute shell commands through the bridge.

This ADR must not be interpreted as proof that those tests or an IPC implementation exist today.
