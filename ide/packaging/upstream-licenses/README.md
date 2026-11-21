# Lu IDE — upstream redistributable license texts

These exact complete text copies were retrieved from the upstream GitHub
repositories identified here for use in **test/verification artifact** notices.
Source revisions (not merely license identifiers) are recorded explicitly
in each filename or URL. Tauri's dual-license files are pinned to tauri-v2.12.5; version-matched legal review is still required before release.

- `LLVM-18.1.8-LICENSE.txt`: https://api.github.com/repos/llvm/llvm-project/contents/LICENSE.TXT?ref=llvmorg-18.1.8
- `Python-3.10.11-LICENSE.txt`: https://api.github.com/repos/python/cpython/contents/LICENSE?ref=v3.10.11
- `Tauri-LICENSE-MIT.txt`: https://api.github.com/repos/tauri-apps/tauri/contents/LICENSE-MIT?ref=tauri-v2.12.5
- `Tauri-LICENSE-APACHE-2.0.txt`: https://api.github.com/repos/tauri-apps/tauri/contents/LICENSE-APACHE-2.0?ref=tauri-v2.12.5
- `CodeMirror-6-LICENSE-MIT.txt`: https://api.github.com/repos/codemirror/view/contents/LICENSE?ref=6.0.0
- `esbuild-0.25.9-LICENSE-MIT.txt`: https://api.github.com/repos/evanw/esbuild/contents/LICENSE.md?ref=v0.25.9

These source texts are **not** proof that every compiled transitive dependency,
runtime native DLL, dynamically loaded module, or its attribution is covered.
The package collector builds an actual resolved-dependency inventory and
marks unresolved notices and required legal review explicitly. Any production
release remains blocked until a reviewer clears every unresolved entry and
verifies the exact binaries/packages being shipped.

Every actual unsigned E2 verification submission now fetches all six exact upstream texts by HTTPS and checks exact upstream Git blob and SHA-256 identities (checkout conversion disabled). It bundles a hashed `upstream-fetch.json` proof alongside complete upstream texts and npm/Cargo-resolved notices; any network error, changed upstream text, absent file or receipt mismatch fails closed. The collector and independent auditor bind the receipt to the actual canonical bytes; a consistent receipt is not a cryptographically signed witness of the live HTTPS event. This does not automatically clear transitive licenses or approve release.
