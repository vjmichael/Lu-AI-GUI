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

## WAVE3-CHAT-01B: bounded native action proposals from ordinary Send
- The completed assistant output may now contain only `{kind:"propose_native_action",action,purpose}` for a tightly enumerated list: copy_file, move_file, trash_file, extract_archive, create_document, revise_office, batch_office.
- The host conservatively classifies the actual current user prompt and refuses proposals for different actions, ambiguous requests, negated requests, paths, permission fields, additional fields, duplicate JSON keys, other conversations or replays. Background-generated notifications cannot stage native proposals.
- The frontend shows an inert, chat-bound proposal card. Accept consumes the staged suggestion but can **only** open the already installed native review workflow; no filesystem authority is conferred. The operator must still select the source/destination in OS dialogs, read the full review, acknowledge and approve with the native confirmation. Denials discard the proposal. `CreateDocument` and `ReviseOffice` open their existing editors rather than guessing missing values.
- Unlike a general autonomous write-tool loop, this safety-first bridge deliberately does not permit a model JSON response to trigger a destructive action. It reuses Wave 2's durable journal and recovery machinery at actual execution.

## WAVE3-CONT-01: actual read-only model/tool evidence in ordinary chat
- `ModelTurnJournal::summaries_for_conversation` now projects per-task counts/status/time from the real SQLite model-turn audit. Its SQL aggregates server-side and never serializes prompt text, untrusted model JSON, tool results, file paths, or credentials into the UI.
- The desktop `lu_tool_turn_history(chatId)` endpoint requires an existing chat, opens the persisted Reader/Workspace audit ledgers only when their files exist, never fabricates evidence, and returns at most 40 content-free summaries. Incorrect chat IDs and malformed/nonregular stores fail closed.
- A **Tool activity** pane in the normal composer shows Reader/Workspace task history across restart, with status `COMPLETED`, `FAILED`, `REJECTED`, or `INCOMPLETE_OR_INTERRUPTED`. Recorded model results are **not** current filesystem proof or grants. The existing Wave 2 File operations pane remains the only display of write-effect reconciliation.

## WAVE3-CONT-02: live native tool activity and cancellation
- The core WorkspaceReadTurnSession now exposes the existing trusted native dispatch observer to its caller, without duplicating the model-tool loop. Tauri emits only three fixed statuses: `model_step`, `tool_started`, `tool_returned`; no tool arguments, paths, source bytes, capability, or untrusted model output enter those events.
- Normal Send's streaming bubble labels the exact current generation with **Lu · thinking with tools**, **Lu · using approved native tool**, or **Lu · checking tool result**. A different conversation/generation cannot overwrite the visible state. The activity is cleared on terminal generation events; no fake byte count is displayed.
- Existing Lu cancel uses the host-owned current-step registry to cancel the backend active step and the native turn cancellation token, followed by durable assistant cancellation and task-grant revocation. Terminated or uncertain effects cannot be automatically retried.

## WAVE3-GUARD-01: progress-based model/tool loop protection
- Removed the old blanket eight-tool-call limit. A bounded **32-tool absolute emergency ceiling** remains to protect local resources; this is not the normal stopping criterion.
- After each completed native tool response, the host hashes a canonical tool-name/argument request and actual typed result **excluding** host-generated call IDs. If the identical request and identical result repeat three times consecutively, another repetition is denied **before dispatch** as `NoProgress`. A changed request or changed result resets the counter.
- An identical write-effect request is blocked on its very next attempt; repeated model text cannot confer a fresh approval or replay a write. The model does not receive raw digest witnesses and cannot change the detection criteria.
- The supervised controller records a sanitized loop-guard failure in the durable model-turn audit, halts the turn, never regenerates JSON on guard failure, and preserves the existing effect journal as the sole evidence of any prior mutations.

## WAVE3-DISCOVERY-01: registered, authorized, typed capability discovery
- New Lu Core `native_capability_inventory` derives available native tool names, effect types, and input/output Protobuf type URLs from the *live registry* and matching model catalog. It intersects with the authenticated `ConversationAuthority` permitted IDs, revalidates model schema presence and rejects missing codecs/effect escalation.
- WorkspaceReadTurnSession projects exactly the currently granted **read-only** capability subset; no global tool list is treated as permission. Tauri emits this host-derived inventory only after a real user turn, native-picked directory, valid grant and safe schema registration.
- Normal Send displays the actual read-only model tool names for the running generation and removes the grant indicator when that generation ends. A reader in another chat cannot inherit these entries. The model still gets full typed tool schemas via the pre-existing Lu Core supervisor rather than UI strings.

## WAVE3-ROUTING-01: natural-language workspace discovery in ordinary Send
- A narrowly classified user request such as **List files in my project folder** can now offer the existing native folder chooser during ordinary Send, without a toolbar action. No model-driven path/permission is accepted; canceling the picker grants nothing. The one-shot manual toggle remains available for other read-only requests.
- The trusted host rejects generic explanation questions, explicit negations, write/move/delete language, oversized requests, model-control tokens, and attached-text envelopes. Only the user's explicit current request can drive this conservative preflight; imported file contents are data, never routing authority.
- Existing normal Send, images and attachments continue unchanged when no read-workspace route is admitted. Existing file-creation hint logic now classifies only the leading human message, not appended untrusted text-file contents.

## WAVE3-READER-01: natural document Reader from ordinary Send
- Clear current-user reading requests (e.g. **Summarize this PDF**, **Inspect the Excel spreadsheet**) now offer the existing native document picker directly from normal Send. They use the exact preexisting authenticated Reader turn, pinned/source-hashed document bytes, bounded Protobuf model tool, revocation, per-chat cancellation and real model-turn journal.
- The previously separate Ask Reader command is a wrapper of the same host entrypoint, not a duplicate backend. Explanatory prompts, writes, negations, folders, attachments and injected model-control tokens cannot trigger this route. Users can deny the native picker without granting anything.
- Document tool selection still requires the genuine human-picked file; neither a model answer nor the WebView supplies an OS path or opens arbitrary files.
