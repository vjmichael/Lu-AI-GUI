# Lu IDE Workspace host — IDE-01B candidate

This **standalone** Rust crate owns local project registration and does not link
against Lu Core. It is not an authorization engine, process executor, tool host,
or OS sandbox. Future Lu communication is an **optional, separately authenticated bridge**.

## Structure

- `src/lib.rs`: host-only native directory picker trait, persistent bounded
  project catalog, host-minted IDs, register/close/reopen/list operations.
- `src/wire.rs`: canonical, size-bounded Protobuf request/reply codec.
- `../proto/lu/ide/v1/workspace.proto`: UI lifecycle schema with **no paths**.
- `tests/lifecycle.rs`: Linux-friendly unit/integration fixtures.

## Local check

```sh
cargo test --manifest-path ide/host/Cargo.toml --locked
```

A crate lockfile has not yet been generated; for first-time dependency resolution
run `cargo generate-lockfile --manifest-path ide/host/Cargo.toml`, then
`cargo test --manifest-path ide/host/Cargo.toml --locked`.

## Security and integration status

The `NativeDirectoryPicker` must be implemented with a trusted OS picker by
a future Tauri host adapter. Do not deserialize a path from UI or model output.
Persisted catalog paths are **host-only metadata**; opening them provides no
capability to read/write files. Stage 2 must use real OS-backed directory/worker
capabilities, revalidate roots and enforce explicit grants. This component
**does not** currently attach Tauri commands or authenticated conversation
turns, launch processes, run package managers, or claim OS containment.
UI-only project lifecycle messages do not authorize tool dispatch.

**Native Linux CI evidence:** Rust 1.88 compilation, 8/8 tests and committed
rustfmt all passed in [run 41961237114](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41961237114).
See [implementation evidence](../docs/IDE_01B_IMPLEMENTATION_EVIDENCE.md).
This was the earlier native library validation. Standalone Tauri UI and Windows
acceptance are now separate gates; see the current standalone app workflow. The current local container lacks
`cargo` and `rustc`; the verified compile occurred on GitHub's Linux runner.
