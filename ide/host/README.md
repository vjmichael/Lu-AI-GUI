# Lu IDE Workspace host — IDE-01B candidate

This Rust crate composes **Lu Core**'s existing `WorkspaceHost` and module registry.
It registers `lu.ide` as a workspace with **zero executable tools**. It is not a
new authorization engine, executable worker, Tauri command, or OS sandbox.

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
This is **not** full Task 1 acceptance: no live Tauri picker/verified UI origin,
desktop integration or Windows native tests. The current local container lacks
`cargo` and `rustc`; the verified compile occurred on GitHub's Linux runner.
