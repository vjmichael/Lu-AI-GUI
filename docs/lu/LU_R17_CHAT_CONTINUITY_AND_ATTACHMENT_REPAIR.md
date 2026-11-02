# Lu R17 — chat continuity, cancellation, switching and attachment repair

**Status:** Local source implemented; Windows x64 Rust type-check and React bundle pass; physical Windows GUI acceptance **pending**. 2026-11-01 development baseline. Canonical repo: `vjmichael/Lu-AI-GUI`.

## Evidence from physical R15 GUI testing

Observed: submitted prompt hidden until generation completes; later draft text remains in composer while model is generating; switching chat during output disabled; cancel deletes visible assistant output; follow-up loses Arkansas context; no drag/drop or clipboard screenshot support; responses limited to narrow bubbles; no quick unload and no collapsible model-emitted think spans.

## Implemented R17 fixes

1. **Immediate user turn / composer separation:** optimistic user bubble on send, prompt cleared before awaiting model completion. Newly typed draft stays editable while output streams, and a failed submission can restore the prior draft on failure (but not after an intentional cancellation).
2. **Per-conversation streaming routing:** record the chat that initiated generation; stream events show only in that chat. Navigating to New Chat, another conversation, folders or favorites is allowed during output. Terminal event updates the originating chat without stealing focus.
3. **Cancel keeps partial output:** Lu Core output collected so far is journaled before `AssistantCancelled`. The journal materializer retains that message marked interrupted/cancelled. Interrupted assistant output is not treated as an authoritative completed answer on the next turn. User prompt persists.
4. **Conversation context:** `prompt_context::assemble` builds bounded model input from previous turns in the same chat plus the new user message, ignoring interrupted assistant output. Empty-history single prompts retain their original inference text. No cross-chat content is mixed.
5. **Window widths and clarity:** user bubbles remain right-aligned with a distinct brighter blue; Lu bubbles remain left-aligned and can grow across available transcript width. Think blocks are collapsible only when the model outputs `<think>...</think>` explicitly; no hidden model/host reasoning is exposed.
6. **Attachment inputs:** click to attach PNG/JPEG and plaintext (`.txt`, `.md`, `.csv`, `.json`, `.log`, code files), paste a screenshot and drop these types on the composer. On Windows, Tauri native drag/drop interception is disabled to allow HTML5 file drop. Image data is bounded, parsed and admitted by the host via `admit_data_url`, never a model- or WebView-selected filesystem path. The host retains only opaque image references for inference.
7. **Quick unload:** a model eject control is available beside the chat-header selector and in its dropdown. Selection still never causes auto-loading.

## Explicit limitations and follow-up acceptance

- PDF/DOCX parsing, drag-and-drop of other binary document types, and document-index search are **not** included. UI tells the user about unsupported formats instead of inventing content or claiming success.
- Image clipboard and drag/drop must be confirmed in physical Windows WebView2 with a loaded MMProj; a successful frontend bundle or native type-check is not sufficient.
- Chat-history context is a bounded in-turn prompt workaround. Full ANSF/SQLite memory and model-native per-session context/KV cache are separate integration gates.
- Verify the Arkansas cancellation/re-prompt scenario with the real Qwen GGUF to assess semantic continuity; unit tests only prove the relevant turns are included and interrupted output excluded.
- Chat switching during live generation needs physical GUI test for correct stream attribution and no draft contamination.
- Windows AMD Vulkan GPU offload and packaged NSIS installer acceptance remain separate gates; device listing is not evidence of layer offload.
- The model loading-progress widget remains deferred by product decision.

## Local evidence

- `r17-journal-context-tests.log`: **9/9** executable Rust conversation/context tests.
- `r17-ui-check.log`: **24/24** static/source/bundle checks, including the original R15 invariants.
- `r17-ui-tests.log`: **10/10** Node tests for think blocks, stream isolation, drag/drop config and host admission contract.
- `r17-windows-cargo-check.log`: `cargo check --offline --target x86_64-pc-windows-msvc --tests` **PASS** against existing offline Rust/Tauri/SDK dependencies.
- The full native Tauri Linux unit-test run requires unavailable system GLib/WebKit libraries; this is a host-environment limitation, not a Windows build-pass claim.

## Manual Windows verification order

1. Back up Lu app data, install development build, verify model list and manual GGUF/MMProj loading.
2. Send first prompt and confirm it appears in transcript immediately while composer clears.
3. Start a long generation and type next prompt; verify the draft remains editable, then switch to another chat and back without crossing streams.
4. Cancel output and confirm partial text remains visibly marked `stopped`, including after restart.
5. Ask `What is the cost of living in Arkansas?`, cancel, then `How much is the cost of living again?`; confirm explicit Arkansas context is retained by the model.
6. Copy/paste screenshot and drag PNG/JPEG, text/Markdown/JSON into composer; verify MMProj image understanding and file content boundary.
7. Confirm quick model unload, chat persistence and eventual worker process cleanup. Capture Vulkan offload logs separately.
