# FIN-02 — Explicit protected alpha feature wiring, never default authority

**Source changes:** `fin02-protected-alpha` is an opt-in Cargo feature
combining the existing independently feature-gated native LSP, Build/Test,
multilanguage workers and project debugger. It deliberately does **not**
enable package-uv execution or the developer-only native debugger-action
dialog. `default=[]` is unchanged.

Build-script admission requires the reviewed `lsp-compile-pins.json` stage
when this explicit feature is used; existing native build-script validation
checks its staged provider bytes and protected-service pins. Missing
resources **fail the feature build** rather than silently building a
protectionless "integrated" binary. The normal dev/default source build
remains inert.

Native `ide_native_feature_inventory` reports compile-time feature booleans,
never a resource hash, worker permission, platform signoff or OS grant.
INT06 Workbench now joins these *only* with independent installed runtime
pin observations before suggesting a protected operation is reviewable.

**Remaining source/physical dependencies:** actual independently approved
Cargo/Python/Node/CMake/LLDB and provider runtime bundle payloads, strict
binary-to-source packaging provenance, Windows protected-service identity,
Linux root-owned runtime/cgroup delegation, Windows uv AppContainer ACL,
production debugger action authorization, install/uninstall migration and
physical human acceptance. The existing reviewed resource manifests still
declare release approval false; do not change those claims without evidence.

CI/PR/native/installed acceptance not executed. No release or main merge.
