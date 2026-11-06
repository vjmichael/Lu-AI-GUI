# Lu Wave 3 — native tools in ordinary conversations

Status: **WAVE3-CHAT-01A SOURCE-IMPLEMENTED**, native and installed Windows verification pending.
Branch: `lu/wave3-conversation-tools-20261105`. Base: Wave 2 recovery/cancellation source head `a2628a50197a39f9a1cbbfe1a165320c05378078`.

## User flow

Ordinary **Send** continues to use the existing direct streaming GGUF route unless the operator explicitly activates **Use workspace tools on Send**. This option only sets a one-shot boolean. On Send, Tauri opens the native OS folder chooser; canceling it starts no tool turn. A chosen folder is admitted through the existing `WorkspaceReadTurnSession`: saved exact user message, per-turn source verifier, native registry, Protobuf argument codec, read-only policy, model tool selection, bounded iterations, model-turn audit and revocation.

The WebView supplies neither directory paths nor permissions. A model cannot upgrade read to write, invent a tool, request an unselected folder, or reuse another conversation's turn. The existing **Ask Workspace** path calls the same implementation; no replacement framework or unsupported provider backend has been added. Users can still submit normal text/images through Send with the option off.

## Acceptance

1. Source wiring/JS regression asserts one-shot selection, native picker routing and no model-selected paths.
2. Native desktop tests validate persisted user-turn verification across restart, cross-chat denial, tool refusal and cancellation.
3. Installed Windows GGUF: explicit opt-in to normal Send, select project folder, ask read/search/list question, inspect tool audit + response, cancel while tool is running, verify later Send has no workspace authority.
4. Wave 3 write-capable integrations require separate native full-content review, authority, journal and recovery. This first slice is deliberately read-only.

No claim of full Wave 3 completion, native green CI, or release acceptance.
