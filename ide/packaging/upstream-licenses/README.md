# Lu IDE — upstream redistributable license texts

These exact complete text copies were retrieved from the upstream GitHub
repositories identified here for use in **test/verification artifact** notices.
Source revisions (not merely license identifiers) are recorded explicitly
in each filename or URL. Tauri's dual-license files are from the upstream
development branch and require version-matched review before release.

- `LLVM-18.1.8-LICENSE.txt`: https://api.github.com/repos/llvm/llvm-project/contents/LICENSE.TXT?ref=llvmorg-18.1.8
- `Python-3.10.11-LICENSE.txt`: https://api.github.com/repos/python/cpython/contents/LICENSE?ref=v3.10.11
- `Tauri-LICENSE-MIT.txt`: https://api.github.com/repos/tauri-apps/tauri/contents/LICENSE-MIT?ref=2e23492ab9bcec06bab041e0e47447aaa0da298c
- `Tauri-LICENSE-APACHE-2.0.txt`: https://api.github.com/repos/tauri-apps/tauri/contents/LICENSE-APACHE-2.0?ref=2e23492ab9bcec06bab041e0e47447aaa0da298c
- `CodeMirror-6-LICENSE-MIT.txt`: https://api.github.com/repos/codemirror/view/contents/LICENSE?ref=6.0.0
- `esbuild-0.25.9-LICENSE-MIT.txt`: https://api.github.com/repos/evanw/esbuild/contents/LICENSE.md?ref=v0.25.9

These source texts are **not** proof that every compiled transitive dependency,
runtime native DLL, dynamically loaded module, or its attribution is covered.
The package collector builds an actual resolved-dependency inventory and
marks unresolved notices and required legal review explicitly. Any production
release remains blocked until a reviewer clears every unresolved entry and
verifies the exact binaries/packages being shipped.
