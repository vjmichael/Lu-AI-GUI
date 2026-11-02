# Lu R15 — Conversation Sidebar, Quick Model Selection, and Model Discovery

**Status:** Windows cross-build engineering candidate. NOT alpha-approved.

## User-driven UI decision

- Remove the "Lu Native Runtime" header chip. Retain technical runtime detail
  behind Settings or diagnostic reports instead of persistent chat clutter.
- The sidebar is for New Chat, favorites, folders and conversation history.
  Folder creation is one level; subfolders deferred unless use warrants them.
- Local GGUF/MMProj import, pairing, rescan, load/unload, and technical model
  status live in Settings. A compact quick-model dropdown is in the chat
  header to select an already known model and *explicitly* load it.
- The presence of a GGUF in the managed models directory is inventory only.
  Startup and model selection may preflight file headers and register an ID,
  but MUST NEVER spawn a model engine or load weights. Only a distinct user
  Load action creates an inference session.
- Models may be imported with the native picker or discovered by rescanning
  the top-level Lu-managed models folder. Skip `mmproj*`, invalid GGUF headers,
  symlinks/reparse points, and folders. IDs of discovered models derive from
  canonical filename hashes; imported models retain existing host-minted IDs.
- The current model loading indicator is compact and transient, while the
  loaded indicator in the sidebar footer is brighter green.

## Conversation persistence and migration

The R14 `chats.jsonl` single flat log is unsuitable for folders, favorites and
resumption. R15 introduces host-owned `conversation-index.json` and separate
`conversations/c-<host-id>.jsonl` append journals. It imports a valid R14 log
once on first startup, retaining the original bytes for rollback and not
silently discarding earlier messages. History references are never filesystem
paths supplied from the WebView.

- New Chat creates an independent persistent conversation ID.
- Favorites and single-level folder memberships are saved atomically in a
  bounded, validated index.
- Journal events distinguish user turns, cumulative assistant checkpoints,
  assistant completion, and cancellation. A canceled partial is hidden from
  history; a crash-interrupted partial is preserved and explicitly marked.
- Each event is `sync_data()`-flushed, and generation makes periodic checkpoint
  writes. Full final answer must be durable before the completion event.
- Migration and restore are tested with five executable isolated Rust tests.
- The existing Lu ANSF/SQLite architecture remains the long-term authoritative
  memory model. These host-owned per-chat journals are an R15 desktop bridge,
  not a replacement for ANSF, and should be migrated under its authority.
- Image attachment bytes and native tools remain separate alpha acceptance
  gates. Do not claim this bridge provides full ANSF or multimodal persistence.

## Related real bug fix

Lu Core's image IDs are `img-` + 32 lowercase hex characters, **36** bytes in
ASCII. The R14 desktop cleanup command incorrectly demanded 68 characters,
causing `Response saved, but image cleanup failed: invalid image reference`.
R15 matches 36 and retains strict lowercase-hex validation.

## Tests / acceptance

- Offline React 19 build: `npm run build`.
- R15 source/UI invariants: `npm run check` (17/17).
- Lu Desktop x64 Windows MSVC target: `scripts/check-win.sh --tests`.
- Isolated ChatStore Rust tests with a Cargo manifest pointing to the actual
  `src-tauri/src/chat_store.rs`: 5/5 passed.
- The installed R15 GUI, physical AMD Vulkan offload and user-data migration
  must still be independently tested on Windows. Do not promote until then.
