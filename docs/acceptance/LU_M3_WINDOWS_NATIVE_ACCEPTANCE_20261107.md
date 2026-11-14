# Lu M3 native Windows installation, hot-scroll and local GGUF acceptance

**Disposition:** source harnesses implemented, **real physical Windows evidence not yet observed**. This is neither a claim of an installed release nor permission to operate on existing personal installations.

## 1. Genuine NSIS cycle on a disposable Windows test account

Run `scripts/m3-windows-nsis-cycle.ps1` on a clean disposable Windows 11 environment, specifying an actual installer file, its independently captured SHA-256, the expected Lu executable SHA-256, bundled llama-server executable SHA-256, and **actual** installed uninstaller filename (do not guess). Explicitly pass `-DisposableWindowsConfirmed`. No network access, fake engine, cleanup of uncertain installation state, or silent override of installed user data occurs.

Example with placeholders to replace using **independently verified** values, not a claimed turnkey command:

```powershell
.\scripts\m3-windows-nsis-cycle.ps1 -DisposableWindowsConfirmed `
  -InstallerPath 'C:\reviewed\Lu_0.1.0_x64-setup.exe' `
  -InstallerSha256 '<64 hex digits>' -ExpectedLuSha256 '<64 hex digits>' `
  -ExpectedEngineSha256 '<64 hex digits>' -UninstallerName '<actual installed uninstaller.exe>'
```

A PASS here means real installer/uninstaller processes exited and the binaries were independently hashed/removed. It **does not** mean model inference, UI behavior or worker cleanup passed.

## 2. Physical interactive installed WebView + model session

Run `scripts/m3-windows-installed-acceptance.ps1` against the actual installed Lu executable and independently pinned genuine local GGUFs. Specify `-PhysicalInteractiveWindowsConfirmed -ExerciseManualScenarios` after confirming all supplied SHA-256 digests. The native UIA verifies that the OS window belongs to the launched executable and queries the history button; OS CIM verifies observed pinned llama-server processes by executable path and SHA-256, and their reaping across a clean restart. It does not collect conversation/model content.

Manual scenario confirmations explicitly record `OPERATOR_REPORTED_PASS`, never a synthetic automated success. Required scenarios include a real assistant GGUF chat turn, cancel, a distinct local embedding GGUF and paraphrase lookup, 2,048+ ANSF original turns with variable-height backward/forward scrolling, concurrent streaming/paging/chat switching, and recovering checkpoint/CCM/permission state after restart. The script labels an unobserved engine as INCONCLUSIVE and cannot self-certify M3.

## 3. Required release decision

Use these JSON receipts **together with** exact-head Windows/Linux CI logs, the independent genuine GGUF Cognitum three-arm raw results (not just summary), hash/provenance/licensing review, output evidence that no worker process tree remained, and a manual keyboard/screen-reader witness. Failure or missing evidence means M3 remains open. The installer provided must have been produced from a real pinned payload; CI-only `NOT_AN_ENGINE_CI_PLACEHOLDER` resources do not count.

The main history viewport now silences old-page re-announcements (its `role=log` has `aria-live=off`); only live generation **status** uses `aria-live=polite`. Native cursor/geometry tests and physical WebView/assistive technology testing are separate evidence levels. Source-ID anchors should remain stable across book opens, chat changes and streaming.

## Known limits

CI cannot certify a physical GPU, interactive WebView2/Windows screen reader, real human model import or install/uninstall on the user's machine. The current operational asset ledger safely detects reusable package hashes and read-only checkpoint proof, but unattended missing-toolchain reconstruction requires additional per-turn user approval and native installer/provenance integration; a previous asset path never constitutes authority to install or execute it. Do not mark full NSIS environment restoration PASS from a read-only ledger.
