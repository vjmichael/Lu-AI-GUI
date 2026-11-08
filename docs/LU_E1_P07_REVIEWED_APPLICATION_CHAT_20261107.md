# E1-P07 — reviewed external applications in Lu model and desktop chat

## Plain-English result

Previously the reviewed external Git and FFprobe adapters were host-only.
This change connects them to the actual native model tool interface and
adds an explicit one-turn desktop opt-in. The local AI model can request
reviewed observations; only Lu's trusted native host can open files,
authorize the request, launch the fixed external program, and publish a
verified bounded result. Lu Core itself is a native user-level desktop
application, not sandboxed. OS child supervision is lifecycle management.

## Available model tool names

- lu_reviewed_git_head — current HEAD hash, no arguments.
- lu_reviewed_git_status — changed tracked entry count, no arguments.
- lu_reviewed_git_branch — current branch or detached null, no arguments.
- lu_reviewed_git_history — reachable commit count, no arguments.
- lu_reviewed_media_facts — one WAV/MP4 duration and audio/video stream
  facts, no arguments.

Model-facing JSON parameters must be exactly an empty object. The host
picks and reviews the Git repo or WAV/MP4 through native OS dialog and
the installed executable through a fixed local system path. Model text
cannot supply paths, argv, executables, permissions, or output publication.

## Human user authority and audited execution

1. Explicit one-Send option in the composer: reviewed Git or reviewed
   media. This choice is mutually exclusive with Browse or Workspace.
   Native picker cancellation creates no task grant.
2. Host records the original prompt as a user message and verifies it
   from ChatStore, then issues a 300-second task with separate exact
   ReadWorkspace and ExecuteCode grants, scoped to this approved resource.
3. Lu registers only those host-reviewed operation identities into the
   current NativeToolRegistry and ModelToolCatalog.
4. NativeToolTurn processes untrusted model JSON, the codec rejects
   unknown fields, ConversationAuthority and host policy recheck
   original-human scope, then the reviewed broker starts the real OS
   subprocess. No new MCP or command execution framework is used.
5. The external process returns bounded stdout only to Lu's host. A
   separate parser validates Git or FFprobe semantics, verifies
   success and reaping, and writes a durable inner APPLIED journal effect.
6. The model receives only normalized facts via a typed ToolOutput.
   The conversation durable dispatcher uses a separate outer effect ID
   derived from the user turn. Different inner OS worker IDs prevent a
   double-journal collision.
7. User cancellation revokes the task via a live witness thread, causing
   the broker's OS process cancellation listener to kill/reap the child.
   Errors become uncertain, and automatic effect replay is forbidden.

No Git writes, arbitrary app command runner, shell, or model-controlled
file system access. The existing first-party Git reader remains available.

## Required exact-head acceptance (still OPEN)

Workflow: .github/workflows/lu-e1-p07-reviewed-app.yml.
- Source SHA/lock verification and React/Node UI contract tests.
- Four native registry/codec/authority denial/revocation tests.
- One real installed Git+FFprobe model-shaped JSON tool integration test
  on Ubuntu 24.04 and Windows 2025, including two typed normalized
  feedback results and four separate persisted outer/inner effect receipts.
- Windows Tauri compile with frontend build and two desktop host tests.
  The compile-only engine resource placeholders cannot count as real
  GGUF inference.

Even when these tests pass, the actual installed Windows local GGUF
model selecting tools in a visible Lu conversation, full user dialog,
cancellation behavior, and any physical-machine performance evidence
remain separate acceptance gates. A handcrafted model-shaped request is
NOT genuine model inference. Prior E1-P03/P04B/P05/P06B/C/D/E open
native and genuine-model gates must not be silently closed.

## Later work

General reviewed app APIs, writable editor/Office/media workflows,
terminal tools, binary upgrades, user policy controls and stronger
handle-pinned filesystem custody remain future separately authorized
work. Production Lu Core stays native and non-sandboxed.
