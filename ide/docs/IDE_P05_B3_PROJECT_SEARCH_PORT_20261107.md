# P05 B3: independent bounded saved-project search port

Original source: draft PR #46 `ide/project-search-20261105`. This port intentionally uses the alternative path `project_search_port.rs`, preserving P05 exact historical source audit and retention of all original PRs.

Host-side implementation has no shell, network, external model, indexing daemon, or writes. Only currently registered/open project capabilities can be traversed; directories, bytes, scanned files, depth, matches and regex memory are bounded. Literal search results return pinned disk SHA and UTF-8/UTF-16 hit coordinates; generated directories, binary files and oversized files are skipped. This stage integrates original native Rust unit tests and explicit one-version Cargo.lock graph. Tauri/E2 UI wiring, native OS and security tests remain pending until exercised. This is not protected-execution or signed-release acceptance.

## Tauri and E2 desktop integration

The Rust module is now connected to a single Tauri `ide_find_in_project` handler on a currently-open project capability. The existing E2 frontend exposes a distinct **Find in project** pane with bounded literal text queries, case sensitivity, incomplete-results warning and original saved SHA-256 results. Clicking a hit rechecks that disk SHA through the existing native preview, refuses stale coordinates and dirty tabs, and opens an independently session-identified CodeMirror tab otherwise. Original repository paths PR #46 remain omitted under P05's historical provenance audit. Tests and real installed GUI acceptance are still pending; native worker permissions, run/test/debug admission, historical CI, signing/legal release remain unchanged.

## Native CI repair: preserve P06 reviewed lock provenance

First B3 build revealed `Cargo.lock` exact SHA was protected by the P06 reviewed graph; adding `regex` as a direct dependency would change that recorded SHA. The final B3 port deliberately **restores the byte-identical P06-approved Cargo.lock and host Cargo.toml** and replaces the old PR's regex-based literal scan with a bounded standard-library Unicode lowercase literal scanner that records original UTF-8 byte offsets and checks max matches. A source regression exercises accented text and emoji, case-sensitive variants and no-match. This is an engineering change, not a legal review waiver. Other exact native test fixes update the typed source capability descriptor, schema, 42-command allowlist, cargo fmt and both mandatory progress ledgers. All native acceptance gates remain open.

## Installed source-revision regression
A separate real Windows NSIS test checks UI project selection and literal saved-source search, modifies the file after rendering a result, expects the UI to refuse the stale SHA, then re-searches and opens a fresh source match. The test verifies disk bytes remain untouched by search/navigation, documents the bound issuer-free notice inventory and uninstalls. This is **not accepted** until exact-head native CI results succeed.


## P05 B3 Windows installed saved-search isolation — November 8 UTC

Real [Windows NSIS/UIA installed run 42437556088](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42437556088) for B3 earlier head `ac00d251` completed actual bundle/license verification (521 packages; 0 unresolved notice texts), install, renderer UIA, source Save/restart, external edit conflict, and native folder selection. Its final **new search fixture** invocation failed on `WO8_NATIVE_UI_DENIED:ui_element_not_exposed_main.rs` after an additional project registration. The failure occurred before the saved-search stale-result action; therefore the search feature itself is **not yet accepted or disproven** by this run.

The B3 installed test now reuses the **same previously registered and already verified original project** instead of creating a fifth independent catalog entry; actual external byte changes, saved SHA conflict denial, fresh search, no unintended disk writes and real browser navigation remain mandatory in `SearchProjectSaved`. The workflow additionally verifies the saved file acquired both externally written markers and changed native disk SHA. No synthetic WebView/native IPC, fixture spoof, release signing waiver, new OS privilege or auto-approval was added. Source contract requires this exact fixture reuse. Dedicated real NSIS Windows run [42443027184](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443027184) and native B3 source matrix [42443027099](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42443027099) were queued on implementation head `db7b14ece254c100a5b14fbcc34d7e445f73dfb3`.

Keep B3 draft and P05 overall OPEN until actual same-head Windows/Linux native source, installed real UI, plus dependent B2 final Save/restart reference acceptance. This is a test-environment isolation improvement, not proof it resolves the main.rs UIA race.
