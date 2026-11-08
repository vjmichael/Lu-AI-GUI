# P05 B3: independent bounded saved-project search port

Original source: draft PR #46 `ide/project-search-20261105`. This port intentionally uses the alternative path `project_search_port.rs`, preserving P05 exact historical source audit and retention of all original PRs.

Host-side implementation has no shell, network, external model, indexing daemon, or writes. Only currently registered/open project capabilities can be traversed; directories, bytes, scanned files, depth, matches and regex memory are bounded. Literal search results return pinned disk SHA and UTF-8/UTF-16 hit coordinates; generated directories, binary files and oversized files are skipped. This stage integrates original native Rust unit tests and explicit one-version Cargo.lock graph. Tauri/E2 UI wiring, native OS and security tests remain pending until exercised. This is not protected-execution or signed-release acceptance.
