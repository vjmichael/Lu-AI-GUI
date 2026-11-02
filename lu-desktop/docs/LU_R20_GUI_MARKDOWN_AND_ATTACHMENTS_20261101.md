# Lu R20 GUI — Markdown, copyable code and attachment drafts

**Repository:** `vjmichael/Lu-AI-GUI` (standalone Lu; no Jan runtime path).  
**Scope:** Independent React/Tauri desktop GUI only.  
**Status:** Developer evaluation pending Windows WebView2 acceptance.

## Reference-first approach

The workflow follows familiar desktop assistant interaction patterns and the
behavioral reference map in `docs/GUI_REFERENCE_FIRST_POLICY.md`. Jan's relevant
references include its `code-block.tsx` component, `useChatAttachments.ts` and
`MarkdownTable.tsx`. These were read as workflow references; this R20 code is a
small new implementation with no Jan imports or dependencies. LM Studio's user
workflow is also a reference for explicit attachment admission.

## Implemented

- Typed, dependency-free Markdown parser (`markdown_view.mjs`) produces only
  paragraph, heading, ordered/unordered list, quote, table, rule, fenced-code,
  inline-code, strong/emphasis and safe HTTP(S) link nodes.
- React renderer (`markdown_view.jsx`) escapes text by construction; raw HTML
  never enters `dangerouslySetInnerHTML` and is displayed inertly. File, data,
  JavaScript, privileged WebView URLs and credential-bearing HTTP URLs are not
  clickable. Model output with unclosed streaming fences still renders.
- Fenced code blocks offer a `Copy` button; tables are horizontally scrollable.
- `<think>` sections continue to use the existing typed reasoning split, but
  now render their contained Markdown; a user-authored `<think>` string remains
  literal input rather than becoming a reasoning section.
- Attached text files remain separate from the editable composer text, showing
  removable name-and-size chips. Their contents are appended only when Send
  constructs a bounded, explicit model prompt. Up to 4 text files, with total
  prompt + text payload limited to 16,384 characters. Existing host GGUF/image
  admission remains unchanged.
- Image files admitted by the native host show removable local previews via
  object URLs; URLs are revoked when removed, sent or the app is unmounted.
  CSP permits `blob:` only for `img-src`, not script execution.
- An asynchronous file read is bound to the originating chat's in-memory draft.
  Switching to another chat before the read resolves does not move the file.

## Limitations intentionally left visible

- Full CommonMark/GFM parity, complex nested lists, rich diagrams, syntax
  highlighting and advanced footnotes are not claimed. The parser covers a
  safe, testable subset and never treats Markdown as HTML.
- `.txt`, `.md`, `.csv`, `.json`, `.log`, and source text are supported as
  plaintext attachments; PDF/DOCX parsing is still not implemented.
- Chat drafts are in-memory per conversation; unsent drafts are not durable
  across application restarts. Native model/tool authorization was not changed.
- Current prompt journal records the combined submitted text, not structured
  attachment provenance; richer journal metadata is follow-up work.
- The physical Windows clipboard, image paste/drop, code copy and real
  multimodal model operation are not certified by Linux/frontend tests.

## Local acceptance and evidence

- `npm run build`: esbuild React UI builds offline.
- `npm run check`: 28/28 source and bundle invariants (subject to fresh run).
- `npm run test:ui`: unit tests for Markdown security, incomplete streaming
  output, bounded drafts, queue compatibility and navigation.
- Node SSR security smoke uses compiled React renderer to verify literal HTML
  escaping and expected headings/code markup.

**Windows test sequence:** Install a separately labelled development build,
load your existing model manually, ask for a code example, verify code Copy and
rendered Markdown; paste a PNG, remove it, switch chats with a text-file import
pending, return to the originating chat, send, and verify the model receives the
file exactly once. Test a visible empty/malformed Markdown fence mid-stream.
The loaded model/AMD Vulkan and Lu native-tool alpha gates remain independent.
