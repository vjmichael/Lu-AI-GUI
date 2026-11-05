# Lu IDE standalone desktop alpha

This is an independent Tauri 2 + React application. It does **not** use Lu Desktop's window, load the Lu runtime, or link against Lu Core. Registering a project via the native folder chooser only records the selected directory as host-side metadata.

## Run locally

Prerequisites: Rust 1.88+, Node 22+, platform-specific [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/).

From `ide/app`:

```sh
npm install
npm run check
cargo run --manifest-path src-tauri/Cargo.toml
```

The desktop application is registered as `org.lu.ide` and stores its project catalog in its own OS app-data directory. It has no dependency on the `lu-desktop` executable or its model files.

The app currently offers Project selection, status, close, reopen and persistent registration. Package manager, editor, terminal, debugger, advanced Git, AI coding, and an active Lu connection are **not yet implemented**. These features will be developed against the eight-stage plan in `ide/docs/IDE_WORKSPACE_DESIGN.md`.

## Security

New projects are **never executed** on open. The WebView supplies no path to the registration command; it invokes an OS-owned directory picker. The native host stores paths privately and sends only opaque IDs, display names, and availability state to the UI.

No OS sandbox is implemented yet. This build cannot safely run untrusted project scripts because it deliberately has **no execution command**.

## Verification

- Linux: `cargo test --manifest-path ../host/Cargo.toml --all-targets`
- Windows: `cargo test --manifest-path ../host/Cargo.toml --all-targets` and `cargo check --manifest-path src-tauri/Cargo.toml`
- UI: `npm run check`
- GitHub: `.github/workflows/ide-standalone.yml`

An actual installed desktop launch and end-to-end OS dialog test remain separate acceptance gates.
