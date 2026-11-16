# IDE-MERGE-02C — two verified first-render frontend faults (2026-11-15)

**Frozen published**: `374f22b9cdfc4e52c8350dc0f9044a36d62f8026`. The next source candidate is deliberately **unpublished** while the original Windows LLVM18 workflow remains running. Do not move PR #236 or stack CI until it is terminal.

## Original OS installed evidence

In exact-SHA six-job [integrated run #43784599636](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43784599636), both genuine unsigned NSIS/WebView2 and Debian/GTK installed jobs failed at their original required `Workspace projects` native UIA/AT-SPI heading, although exact P06 lock/upstream notices, package/install, real window, and frontend unit checks passed. The newly bundled limited diagnostic reported `visible_render_failure` on **both platforms**, but original logs did NOT expose raw JS exceptions. The strict native OS assertions were preserved.

## Deterministic source-level root cause reproduction

A preserved original Linux 26.04 installed unsigned ELF `/usr/bin/lu-ide` was extracted from original CI artifact, and its Brotli-packed real `/assets/app.js` was decoded. Under a synthetic React 19 hooks dispatcher (no DOM acceptance or native IPC), direct first-render execution of the production bundled `App` with the same empty initial state threw:

1. `TypeError: Cannot read properties of null (reading 'providerId')` in the `lspProviderForTab` expression. Optional comparison `lspSession?.projectId === activeTab?.projectId` evaluates `undefined === undefined` when both objects are absent, then incorrectly dereferences `lspSession.providerId`.
2. After applying only that guard to the unchanged bundled JS in a local diagnostic copy, the next first render threw `TypeError: Cannot read properties of null (reading 'rows')` in `diskAuditCounts`. The same `undefined === undefined` false-difference condition lets a null `diskAuditReport` fall through to `diskAuditReport.rows`.

After adding **both explicit presence checks** to that temporary extracted JavaScript, the same synthetic hook-driven initial render returned the actual React `div.ideShell` tree including `Workspace projects`. This reproduces and explains the shared initialization failure at the **real bundle's render logic**, but is NOT a substitute for installed native Windows UIA or Linux GTK/AT-SPI acceptance; a captured installed JS error stack was not provided by the original driver.

## Minimal production source repair and regression

- In `ide/app/web/main.jsx`, require **both** LSP session and active tab objects before comparing exact project ID/provider; retain original language-provider fallback, provider deny and cross-project isolation.
- In the same file, require **both** disk audit report and active project objects before comparing exact identity or reading report rows; preserve actual per-status counts and original security paths.
- New Node native-source regression `ide/app/tests/ide_initial_workspace_render.test.mjs` exercises the **exact source expressions** against absent, foreign, valid and denied provider cases; and builds the actual esbuild/React 19 production entry to evaluate the **entire original initial App render** using a fake hook dispatcher, requiring the actual initial `Workspace projects` heading and disallowing an error-message substitute. This is bounded source-render acceptance, **not physical/installed OS evidence**.

No production IPC, native custody, P05/P06, Tauri CSP, release/signing, OS WebView or installed UI driver is changed. Do not claim original installed acceptance until a subsequent Windows NSIS UIA + Linux GTK AT-SPI run succeeds with all original unchanged assertions.
