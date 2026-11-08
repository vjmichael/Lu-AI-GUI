# P05 B3: independent bounded saved-project search port

Original source: draft PR #46 `ide/project-search-20261105`. This port intentionally uses the alternative path `project_search_port.rs`, preserving P05 exact historical source audit and retention of all original PRs.

Host-side implementation has no shell, network, external model, indexing daemon, or writes. Only currently registered/open project capabilities can be traversed; directories, bytes, scanned files, depth, matches and regex memory are bounded. Literal search results return pinned disk SHA and UTF-8/UTF-16 hit coordinates; generated directories, binary files and oversized files are skipped. This stage integrates original native Rust unit tests and explicit one-version Cargo.lock graph. Tauri/E2 UI wiring, native OS and security tests remain pending until exercised. This is not protected-execution or signed-release acceptance.

## Tauri and E2 desktop integration

The Rust module is now connected to a single Tauri `ide_find_in_project` handler on a currently-open project capability. The existing E2 frontend exposes a distinct **Find in project** pane with bounded literal text queries, case sensitivity, incomplete-results warning and original saved SHA-256 results. Clicking a hit rechecks that disk SHA through the existing native preview, refuses stale coordinates and dirty tabs, and opens an independently session-identified CodeMirror tab otherwise. Original repository paths PR #46 remain omitted under P05's historical provenance audit. Tests and real installed GUI acceptance are still pending; native worker permissions, run/test/debug admission, historical CI, signing/legal release remain unchanged.
