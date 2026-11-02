LU R20 INDEPENDENT DESKTOP — DEVELOPMENT EVALUATION (NOT ALPHA)
==============================================================
Windows x64 experimental app. This installer does not certify physical AMD
Vulkan offload, Windows drag/drop, or MMProj vision acceptance. Do not treat
this build as the approved Windows alpha.

R20 CHANGES
- Host-owned FIFO with up to eight accepted jobs across different chats.
- One model invocation runs at once, as required by the current single-session
  inference supervisor. Queued chats are NOT discarded or implicitly canceled.
- Per-chat token buffers, queued/running status, and per-chat Cancel control.
- Canceling a queued chat does not interrupt another chat's active generation.
- One pending generation per conversation until its turn completes.
- Model unload/load blocked while active or queued generations remain.
- Both normal and canceled turns keep their own conversation journals.
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
2. Close previous Lu builds before installing R20 to avoid shared AppData
   contention. R20 installs under its own Program Files path.
3. Verify imports, separate conversations, native text generation and quick
   model unload.
4. Enter a first prompt; confirm that it appears immediately and the composer
   clears. Draft another while Lu is generating.
5. While chat A is generating (do NOT cancel it), switch to chat B and send a
   prompt. Verify B shows Queued while A keeps generating. A should finish,
   then B should begin and complete. Return to A to confirm its answer.
6. Queue a request in chat B and cancel only B while A is running. A must
   continue uninterrupted. Also test canceling A without losing chat B.
7. Send a second prompt in the SAME chat while its first request is active:
   Lu must not submit a second competing generation for that chat.
8. Start a longer response, cancel it, and verify partial text remains with
   'stopped' status after restart.
9. Ask about Arkansas cost of living, cancel the answer, then ask the cost of
   living again in the same chat. Verify the model remembers Arkansas.
10. Paste a screenshot or drop a PNG/JPEG with a compatible MMProj attached;
   try dropping a .txt/.md file. Record UI and backend results.
11. Verify native Windows AMD Vulkan *layer offload* in logs; seeing a Vulkan
   device alone is not sufficient.

LIMITATIONS
- Development-only Windows cross-build with real embedded ComCtl32 v6
  RT_MANIFEST, but the complete production icon/version resource path has not
  passed real Windows toolchain acceptance.
- Native tool routing and full ANSF memory integration in the independent
  desktop remain alpha gates.
- Uninstall leaves user-owned chats and GGUF assets in AppData.
- Model load progress remains a deferred enhancement.

R20 GUI validation targets (development only):
- Markdown headings, lists, tables and fenced code render as React text nodes.
- Raw HTML and non-HTTP(S) links remain inert, without WebView HTML injection.
- Code blocks have copy controls; streamed incomplete fences remain visible.
- Image attachments display temporary local thumbnails; blob: is permitted only
  in the image-src CSP rule, not scripts. Remove or submit revokes previews.
- Text files display removable name/size chips; content is appended to the
  model prompt only after Send. Four files / 16,384 chars maximum total.
- Pending files remain attached to their originating chat across chat switches.
- Markdown/PDF/DOCX parsing is not complete: simple .md text files are sent as
  source text, and PDF/DOCX are explicitly rejected by this development build.
- Windows WebView2 paste/drop, code copy and rendered table behavior require
  physical acceptance before any R20 alpha claim.
