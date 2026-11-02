# Lu R20 — Reference-first Markdown and attachment GUI pass

Status: Windows x64 **development evaluation only**. Not alpha-approved.
Canonical GitHub repository: `vjmichael/Lu-AI-GUI`.
Source baseline: R19 main `1a78387ab0ab69f8fc95f1e9ce009404e1114b94`.

## Implemented

1. New restricted Markdown renderer: headings, lists, tables, quotes, code blocks, inline formatting and safe HTTP(S) links. Supports incomplete streaming fences. No `innerHTML`/HTML execution.
2. Code blocks have copy buttons and scroll horizontally rather than widening chat.
3. Image thumbnail previews via local blob URLs with explicit object-URL cleanup. The Tauri CSP grants `blob:` to `img-src` only.
4. Removable text-file metadata chips. Text content is inserted into the model prompt at Send, not into the textarea while drafting. Four text-file cap, bounded by 16,384 combined characters.
5. File read/import results are scoped to the initiating conversation across chat switches.
6. User-authored `<think>` markers stay literal user text; only model output is separated into reasoning sections.

## Validation completed in local Linux/offline environment

- React/esbuild frontend built: `dist/assets/app.js` (about 214 KB).
- Source and built-bundle checks: **28/28 passed**.
- Node executable frontend unit tests: **34/34 passed**.
- React server-side rendering security smoke: escaped raw HTML, no unsafe links; semantic heading/code markup present.
- Windows x64 Tauri GUI executable cross-linked; PE verification confirmed native GUI subsystem, embedded Common Controls v6 manifest and expected TaskDialogIndirect import.
- Windows pinned Vulkan engine manifest and four SHA-256 payload checks passed.
- NSIS development evaluation installer built successfully using the existing offline toolchain. No dependency downloads.

## Not yet accepted

- No physical Windows/WebView2 interaction test for the new Markdown renderer, link/copy behavior, thumbnails, drag/drop, paste, or per-chat attachment switching.
- Native tool/ANSF end-to-end integration, Vulkan AMD device offload and independent GGUF + MMProj physical acceptance remain separately open.
- PDF/DOCX binary parsing, syntax highlighting and full CommonMark/GFM compatibility are not implemented.
- Existing conversation journal stores the expanded submitted text rather than structured attachment metadata; follow-up improvement, not hidden.

## Windows acceptance checklist

- Install R20 independently of older development builds. Load GGUF explicitly, without automatic model load.
- Ask for a Python example: headings, fenced code, and a Copy action must work.
- Ask for an ordered list and simple Markdown table. Scroll sideways inside wide tables and code blocks.
- Ask model to display raw `<img ...>` or `javascript:` links: show inert literal text and never execute script.
- Paste and drag PNG/JPEG files. Confirm preview appears, remove works, and submitted images still reach MMProj.
- Attach two text files in chat A, switch to chat B while another file import finishes, return to chat A: files stay with A.
- Send a prompt with attached text: file content sent once; on subsequent new draft, removed files are absent.
- Retry canceled generation and verify R18 per-chat FIFO, partial-response persistence and scroll behavior remain stable.

Keep the source repository as source/milestone record; use GitHub Actions only when Windows-specific verification is needed. No legacy Jan execution code is imported.
