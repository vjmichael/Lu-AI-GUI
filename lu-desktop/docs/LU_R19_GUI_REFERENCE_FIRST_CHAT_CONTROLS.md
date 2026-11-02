# Lu R19 — Reference-first chat controls

**Status:** Source-level GUI development; not a Windows alpha release.
**Repository:** `vjmichael/Lu-AI-GUI`
**Source baseline:** R18 (`bd431cc25b1523c815096d64e47b3485838c44f8`) plus the GUI reference-first policy (`f66ebc3d342ab86b6a8b2047d205c58cb7605239`).

## Reference and scope

Use the narrow interaction pattern from Jan `web-app/src/hooks/useAutoScroll.ts`: follow a streaming transcript only if the user was already near the bottom, rather than scrolling unconditionally on every token. Study Jan `useChatAttachments.ts`, `useThreadManagement.ts`, `useHotkeys.ts`, and LM Studio's chat/model UX for future work. This R19 pass uses Lu-owned React state and small pure helpers; **no Jan runtime, Cowork, model loader or plugin framework is imported**. No backend/worker or authorization code is modified.

## Implemented

- **Read-position-preserving transcript:** per-chat scroll position and follow/paused state; streaming tokens no longer yank the transcript down after the user scrolls away. A **Jump to latest** action restores following. This replaces unconditional `scrollIntoView`.
- **Chat search:** sidebar title filter across all known chats (including those in folders); search shows matching conversations without editing persistence or triggering a model load. `Ctrl/Cmd+K` focuses search.
- **Keyboard consistency:** Enter sends; Shift+Enter inserts newline; Enter during IME composition does not send. `Ctrl/Cmd+Shift+N` requests New Chat. Escape closes Settings or the model menu, or exits search.
- **Composer accessibility:** the separator supports keyboard Up/Down resizing, a labeled textarea receives focus after chat navigation, model picker closes after an outside click, and focus-visible indicators are more legible.
- **Regression hygiene:** previous R18 scheduler assertions now tolerate Rustfmt's legitimate line break before `.enqueue` and `.reserve_generation`, without relaxing the queue-order guarantee.

## Evidence from this development environment

- `node scripts/build.mjs` — frontend bundled successfully using the existing offline React 19/esbuild dependencies.
- `node scripts/check-ui.mjs` — **25/25** source/bundle checks pass.
- `node --test tests/*.test.mjs` — **24/24** tests pass (R17 10 + R18 6 + R19 8).
- R19 tests cover near-bottom policy, per-chat scroll restoration, case-insensitive chat title search, Enter/Shift+Enter/IME safety, shortcuts, source integration, and unchanged explicit model-load boundary.
- No Windows installer, Tauri WebView runtime or physical GPU test is claimed by these frontend checks. R18 remains the last installer evaluation baseline until a new Windows package is built and accepted.

## Windows acceptance for R19

1. Load a GGUF, generate a long answer and scroll up while tokens are still arriving. **Expected:** text continues generating but your view stays where you were reading; Jump to latest appears.
2. Click **Jump to latest**; **expected:** follow resumes for the same conversation.
3. Switch between two chats at different scroll positions. **Expected:** each chat restores its own reading position; background queued generation does not steal scroll focus.
4. Search for a title with Ctrl+K, try upper/lower case, and switch to a result. **Expected:** no model loading, no chat deletion and no accidental message submission.
5. Send with Enter, add newlines with Shift+Enter, verify IME composition where applicable, create a new chat with Ctrl+Shift+N, resize the composer using keyboard arrows.
6. Click outside model menu and press Escape in Settings. **Expected:** predictable dismissal without an unintended change to model load state.

## Deferred

- True model-loading percentage widget stays deferred by user request.
- PDF/DOCX and document parsing, advanced attachment metadata/preview, markdown library adoption, complete ANSF integration, and native tools remain separate milestones.
- This pass does not establish cross-platform hotkey behavior or a successful installed-app test.
