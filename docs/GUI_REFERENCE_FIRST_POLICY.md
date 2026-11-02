# Lu desktop — GUI reference-first implementation policy

**Status:** Engineering directive, adopted after R18.  
**Canonical repository:** `vjmichael/Lu-AI-GUI`.  
**Goal:** Do not reinvent mature desktop chat interaction patterns. Inspect existing Jan source and LM Studio user workflows before implementing another basic GUI control.

## Sources and permitted use

1. **Jan reference implementation:** `vjmichael/jan_alt`, especially `web-app/src/`. Read its relevant code, tests, and interaction contracts; borrow good ideas and, when appropriate, adapt narrowly scoped components only after checking attribution/license compatibility. Do **not** restore Jan's desktop bootstrap, legacy execution paths, plugin architecture, Cowork, or global mutable runtime state. Lu Core remains responsible for model admission, native tool authorization, memory and IPC.
2. **LM Studio user-experience reference:** https://lmstudio.ai/docs/app/basics/chat, https://lmstudio.ai/docs/app/basics, https://lmstudio.ai/docs/app/basics/rag. Study its visible user flows and user-tested affordances. Do **not** copy proprietary source, artwork, branding, or trade dress. Its chat shortcuts, quick model picker and optional explicit load parameters, folder navigation, model ejection, and document workflow are useful reference behaviors.
3. **Lu's physical Windows test screenshots and user decisions** outrank superficial resemblance. Preserve the independent Lu identity and its local/offline-first, no-surprise-execution design.

## Specific Jan implementation reference map

| Lu feature | Jan reference | What to carry forward |
|---|---|---|
| Chat streaming scroll | `web-app/src/hooks/useAutoScroll.ts` | Follow transcript only while user is near bottom; preserve reading position when they scroll up, provide jump-to-latest |
| Per-chat attachment drafts | `web-app/src/hooks/useChatAttachments.ts` | Attachments keyed by conversation; retain unsent draft on chat switches; clear only on explicit removal or accepted send |
| Chat folders/history | `web-app/src/hooks/useThreadManagement.ts`, `useThreads.ts` | Rename, favorite, move, delete safely; durable folder metadata, keyboard shortcuts |
| Model loading progress | `web-app/src/components/PromptProgress.tsx`, `web-app/src/hooks/useModelLoad.ts` | Loading, ready, failed and canceled are distinct states; true percentage only when backend supplies measurable progress |
| File input and model capabilities | `web-app/src/hooks/useAttachments.ts`, `useImageAttachmentConfig.ts`, `useModelSources.ts` | Clear file admission rules and capability feedback; no silent image/text downgrade |
| Input size and keyboard behavior | `web-app/src/components/ui/`, `web-app/src/hooks/useHotkeys.ts` | Familiar, accessible composer focus, Enter/Shift+Enter, pasted-image and drag/drop behavior |

Links: https://github.com/vjmichael/jan_alt/tree/main/web-app/src/hooks and https://github.com/vjmichael/jan_alt/tree/main/web-app/src/components

## LM Studio interaction checklist

- Chat-list sidebar, New Chat, search, folders and drag-to-folder behavior; do not make basic navigation modal or blocking.
- Fast model menu from chat header, with explicit **Load** / **Unload** and clear resident-memory state. Merely discovering a GGUF never triggers loading. Settings holds full hardware and context options.
- Document admission for text, PDF and DOCX with transparent processing and size/context limits, prioritizing local parsing and provenance. A selected file is not automatically sent to a model before explicit submission.
- Prompt composer that supports ordinary paste, image paste, drag/drop, a responsive resize handle and keyboard shortcuts.
- Streaming responses retain a per-conversation identity; switching chats does not cross-route tokens. One loaded backend may queue requests rather than parallelize them.
- Sensible feedback for progress, failures, cancellation, loaded/unloaded model, and interrupted messages. Do not erase already emitted text on stop.
- Optional advanced split-view and nested folders **deferred** unless proven important to the alpha.

## Before building a new basic GUI feature

1. Check Lu's current implementation and regression tests.
2. Inspect Jan's narrowly relevant implementation and the actual LM Studio user flow.
3. Record a behavior-level acceptance test first; adapt the smallest supported behavior, not another product's whole architecture.
4. Integrate through Lu-owned typed APIs; no direct WebView file-system/executable grants, no hidden model loads, and no bypass around conversation-bound authorization.
5. Verify Windows WebView2 semantics, keyboard accessibility, drag/drop, cancellation, persistence, and restart. Distinguish source tests from physical Windows acceptance.
6. Document only non-obvious deviations from established GUI patterns; prefer predictable controls over inventive interactions.

## Scope and priority

This policy is **not** a request to redesign the entire desktop or block R18 alpha gates. Use it during targeted R19+ GUI work and when fixing concrete user-reported behavior. Keep GitHub as source repository and milestone record, not development hub. Reuse already supplied dependencies.
