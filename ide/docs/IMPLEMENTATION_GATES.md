# Lu IDE Engineering Gates — 2026-11-04

Status: design only, no claim of integrated implementation or sandbox verification.

| Gate | Scope | Acceptance |
| --- | --- | --- |
| IDE-01B | Rust workspace registry and Protobuf boundary | Create/open/close/reopen project; host-minted handles; no JS arbitrary path; authentic user origin; Linux tests |
| IDE-02 | Editor + project explorer | Multi-tab split editor, undo/redo, search, file tree, diff preview, reliable save/recovery |
| IDE-03 | LSP + diagnostics | Python/Rust/C/C++/TS server adapters, lifecycle, no auto-exec untrusted, cancellation |
| IDE-04 | Git advanced | Worktrees, staging, rebase/conflict repair, branch/review and crash-safe operations |
| IDE-05 | Package master manager | uv, vcpkg/Conan, Cargo, JS, toolchains; detection, preview, explicit approval, verification |
| IDE-06 | Build/test/debug + DAP | CMake/Cargo/uv etc., task log streaming, debugger attach policies, clean termination |
| IDE-07 | Sandbox boundary | Real Linux/Windows hostile project, package hook, LSP, debug, network and file escape tests |
| IDE-08 | Lu coding agent | Reviewable diffs, plan checkpoints, per-turn grants, verified tests, durable recovery |
| IDE-09 | Self-development | Disposable Lu Dev process/worktree, build/test, merge proposal, rollback; live core untouched |
| IDE-10 | Release acceptance | Installer, update/migration, performance, security, native Linux+Windows E2E |

Sequence: IDE-01B -> IDE-07 minimal execution boundary -> IDE-02/03 -> IDE-05/06 -> IDE-04 -> IDE-08/09 -> IDE-10. Never ship package/build execution before basic containment is independently verified.
