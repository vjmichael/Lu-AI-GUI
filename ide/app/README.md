# Lu IDE standalone desktop alpha

This is an independent Tauri 2 + React application. It does **not** use Lu Desktop's window, load the Lu runtime, or link against Lu Core. Registering a project via the native folder chooser only records the selected directory as host-side metadata.

## Run locally

Prerequisites: Rust 1.90+ for the Tauri desktop (Rust 1.88+ for the independent host crate), Node 22+, platform-specific [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/).

From `ide/app`:

```sh
npm install
npm run check
cargo run --manifest-path src-tauri/Cargo.toml
```

The desktop application is registered as `org.lu.ide` and stores its project catalog in its own OS app-data directory. It has no dependency on the `lu-desktop` executable or its model files.

The app currently offers project selection, close, reopen, registration removal (which **never deletes on-disk folders**), a project explorer, and a multi-tab **CodeMirror editor** for UTF-8 source. In Projects, click **Edit** on an active project, browse a folder, open source tabs, make human-owned edits, and press **Ctrl+S** or **Save**. Saves are size-bounded, capability-relative and revision-checked; if a file changed outside the IDE, your draft remains unsaved instead of silently overwriting the disk version. The editor maintains per-tab undo histories.

**R3.3 candidate:** **Review changes** produces a bounded, read-only change summary before an optional separate **Save reviewed changes** action. Truncated reviews cannot be saved from the review dialog. This is a foundation for future Lu coding proposals; it is **not an active Lu bridge** or an AI-generated patch. See [change-review limitations and tests](../docs/IDE_07_PROPOSAL_REVIEW_FOUNDATION.md).

Package manager, terminal, debugger, advanced Git, AI coding connection, multi-file agent plans and an active Lu connection are **not yet implemented**. These features follow the eight-stage plan in [the IDE design](../docs/IDE_WORKSPACE_DESIGN.md).

## Security

New projects are **never executed** on open. The WebView supplies no path to the registration command; it invokes an OS-owned directory picker. The native host stores project roots privately and sends only opaque IDs, display names, and availability state to the project UI. Explorer commands accept validated relative paths and use a capability-relative directory handle; browsing is bounded to 400 entries per folder and 512 KiB per source file.

The separate `ide/sandbox` crate has passed limited Linux isolation probes, but **no executable worker or OS sandbox is attached to the desktop yet**. This build deliberately has **no project code execution command**.

## Verification

- Linux: `cargo test --manifest-path ../host/Cargo.toml --all-targets`
- Windows: `cargo test --manifest-path ../host/Cargo.toml --all-targets` and `cargo check --manifest-path src-tauri/Cargo.toml`
- UI: `npm run check`
- GitHub: `.github/workflows/ide-standalone.yml`

The user's previous Windows R3.2 launch/edit/remove smoke tests passed; R3.3 change-review interaction and remaining adversarial save/security gates require new revision-specific Windows acceptance.
