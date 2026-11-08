# Lu IDE DEV-01.2 — explicit native project entry creation

**Status:** SOURCE-IMPLEMENTED, exact-head native source CI and actual installed GUI acceptance pending.
**Dependency:** draft [DEV-01.1 quick-open #119](https://github.com/vjmichael/Lu-AI-GUI/pull/119), not integrated into the Lu IDE release candidate.
**Security classification:** local, user-initiated project-scoped native mutation; no shell, AI authority, script execution, code installation, arbitrary host path, or overwrite.

## Delivered

- New `lu_ide_workspace::source_create::IdeWorkspaceRegistry::create_source_entry` creates exactly one **empty UTF-8 source file** or one directory inside a previously native-picker-registered and currently open project. Parent path must exist and be a normal directory; no symlink parent traversal is authorized. Directory handles are capability-relative.
- Names and parent paths pass the existing bounded project-relative validator and a portable guard for Windows aliases, trailing period/space, forbidden device characters, `.git` and `.lu-ide-*` private metadata names. No recursive directory creation. On Linux, scan existing sibling names and refuse case-only aliases so collisions do not become Windows portability surprises. Large sibling scans deny rather than silently skip collisions.
- File creation uses `create_new(true)`, folder creation uses exclusive `create_dir`; neither can overwrite an existing target. The existing host IDE write lock serializes this operation with explicit editor Save. Files are flushed/synced; on Unix, the directory is synced before/after creation. An immediate external writer may still modify project content: there is no claim of cross-process mandatory filesystem locking.
- The Tauri command `ide_create_source_entry` exposes only project ID, **relative parent directory**, one basename and explicit kind, returns a structured receipt, and records a bounded IDE ledger observation. The browser cannot choose an absolute root. The actual native host validates all parameters again.
- Files pane now shows **+ File / + Folder** with editable basename and explicit **Create / Cancel**. A successful create refreshes the real native listing; it does **not** delete an existing editor tab, mutate an unsaved draft, or auto-Save. Errors make no success claim and collisions show a specific refusal.
- New real-host integration tests create, preview and enumerate disposable project files/folders; test existing target, case-only alias, private names, bad paths, closed project, symlink escape rejection on Linux, and racing same-name creates. Frontend contract tests preserve native command origin and editor state. Exact-head workflow tests Windows 2022, Ubuntu 24.04, Rust host, Tauri, frontend npm and immutable lock bytes.

## Native acceptance remaining

1. Inspect Windows2022 and Ubuntu24 exact-head `ide-dev01-native-create-ci.yml` for Rustfmt, Rust host tests, Tauri type-check and frontend tests; fix failures without weakening path/no-clobber assertions. Record actual run IDs.
2. Demonstrate **real installed OS GUI** file/folder creation under a selected project, creation refusal for existing names and external symlink paths, normal save/restart, explicit editor-draft retention, clean uninstall, and exact packaged license notices on both supported OSes. CI-only source compilation does not close that.
3. Design guarded rename, move, trash/restore as **different** native operations: require no-clobber and crash/recovery semantics, check external revisions, no silent directory subtree mutation, and protect open unsaved tabs. Do not implement a generic `fs::rename` without a target-creation race policy.
4. Remaining DEV-01 tabs/splits, checkpointed draft recovery and large-project benchmarks are separate phases. No source-only result is evidence of those features or protected debugger, signing/legal review, P07, or final physical Windows acceptance.

## Evidence distinction

The dedicated workflow explicitly prints `DEV01_REAL_INSTALLED_UI=NOT_ACCEPTED`. No native security policy is expanded, historical PRs/workflows remain open, and Lu Core `main` is untouched.
