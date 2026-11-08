# DEV-02.5 — genuine upstream native LSP test-only payload

Status: source candidate; exact-head Windows/Linux native job results required.
Stacked on DEV-02.4 draft PR #134.

## Deliverable
- Check three original SHA256-locked archives for the current native OS:
  clangd 23.1.0, rust-analyzer 2026-11-02, Ruff 0.17.4.
- Check four actual Git blob pinned original upstream license notices.
- Extract exactly one bounded executable from each archive using the existing
  hostile-archive verifier. Copy the verified four notice files.
- Build a sealed seven-file local ZIP in GitHub Actions RUNNER_TEMP with no
  release, redistribution, containment or execution authority.
- Hash-install every file into a second new disposable folder, reject unknown
  or duplicate ZIP members and recheck exact on-disk hashes.
- Publish only JSON results, never the original archives or candidate ZIP.
- Test corrupt digests, forged notices, malicious members, out-of-temp
  destinations and already existing install roots.
- Keep the production language-server manifest releaseApproved=false.

Workflow: .github/workflows/ide-dev02-disposable-native-test-payload.yml
Code: ide/packaging/stage_native_lsp_disposable_test_install.py
Tests: ide/packaging/test_stage_native_lsp_disposable_test_install.py

## Scope of proof
The temporary hash-installed payload is NOT an actual Windows NSIS/Linux
Debian application installation. It is NOT a running protected language
service, not a completed original-release dependency closure and does not
prove installed Lu desktop completion, hover, diagnostics or navigation.
Only one native executable is extracted per archive: clangd may require
original LLVM dynamic libraries or resource directories, so the receipt
explicitly declares dependencyClosureVerified=false.

Python semantic BasedPyright versus ty remains the separate DEV-02.4
candidate comparison; Ruff remains diagnostics-only in the protected Lu
source. The temporary installer is never authorized for production Lu
execution and is never uploaded as a binary artifact.

## Remaining acceptance order
1. Accept exact-head DEV-02.4 BasedPyright/ty Windows/Linux real metric
   receipts with same-file Windows URI and clean shutdown/reaping.
2. Accept genuine DEV-02.5 native Windows/Linux source plus pinned-asset
   download, stage, hash-install, notice and tamper-denial receipts.
3. Perform full original clangd DLL/resource dependency audit and actual
   pinned native version/LSP smoke tests under protected OS custody.
4. Prove Linux root-owned Bubblewrap/delegated cgroup-v2 and Windows signed
   restricted LocalService/AppContainer/Job Object launch, process reaping,
   exact-source consent and no unauthorized project access.
5. Run real installed NSIS and Debian application UIAutomation/GTK tests.
6. Leave deferred signing, licensing and redistribution review open until
   independently approved; never set releaseApproved by assertion.
