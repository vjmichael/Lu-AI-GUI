LU INDEPENDENT DESKTOP R15 — DEVELOPMENT EVALUATION (NOT ALPHA)
==============================================================
This installer is for Windows acceptance testing only. It preserves R14's
original flat chats.jsonl file while migrating history to separate journals.

R15 UI: New Chat, Favorites, single-level Folders, Recent Chats, compact quick
GGUF picker, Models/Import/Attach MMProj under Settings, bright loaded indicator,
resizable/collapsible composer. Models in Lu's models folder are discovered on
startup without starting inference. Use the Settings Rescan action for newly
copied GGUF files; no model is automatically loaded on startup or selected by
autocomplete. Explicitly press Load selected from the header or Settings.

TEST STEPS
1. Close R14, install R15 separately, launch Lu.
2. Verify that all messages from the prior R14 flat log are visible in a
   "Previous Lu conversation" entry. Original chats.jsonl is preserved.
3. Create two new chats, add text turns, favorite one and move one into a
   folder. Restart Lu; verify both histories, folder placement and favorite.
4. Place a GGUF in Lu's managed models directory while stopped, then restart.
   It must be listed but not loaded. Load a chosen GGUF explicitly via the
   compact header dropdown. Verify normal text response and unload/reload.
5. Settings > attach MMProj to compatible admitted GGUF; load and ask an
   image-dependent question. Check that there is no invalid image-reference
   warning on successful image cleanup.
6. Cancel a streaming answer; no partial assistant response or alarming red
   cancellation error should persist after restart.
7. Verify the composer drag handle and collapse/expand; no runtime banner
   should appear in the chat header.

LIMITATIONS
- R15 is NOT ALPHA and has not been executed on physical Windows by the build
  environment; actual AMD Vulkan layer offload is unverified.
- Image bytes are host-pinned only during a turn; image attachments and native
  tool execution are not yet fully persisted/wired to the independent desktop.
- This is a development-only cross-build with a real embedded ComCtl32 v6
  RT_MANIFEST, but the complete Windows icon/version resource build remains
  pending genuine Windows resource compiler acceptance.
- Folder nesting and drag-and-drop were deliberately deferred; one folder
  level and native file pickers work in this iteration.
- Lu never deletes the user's AppData on uninstall. Backup existing data before
  a new evaluation build if it is important.
