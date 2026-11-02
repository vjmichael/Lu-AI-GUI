# Lu R16 — Dedicated GGUF directories and Jan-free working-tree extraction

Date: 2026-11-01 (America/Phoenix)
Release status: development change, not alpha-accepted.

## User direction

1. Each imported model gets its own folder.
2. Loading progress UI may be deferred.
3. The physical Windows load benchmark is deferred while LM Studio was using Flint; do not mark Vulkan offload accepted.
4. Update GitHub and isolate Lu from the inherited Jan application.

## Implemented file organization

New imports use a host-generated opaque model ID as the folder name:

```text
%APPDATA%/org.lu.desktop/lu-models/
  m-00112233445566778899aabbccddeeff/
    Qwen3.8-27B-Q4_K_M.gguf
    mmproj-for-m-00112233445566778899aabbccddeeff.gguf
  # Earlier flat-layout files remain supported:
  previously-imported.gguf
  mmproj-for-m-<legacy-id>.gguf
```

Model IDs prevent collisions between identically named GGUFs. The catalog keeps a validated `model_subdir` field, which is absent for existing flat-layout entries. Folder names must match the host-minted model ID exactly. The Tauri host uses the existing bounded atomic Lu Core importer inside the new directory and admits the projector in that same directory. The model registry is not a list of files to auto-start: discovery never starts a worker or loads weights.

The directory scanner recognizes legacy flat files and first-level `m-<32 hex>` directories. Symlinked/reparse-point directories are rejected, and projectors never become independent models. There is no automatic migration or movement of existing multi-gigabyte files. Explicit migration can be planned later with a rollback strategy.

## Verification performed

- Rust formatted source and independent Tauri Windows-target host `cargo check`: passed.
- Windows-target test compilation (`cargo check --tests`): passed, **tests were not run under Windows**.
- Native Linux Lu Core importer tests: 3 passed, 0 failed.
- New host regression tests added for folder discovery, co-located GGUF/MMProj import, legacy catalog compatibility, and rejection of mismatched/traversal directories. They compiled for Windows, but have not been executed on physical Windows.
- Loading progress UI deferred; no new installer should be marked alpha.

## Clean branch vs new repository

A clean-tree branch in `vjmichael/jan_alt` contains only the independent `lu-desktop/`, `src-tauri/lu-core/`, `src-tauri/lu-win-worker-job/`, and provenance/docs, without Jan application files. Its commit *history* remains part of the Jan fork, however; this is a staging tree for a new `vjmichael/lu` repository. The current GitHub connector can modify trees/commits/branches, but cannot create a new repository. New-repository creation or an authenticated import requires a suitable GitHub write capability or owner action. Do not rewrite or delete Jan's reference repository.
