LU R17 INDEPENDENT DESKTOP — DEVELOPMENT EVALUATION (NOT ALPHA)
==============================================================
Windows x64 experimental app. This installer does not certify physical AMD
Vulkan offload, Windows drag/drop, or MMProj vision acceptance. Do not treat
this build as the approved Windows alpha.

R17 CHANGES
- Prompt appears in the transcript immediately; composer clears on send.
- While Lu generates, draft your next prompt or open another chat.
- Cancel preserves generated text with a stopped/interrupted status.
- Model prompt includes bounded history of the current conversation, excluding
  canceled assistant fragments from authoritative context.
- More distinct user bubbles, flexible transcript width, collapsible model
  <think> blocks when emitted, and quick header model unload.
- Click Attach, paste an image, or drop PNG/JPEG and plaintext files on the
  composer. Plaintext formats: txt, md, json, csv, log, py, rs, js, jsx, ts,
  tsx. PDF/DOCX parsing is not implemented; other binaries are rejected.
- Windows WebView HTML5 drag/drop enabled (native Tauri interception disabled).

TESTING
1. Back up %APPDATA%\org.lu.desktop if your Lu chat history matters.
2. Close previous Lu builds before installing R17 to avoid shared AppData
   contention. R17 installs under its own Program Files path.
3. Verify imports, separate conversations, native text generation and quick
   model unload.
4. Enter a first prompt; confirm that it appears immediately and the composer
   clears. Draft another while Lu is generating.
5. Switch to another chat during generation; verify tokens do not appear in
   the wrong chat. Return to the original chat when generation completes.
6. Start a longer response, cancel it, and verify partial text remains with
   'stopped' status after restart.
7. Ask about Arkansas cost of living, cancel the answer, then ask the cost of
   living again in the same chat. Verify the model remembers Arkansas.
8. Paste a screenshot or drop a PNG/JPEG with a compatible MMProj attached;
   try dropping a .txt/.md file. Record UI and backend results.
9. Verify native Windows AMD Vulkan *layer offload* in logs; seeing a Vulkan
   device alone is not sufficient.

LIMITATIONS
- Development-only Windows cross-build with real embedded ComCtl32 v6
  RT_MANIFEST, but the complete production icon/version resource path has not
  passed real Windows toolchain acceptance.
- Native tool routing and full ANSF memory integration in the independent
  desktop remain alpha gates.
- Uninstall leaves user-owned chats and GGUF assets in AppData.
- Model load progress remains a deferred enhancement.
