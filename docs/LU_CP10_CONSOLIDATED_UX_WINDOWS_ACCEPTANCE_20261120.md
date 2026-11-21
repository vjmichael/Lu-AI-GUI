# CP-10 — Consolidated single-interface Windows acceptance candidate (2026-11-20)

**Status:** SOURCE INTEGRATED, native/installed acceptance on this new source SHA remains OPEN. The R7 transport and Windows desktop CI **passed** at prior immutable SHA `6c7eb0569f5061d5d177b8fa0de0f26c015ed706`: [run 44838204515](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44838204515), both jobs success. Do **not** transfer that pass to this consolidated tree without requalification.

## Consolidated changes

- Merge source from local UX-R2 commit `b7fbd61f098127d1c6d8b16786bb37f6f843a196` (includes CP-13-R1 design reconciliation) onto the **same-source** local CP10 candidate tree `6ff4047e7ac666a24c7ad89b30869212d4ab9447`. The published CP10 SHA is the parent for the GitHub integration candidate, with no destructive edits to original R7 or previous accepted CP10 source.
- Explicit owner direction: **one Lu interface**; NVP alone owns Basic/Advanced mode. Chat, Settings, Model Library, History and Project Tasks remain ordinary user-facing places; no new global mode switch. Source hashes, traceability and consent remain available through individual disclosures without becoming an alternate UI mode.
- Preserve all Rust/worker/authorization/default-off implementation from the verified CP10 parent. UX changes are frontend/tests/docs; no new automatic permissions, GGUF retrieval or network egress.
- Point a single narrowly scoped source-locked Windows workflow at the isolated consolidation branch, requiring one exact commit message. The workflow checks actual Windows-native kernel protocol and default-off Rust tests, desktop source compilation, existing consent contracts and the merged UX contract tests.
- Update historical workflow-source assertions to track the actual current immutable push branch/message, still requiring exact SHA, restricted paths, no broad PR/schedule, no credential persistence, and no physical-acceptance substitution.

## Native and installed gates

1. Source check: committed source-tree lock, frontend build and UI checks, focused consent/UX tests, full UI suite with **historical workspace-routing and P4 scheduler-priority failures retained**. Scanner fixture may be reused from unchanged native source only as a local test input; not a fresh native compile.
2. Hosted CI: exact-SHA consolidated candidate requires both Windows-2025 jobs to finish success with actual native transport and default-off regression. CI is **not** installed physical acceptance.
3. Windows NSIS installer: **not buildable in this container** because the target is Windows/NSIS and required first-party, verified `llama-server.exe`, MSVC runtime and native codecs must be genuinely provisioned. Do not override `tauri.conf.json` resources for a distributable. An earlier CI type-check's temporary resource override is not a shipping package.
4. Installed GUI: require real installed x64 `Lu.exe`/`lu-desktop.exe`, original package/runtime hashes, interactive model-loading and both native consent dialogs. Existing `scripts/cp12br7r1-windows-installed-acceptance.ps1` requires `-InstalledExe`, `-ExpectedExeSha256`, `-ModelFile`, `-ExpectedModelSha256`, `-EvidenceDir`, `-DisposableTestProfileConfirmed` and optionally `-AllowControlledCrash`; its observations are explicitly user-reported and **cannot independently certify** acceptance. Do not save personal contents, invitation codes, account IDs or model paths in reports.
5. Physical Radeon R9700 Windows host: explicit installed GPU/CPU GGUF inference, native permissions, separate-model processes, cancellation/revocation/replay, fail-closed service/guardian crash, restart, upgrade/rollback, ANSF source retention and data recovery. Real second Windows account needs separate account-operated evidence; cannot be simulated by hosted Windows CI.
6. Release remains **NOT QUALIFIED** until native, installed and physical gates pass on the same final candidate. No new major features or Basic/Advanced mode.

## Next task

**CP-10-R3** — Complete exact-source hosted Windows CI for merged single-interface candidate, resolve any actual failures as one consolidated batch; then qualify native NSIS installer and genuine two-model installed Windows GUI on owner hardware.
