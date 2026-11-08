# B2 — Unsaved buffer identities and private persistent draft recovery

**Source implementation on new draft PR stacked on B3 #143.**
Native Windows/Linux verification and installed NSIS/Debian editor-workflow acceptance remain **OPEN** until real exact-head jobs pass.

## Two different kinds of source identity

An existing `ide-source:v1` references the last saved file SHA-256.
Original B2 PR #43 `ide-buffer:v1` references a **session-only** CodeMirror editor buffer (random UUID, monotone revision including Undo, exact UTF-16 selection and UTF-8 offsets, base disk SHA, complete-draft SHA). It is invalid after close/reload/restart and grants no filesystem rights.

The new `ide-draft:v1` instead identifies one **immutable, content-hashed durable checkpoint** retained privately by the standalone IDE. It includes project ID, relative path, original editor buffer ID/revision, base saved SHA-256, entire-draft SHA-256 and checkpoint SHA-256, but **never the original source text** in a copied reference. The native host can load that exact checkpoint even after a newer draft is saved or the app restarts. This is not an IDE project file nor an authorization token.

## Durable private storage and explicit consent

Lu IDE's Tauri native host owns its app-data directory and creates `private-draft-recovery/`. All checkpoints are bounded UTF-8 text (512 KiB source, 600 KiB JSON record), privately stored using create-new temporary files, fsync, atomic same-parent rename and on Unix 0700 directories/0600 files. Each registered project/path maps to a SHA-256-named private subdirectory. The vault stores immutable checkpoint bytes and an independent latest-head pointer, protected by CAS on the previously observed head and strictly increasing revision for the same buffer. A late stale writer cannot silently replace the current pointer. At most 24 immutable checkpoints per file are accepted; further persistence fails closed rather than evicting a reference users may have copied.

The user explicitly clicks **Preserve draft + copy durable reference** to opt into private local backups for that tab. Only after this action does the editor checkpoint subsequent unsaved changes using a bounded debounce. A cancelled/failed checkpoint never constitutes an accepted durable recovery. Older immutable references remain loadable unless the human explicitly discards the entire private history. Storage contains plain UTF-8 draft text in the local application data (not encrypted, no remote sync); the editor must disclose this.

After app restart, opening a registered file checks for a matching private checkpoint and offers **Recover preserved draft**. It never silently overwrites the saved source or the current unsaved tab. If the original disk base has changed, the user must separately acknowledge the source drift before recovery into the editor. A pasted `ide-draft:v1` reference can retrieve an older immutable checkpoint by exact hash using **Recover by reference**, with current project/path and returned digest validation. Recovery replaces the editor draft only after explicit human action; **Save** is still the existing optimistic SHA-checked project file write. Users may select **Discard private recovery** to remove the private history only, requiring the exact latest checkpoint hash.

All vault operations first confirm an actively registered source file via the native picked-directory capability. The Tauri bridge exposes four typed host capabilities:
`draft.checkpoint` (private catalog write), `draft.latest` (read), `draft.load` (read), `draft.discard` (private catalog write). None enables unrestricted project-file writes, worker launch, Lu Core/AI connections or external network.

## Native/security acceptance

The original PR #43 CodeMirror revision, Unicode, CRLF, surrogate pair, Undo, Save rebase and stale-source-reference tests were ported to current React+protected-LSP editor without removing its existing features. New unit tests cover durable token parsing/tamper and the Rust vault: native restart, exact historic checkpoint load, CAS stale revision/replaced head, private history deletion leaving project source unchanged, path traversal/oversize denial, immutable record tamper and Unix symlink directory refusal.

Windows/Linux CI: `.github/workflows/ide-b2-durable-drafts-native.yml`. It runs actual host Rust tests, current frontend full build and Node tests and exact locked native Tauri compilation on each OS. Passing source/native tests does **not** equal installed GUI acceptance.

## Genuine installed editor-workflow gates

Separate actual installed Windows unsigned NSIS and Linux Debian/GTK workflows must:
1. Verify exact source and legal notice packages; install a real built binary.
2. Open a real registered fixture through OS-native chooser and CodeMirror.
3. Modify source without Save; use real visible editor buttons to copy session-only and durable references; verify clipboard has IDs/hashes but **no code**.
4. Close/restart the installed GUI and recover private checkpoint from existing app data using real OS UI interactions, without a direct backend/JS shortcut.
5. Verify the original source disk SHA was unchanged throughout unsaved operations; only an explicitly invoked Save can change it.
6. Confirm stale editor/Undo references, external disk changes, exact recovered bytes and cleanup/uninstall wherever actual GUI fixtures permit. Missing GUI/clipboard accessibility is an **open or failing gate**, never a green proxy.

### Scope and status caveats

No signed release, independent redistribution/legal approval, true protected LSP execution or Lu Core integration is claimed. The original B2 PR #43 and its installed Windows PR #45 are preserved; the current-stack B3 port #143 is also preserved. Source staging is **not** physical installed acceptance. Exact native CI and actual OS installed results must be obtained and repaired before closure.
