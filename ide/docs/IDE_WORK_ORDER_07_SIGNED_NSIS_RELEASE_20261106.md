# IDE WO7 — Signed NSIS release packaging and protected worker distribution

**Started:** 2026-11-06 (America/Phoenix). **State:** implementation IN PROGRESS; real signed release and installed application NOT ACCEPTED. Branch `ide/wo7-signed-nsis-release-20261106` stacks on WO6 draft PR #70.

## Goal

Produce a one-click native Windows NSIS distribution of **standalone Lu IDE**, installing only independently reviewed, pinned and licensed runtime components and its separately protected LocalService recovery worker. The build must retain the existing **unsigned default-disabled alpha** for development without confusing it with the signed protected-feature release.

## Work items

1. **WO7-A — Trusted release admission (first source increment):** `packaging/wo7-release.manifest.json` starts `releaseApproved=false`; `verify_wo7_nsis_release.py` checks clean exact source SHA, explicit native and installed acceptance, real Windows Authenticode validation of selected installer/service binaries, signer thumbprints, actual SHA-256 bytes, file path/symlink protections, reviewed dependency licenses and WO6 debugger manifest status. Test-only signature injection exercises logic but cannot approve runtime. No auto-download, signing, installer execution or authority escalation.
2. **WO7-B — Reproducible, separately audited package tree:** stage exactly the pinned app, protected SCM executable, manifest, approved LLDB and LSP bundles, full third-party notices and immutable package content. Independently hash the complete tree and inspect actual runtime DLL imports, without treating fixture hashes as release pins or passing metadata as cryptographic evidence.
3. **WO7-C — Real reviewed Windows signing:** sign the final NSIS installer **and** separately protected service; verify Authenticode chain and exactly approved signer. Package service with installer-owned restricted ACLs, SCM install/reinstall/uninstall and failure-safe rollback. No wildcard AppContainer ACE or silent elevation.
4. **WO7-D — Exact installed security acceptance:** from real signed NSIS, install, launch, consent to fixed protected Run/Test/Debug, verify native AppContainer+restricted SCM, cancellation, service crash/desktop restart recovery, source tamper denial, uninstall/reinstall and no leftover Job/service/device lease. Native acceptance from WO4–WO6 is a prerequisite, not assumed from CI fixture success.
5. **WO7-E — Provenance and release declaration:** require CI exact SHA, source cleanliness, artifact checksums, target OS signatures, license inventory and actual installed tests; explicitly promote `releaseApproved` only after those independent records are available. Keep WO8 broader installed E2E and WO10 optional Lu bridge separate.

## Release restrictions and evidence

The first script's successful return (possible only with genuinely approved metadata and valid Windows signatures) is **binary preflight**, not a complete release certificate. Signatures do not prove a binary's safety or OS containment. Merely including a debugger license is necessary but does not close the dependency inventory. Do not turn on native build/debug features by default or infer OS acceptance from LSP/DAP protocol unit tests. Existing unsigned NSIS smoke does not certify a signed SCM installer.

**As of start:** WO6 Linux protected Cargo check/test/cancel passed in native CI; Linux protected LLDB-DAP initialize under the cgroup supervisor and Windows native debugger/install feature acceptance remain open. Real Windows release signer and licenses/hashes are not selected. No release binaries staged or signed and no installer certificate available. All such gaps remain open.

## Verification

`.github/workflows/ide-wo7-signed-release-preflight.yml` runs self-contained hostile manifest tests on Windows and Linux and proves the unapproved source manifest is rejected. Actual signer/native installed acceptance is reserved for later independent Windows release workflow and physical installed app.
