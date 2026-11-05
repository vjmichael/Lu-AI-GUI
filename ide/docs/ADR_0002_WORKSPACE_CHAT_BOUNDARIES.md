# ADR-0002 — Native chat and domain workspace conversation boundaries

**Date:** 2026-11-04  
**Status:** Accepted design constraint; cross-application integration deferred  
**Scope:** `ide/` in `vjmichael/Lu-AI-GUI`; describes the intended distinction between Lu general chat and domain-specific workspace conversations, not a change to Lu Core or NVP source.

## Origin

The user's original `Jan-Mod-Design-Document.md`, §3.9 (*Workspace Extensibility*) states that the core is shared, but the workspace changes the UI, available tools, AI behavior, contextual state, workflows and domain assets. Its illustrative examples are:

- **Code/IDE:** editor + chat + terminal; coding/project context; debugger, linter, Git, tests; debug-oriented AI.
- **Film Studio/NVP:** timeline + asset browser + preview + chat; scene/shot/asset context; editing, rendering, audio, film workflows.
- **Native/general Lu chat:** general conversational interface, core reasoning, memory, native tools and user interactions without assuming a code project or active film.

**Clarification:** The chat *surface and scope* inside a workspace is not the same experience as native Lu chat. Shared or borrowed model inference is an implementation possibility, not a requirement to display the same UI, use the same active thread or expose the same tools.

## Three distinct interaction experiences

| Experience | UI and working context | Authorized capabilities |
| --- | --- | --- |
| **Lu native chat** | General chat history, normal AI prompts, general memory and ordinary task workflow | Lu's native capabilities and each human-approved permission |
| **Lu IDE** | Editor, project explorer, coding chat, terminal, diagnostics, debugger, tests, dependency management and advanced Git | IDE project-specific operations under the IDE host's own scoped grants and OS worker isolation |
| **NVP / Film Workspace** | Project/film-oriented chat alongside screenplay, scene/shot context, timeline, preview, assets, audio and rendering workflows | NVP film-project operations under the film application's own domain-specific authority |

NVP is referenced here only to preserve the conceptual distinction. This ADR does **not** dictate whether NVP eventually embeds, runs standalone, or connects by bridge; those integration decisions are governed by the NVP/Lu workstreams.

## Conversation, model and workspace identities must not be conflated

Every assistant interaction should have independently tracked identity dimensions:

- **Client/surface:** `lu.native-chat`, `lu.ide`, `nvp.film` or a later registered client.
- **Conversation/thread ID:** identifies the specific message history.
- **Workspace/project ID:** identifies the IDE project or film project; may be absent in native chat.
- **Task/operation ID:** tracks planned work, cancellation, effect receipts, recovery and provenance.
- **Model runtime identity:** identifies the separately owned inference engine, if any.

These identifiers are **not interchangeable**. Reusing a model process does not merge histories, working contexts, permissions or application databases.

### Conversation scoping

1. Default to separate per-conversation history and per-workspace state. Opening IDE does not import the current native-chat transcript; opening NVP does not import IDE source context.
2. IDE coding chat has access only to the registered project and explicitly granted project files, diagnostics, Git diffs, task state and selected conversation links. NVP filmmaking chat is scoped to its film/project artifacts. Native Lu chat does not silently inspect either workspace's private assets.
3. The earlier design's **soft switch** may preserve a conversation thread *when the user explicitly intends to carry it across workspace surfaces*. It must preserve provenance of each message, display the context transition, and revalidate every permission in the destination. It is an optional **designed behavior**, not implemented in this standalone alpha.
4. Support explicit **link/include selected context**, or a reviewable summary/handoff between workspace chats; never mix full histories automatically. An apparent continuation must not silently extend the authority of a prior workspace's user grant.
5. Conversation and project state should be durable and discoverable in the appropriate client, with ownership/access controls. A future cross-device/account-level view must not make one workspace's tools or raw file permissions available in another.
6. Account ownership, project association and authorized tool visibility are separate from the shape of the chat UI.

### Shared model access (future)

The earlier Lu Development Mode RFC separates **UI/client identity**, **workspace/build-worker identity**, and **inference/model residency**. A workspace may later borrow one already-loaded Lu model through a trusted, authenticated, versioned interface. This must preserve independent conversation context, bounded queued work, resource arbitration and per-client cancellation. Starting the IDE should not auto-load a second expensive model or require Lu to be running.

For the currently implemented `ide/app`, the correct visible state is **Lu connection: Not connected**. There is no chat engine or inter-app model bridge yet. Until that bridge exists, the IDE must remain useful as a standalone software-development environment without pretending its coding chat is connected.

### Trust and authorization

- An IDE coding chat is a domain-specific **agent interaction surface**, not unrestricted terminal authority. Model output and source files are untrusted input; agent proposals undergo IDE host validation, user consent when required and OS-enforced execution boundaries.
- NVP film requests require film-project-scoped authorization; NVP's own command/asset APIs determine valid actions.
- Lu's native chat cannot grant access to IDE project roots or NVP film assets merely because the applications share branding or a future model backend.
- An inter-app handoff passes typed scoped content with provenance, not ambient security credentials, raw private memory databases or reusable privileged process handles.

## Standalone IDE implementation implications

- Keep `ide/app` independently launchable and `ide/host` independent of `lu-core`.
- **Future UI:** Add a project-aware **Coding Chat** panel alongside editor/diagnostics when chat integration is actually available; do not place the native Lu chat page inside the IDE window.
- The coding chat will have its own project/thread selector, history, context attachments, visible task plans, diff reviews and tool outcomes. Its messages can be displayed alongside the editor without replacing the editor.
- The future Lu bridge should expose model inference and optional planning as explicitly negotiated capabilities, not take over the IDE's window, filesystem or worker supervisor.
- NVP's film chat is analogous only as a **workspace-specific interface**, not as a dependency or UI template for the IDE.
- Preserve this architectural distinction in test expectations and UI labels; do not introduce a single undifferentiated shared "chat" across all clients.

## Acceptance cases for a future inter-app bridge

- IDE coding chat works as a project-scoped surface; no automatic native Lu transcript import.
- Two different IDE projects produce separately identified histories and context manifests.
- NVP film chat context is not visible to IDE without explicit selected-content handoff.
- Native Lu chat and IDE may use one verified model runtime while retaining distinct conversation state and permissions.
- Cancel IDE task does not cancel another user's native Lu chat or unrelated NVP task.
- A conversation intentionally transferred between surfaces shows an explicit transition, provenance, selected context and renewed destination permissions.
- Disconnecting Lu leaves the standalone IDE editor, project catalog and authorized non-AI workflows usable.

## Implementation status

This ADR is **design documentation**, not an assertion of implemented coding chat, shared model residency, NVP integration or context handoff. The standalone IDE alpha currently contains only the project-management shell and the native metadata registry.

See [ADR-0001](ADR_0001_STANDALONE_APP_AND_LU_BRIDGE.md) and [canonical eight-task design](IDE_WORKSPACE_DESIGN.md).
