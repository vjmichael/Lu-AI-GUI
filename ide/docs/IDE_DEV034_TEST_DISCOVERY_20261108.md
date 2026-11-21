# DEV-03.4 — bounded native test discovery

## Scope
Implementation branch `ide/dev03-4-bounded-test-discovery-20261108`
is stacked on unaccepted DEV-03.3B PR #160. This feature is deliberately
read-only and does not depend on the native streaming CI being accepted.

The Jobs screen now offers an explicit, native registered-project-only
conventional test-file scan. The native host traverses seven fixed relative
directories using existing capability-bound source directory listing, reads
no file contents, starts no process and never reads project scripts or package
manager declarations. Only conventional extensions and filename patterns
are returned. At most 96 file metadata records are retained. Each file is
`runnable:false`; Rust filenames may indicate an existing fixed Cargo recipe
as a hint, but discovery does not select, approve, execute or certify that
recipe. Non-Rust candidates never imply an approved runtime adapter.

Every result declares the scan incomplete: nested directories, package
workspaces, generated or macro-produced tests, runtime parametrization and
framework plugin collection are deliberately out of scope. A missing test
filename does not mean there are no tests. Unknown project identities,
closed projects, filesystem errors, and symlinks fail closed or remain
unverifiable.

## Verification
- Host native fixture: metadata-only discovery; absent authorization;
  bounded result count and stable sort; closed/unregistered project denied;
  symlink not followed on Unix.
- Frontend hostile contracts: no path traversal, no forged runnable status,
  no project swap or claimed complete discovery.
- Exact-head Windows/Linux native Node, host Rust and Tauri workflow:
  `.github/workflows/ide-dev03-4-discovery-native.yml`.
- Installed GUI, real multi-language contained test execution and comprehensive
  test framework enumeration remain OPEN. These are separate later gates,
  and passing source tests must not be presented as installed acceptance.
