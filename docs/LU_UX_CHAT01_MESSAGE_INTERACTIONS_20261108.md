# UX-CHAT01 — Dynamic message history, reactions and appearance

**Date:** November 8, 2026. **Owner design decision:** messaging-app-like controls inside Lu's existing ANSF original-conversation history. **Parent:** AGENT-01E / E1-P15 integration draft PR #156. **Status:** draft native/desktop implementation awaiting exact-head CI and installed-app verification.

## Central design reconciliation

The existing `docs/LU_CENTRAL_DESIGN.md` section 6 already committed to **copy, retry, editing, quoting/transclusion, provenance, visibly distinct roles and theme preferences**. It **did not explicitly include** per-message emoji reactions, independently customizable user/AI message-bubble colors, chat background selection or precise "replay" semantics. Those omissions have now been corrected in the central design.

## Implemented UX

1. **Per-message reactions:** each real visible ANSF message has accessible options 👍, ❤️, 😂, 🎉, 😮, 👀. Exactly one local reaction per message (press the selected one again to remove). Each reaction is keyed by `(chat_id, original ANSF visible seq)`, not an array index or mutable rendered bubble position. Host re-reads the selected original chat message sequence before changing it. Reactions survive restart, pagination and moving between chats.
2. **Copy:** copies the message's displayed original text through the WebView clipboard API and reports clipboard errors. This copies text; it does not generate, execute, attach original files or transfer tool permissions.
3. **Replay:** reopens the previously rendered, immutable text in a read-only dialog. Model reasoning blocks remain collapsible. **Replay NEVER re-generates a model response, reruns the original Git/FFprobe/other tool, resumes an old task, or copies historical user authority.** This UX is not an audio/TTS playback engine. Audio replay and accessible speech support require a separately reviewed provider.
4. **Reuse prompt / quote:** a prior human message may populate the editable composer, and any message may be quoted into a draft. Neither action sends automatically; the next explicit Send creates a fresh current human turn under existing authority checks. No prior tool effects are replayed.
5. **Per-conversation appearance:** four presets (Original, Ocean, Forest, Light), plus native HTML color picker inputs for the conversation background, user's bubble, and Lu's bubble. Colors are stored as strictly validated `#RRGGBB` values. Contrasting foreground colors, visible focus states and Markdown color inheritance support light and dark bubbles. **Custom wallpaper images are deferred** until reviewed local-image admission, asset persistence and performance/a11y testing.

## Actual persistence / trust boundary

- New host module: `lu-desktop/src-tauri/src/chat_message_interactions.rs`. Local SQLite UI-only metadata in `lu-message-interactions.sqlite`, separate from original ANSF immutable chat storage. Bounded 4096 reactions per chat, 64 visible requested sequence IDs per read, 32 MiB bounded DB, safe regular-file and WAL/SHM sidecar checks. Invalid records are errors, not plausible fake reactions.
- New registered Tauri commands: `lu_message_decorations`, `lu_set_message_reaction`, `lu_set_chat_appearance`. All accept an existing selected chat and regular scalar properties, never arbitrary SQL, CSS, filesystem path, scripts, URL or model-provided capability tokens. An invalid or another-chat sequence is refused by an indexed original-history lookup. Read calls do not create missing metadata DBs.
- New frontend helper `message_interactions.mjs` validates palettes and maps genuine positive sequence IDs to whitelisted emoji. No remote image, custom CSS, database query or tool invocation leaves the WebView.
- Only settled original entries get actions. A live streaming placeholder has no durable message ID and cannot be reacted to as if finalized. Per-chat decoration reads load with the 64-entry original visible ANSF window, not with a huge transcript, preserving hot-scroll memory goals.
- Display replay and manual text reuse are not the operational restart/retry mechanism. E1-P15 retains the original uncertainty and verified receipt timeline, untouched.

## Native verification required

`.github/workflows/lu-ux-chat01-message-interactions.yml` requests exact-head REPO-01 source lock, actual Rust module under the *real production ChatStore* portable harness on Windows Server 2025 and Ubuntu 24.04, Node tests for palette and UI source semantics, and a Windows desktop React build and Tauri `cargo check`.

Tests cover persistence across ChatStore restart, original-chat source identity, wrong/unrecorded message rejection, unsupported emoji, invalid CSS/URLs, SQLite symlink refusal, read-only viewing with no new metadata file, palette contrast, distinct roles, copy/replay/quote/prompt reuse UI, and central-design documentation presence.

**These tests do not establish physical Windows installed-app UI acceptance.** Manual release candidate checks must include paginating old conversation pages, emoji reaction persistence, changing chats during active model output, clipboard permission errors, keyboard navigation, color contrast, actual Windows OS appearance, restart and non-replay of reviewed native effects.

## Follow-up

- UX-CHAT02: durable message edit/revision UI with preserved immutable originals, linked quote/citation provenance, optional threaded reply context and richer reaction presentation. Edited text must not rewrite historical tool authorization.
- UX-CHAT03: optional admitted local background wallpapers and text-to-speech playback; do not add an unreviewed speech engine or remote image URL.
- Native and installed-OS test gates remain open; do not promote UX-CHAT01 solely from GitHub source acceptance.
