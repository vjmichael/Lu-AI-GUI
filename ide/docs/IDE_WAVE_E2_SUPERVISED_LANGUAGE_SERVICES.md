# Lu IDE Wave E2 — Supervised Language Server Integration

**2026-11-06 · Draft/source integration; live OS execution deliberately blocked.**

## Goal and limits

Provide one language-intelligence path to the existing human CodeMirror editor and,
later, independently authenticated Lu coding assistance. Reuse off-the-shelf LSP
servers instead of building language parsers in an LLM. Servers are local
**executables**, potentially able to execute project scripts, plugins or other
programs. Merely finding rust-analyzer, clangd, Pyright, Ruff or TypeScript in
PATH is neither trustworthy provenance nor permission to run it.

**This branch adds protocol, guarded transport and editor presentation source,
not an activated language-server process.** Actual native Windows and Linux
launcher integration and installed-app acceptance remain separate open gates.
Wave D7D restricted LocalService OS worker acceptance was queued and D6's
zero-network AppContainer denial is not yet proven. There is no admissible
reason to introduce a direct desktop or shell spawn to bypass those gates.

**Independent protocol evidence (2026-11-06):** Clangd 17.0.0 was launched only as a controlled local disposable test fixture, never through Lu IDE or the protected Windows service. Actual initialize, diagnostics, hover, same-file definition and completion replies were observed. Its completion `textEdit` replaces exactly an identifier prefix; the parser now validates exact same-line cursor-bound ranges, refuses additional edits, commands and snippets, and leaves insertion to the human editor. The sanitized transcript is committed as `ide/host/fixtures/clangd17_real_wire.json` and used by Rust unit tests. This is real LSP interoperability evidence, **not** an OS-security or installed-app pass.

## Implemented source

| Component | Current behavior | Limit |
| --- | --- | --- |
| \`host/lsp_session.rs\` | Bounded initialize/initialized, didOpen/didChange/didClose, request correlation, completion, hover, same-file definition, version-checked diagnostics, safe workspace/configuration response | In-memory protocol processor, no process or runtime pipe |
| \`supervisor/lsp_transport.rs\` | Internal \`lsp-transport\` feature; running D3 job and matching plan hash; complete OS-proof contract; kernel tree cleanup interface; bounded reply/send | Native Windows Job+service and Linux cgroup+bwrap adapters **not yet wired** |
| \`app/web/lsp_editor.js\` | Revision checks, filename→language mapping, UTF-16 positions, bounded safe presentations, no executable/server edit handling | Native adapter must return host-authenticated project/file identity |
| \`app/web/code_editor.jsx\` | CodeMirror diagnostic underlines, cursor reporting, source-bound definition selection, human-approved plain completion insertion | Only rendered for authenticated, matching live reply |
| \`app/web/main.jsx\` | Language Services catalog; status from trusted native Tauri command; completion/hover/definition/problems actions, panels | Buttons are disabled until the protected worker reports verified readiness; no mock result used |
| \`app/src-tauri/src/lib.rs\` | \`ide_lsp_worker_status\` explicitly reports \`LSP_PROTECTED_WORKER_UNVERIFIED\` | No \`ide_lsp_feature\` or \`ide_lsp_read_diagnostics\` native runtime command exists yet |
| \`packaging/language-servers.manifest.json\` | Native rust-analyzer, clangd, Ruff candidates; Pyright/TypeScript need audited Node/TypeScript dependency bundles | All assets unpinned/unbundled, \`releaseApproved=false\` |
| \`packaging/verify_language_servers.py\` | Refuses release without pinned SHA-256 per platform, copyright review, license notices, audited dependencies and reviewed approval | No server binaries are acquired, copied or distributed by this branch |

## Next activation: E2B — native OS-restricted process sessions

1. **Windows:** Add a fixed-language-server recipe family to D3 only after independent
   approval, stage a signed/pinned executable in the D7C rooted AppContainer tree,
   launch via \`PROC_THREAD_ATTRIBUTE_JOB_LIST\` with zero network capabilities,
   restricted token, quota limits, standard I/O handle allowlist, Job ownership
   held by the D7D restricted LocalService service, pipe authentication and
   fsynced durable cleanup. No fallback to unconstrained CreateProcess.
2. **Linux:** Reuse the D5 delegated cgroup-v2, Bubblewrap namespaces,
   pre-exec cgroup placement, read-only staged workspace, minimal mounts
   and no network namespace. Require verified cgroup kill and no descendant
   survival after cancel, editor close, host crash or IDE restart.
3. Use a host-only per-project session register, supervised worker job ID,
   pinned installed hash, source object ID and per-buffer revision. User chooses
   trusted language server and authorizes first launch. No automatic install
   or code/script execution on project open.
4. Carry real LSP stdio frames through the protected worker. Reject server
   workspace edits, arbitrary commands, external URIs, malicious markdown,
   oversized output, untrusted server-initiated requests, stale/unsaved buffer
   versions, non-UTF16 boundary coordinates and workspace escape attempts.
5. Implement the \`ide_lsp_feature\` and \`ide_lsp_read_diagnostics\` commands
   *only when* the native receiver verifies session/project/revision/worker
   authorization. Enable the buttons after the OS proof and server handshake.
6. Real-Windows and delegated-Linux E2 fixtures: initialize rust-analyzer,
   clangd, Ruff, Pyright and TypeScript; open/edit/close source; request
   completion, hover, definition and diagnostics, prove current revisions,
   force worker crash and parent crash; check service cleanup, journal replay,
   secret denial and network denial.
7. Restore/restart must not auto-respawn. Unknown worker state is a fatal
   admission block; no fake green language-server indicator.

## Installer packaging policy

Candidate **native first**: rust-analyzer (dual MIT/Apache-2.0), clangd
(Apache-2.0 WITH LLVM exception), Ruff (MIT). Candidate **later**: Pyright
(MIT, Node runtime) and TypeScript Language Server (MIT with third-party
Apache-2.0 notices and Node/TypeScript dependencies). Packaging is not
authorized by knowing an upstream GitHub URL; require exact version/release
artifact provenance, reviewed bytes, complete transitive notices and
per-platform hashes in \`ide/packaging/language-servers.manifest.json\`.
\`verify_language_servers.py --release\` must fail while \`releaseApproved=false\`.
No downloads, auto-updaters or auto-installs are introduced in Wave E2.

## Acceptance accounting

- Code/source implemented: **yes** for LSP protocol, transport interface,
  editor UI and offline license/bundle audit.
- Compiled/native-tested at exact PR head: **not yet verified**.
- Real LSP running under protected Windows/Linux worker: **no**.
- Actual autocomplete/diagnostics/hover/definition in installed Lu IDE: **no**.
- Native Wave D job/service and Linux/Windows hostile acceptance: **open**.
- Release-bundled language server executables: **none**.

**No UI field, LLM request or GitHub CI result can directly authorize a
language-server execution effect.** Do not claim E2 complete until real
native server transcripts and OS cleanup pass on the exact shipped build.
