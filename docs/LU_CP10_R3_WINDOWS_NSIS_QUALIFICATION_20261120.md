# CP-10-R3 — Exact-source Windows NSIS qualification

**Status:** R3 source/contract implementation prepared; hosted NSIS installation and owner physical Windows acceptance **NOT YET ACCEPTED**. No merge to `main` and no release/v1.0 claim.

## Observed exact-source CI

- Consolidated UX candidate `08b8d240e426e4ab60f462617b5dcd85673a1c3b`, [run #44843345136](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44843345136): **Windows native-protocol passed**. Desktop source compilation was still in progress at R3 preparation; mark the consolidated run **PENDING** until both jobs terminal and successful at the unchanged SHA.
- Previous pre-UX candidate `6c7eb0569f5061d5d177b8fa0de0f26c015ed706`, [run #44838204515](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44838204515): both jobs PASSED at that **earlier** SHA. Do not transfer its PASS to the consolidation or a subsequent R3 commit.

## Installer qualification route

The checked-in `lu-desktop/src-tauri/resources/lu-inference/manifest.json` describes an earlier Vulkan development build whose actual EXE/MSVC DLLs are not checked into this repo. This is an accurate **pre-provisioning inventory**, but the dedicated current-source Windows build workflow **does not use the missing historical binaries to fake an installer**. Instead:

1. Existing `.github/workflows/lu-core-c6-hosted-dual-nsis.yml` explicitly remains **manual-only / reusable**, as its independent source contracts require.
2. New `.github/workflows/lu-cp10-r3-windows-nsis.yml` permits one self-path push on `lu/cp10-r3-windows-nsis-20261120`, with the **exact commit message** `CP-10-R3 source-locked Windows NSIS qualification`. The new wrapper first requires immutable source verification, native Windows named-pipe/default-off tests, real frontend build/check and the single-interface UX contracts. It only invokes the reviewed C6 workflow if those source/native checks pass. No schedule, broad pull-request trigger or CI stacking is introduced.
3. The reusable workflow requires a genuine Windows 2025 MSVC toolchain, pinned llama.cpp source (`e0f142d5ddc1461ce721644d92af33db23545105`), source-built distinct CPU and Vulkan executables, pinned Vulkan SDK 1.4.350 and NSIS 3.11, a verified signed Windows research client and exact compiled first-party Lu app. It reports installer SHA/size/provenance rather than claiming a physical GPU or model proof.
4. Its existing `m3-windows-nsis-cycle.ps1` actually runs NSIS in a disposable Windows profile, hashes installed executables, verifies installed WebView2/Settings/Model Library UI, uninstalls and independently checks that an AppData sentinel survives. If any real run fails, retain that failure as the next repair task; do not bypass toolchain, source or installer checks.
5. Browser runtime is deliberately **NOT_PACKAGED** in the simple C6 workflow; a separate C6.2B candidate owns reviewed-browser packaging. Do not describe a successful C6 run as a complete v1.0 installer or fully functional browser. Likewise, the present codec resource folder does not qualify every codec DLL simply by existing.

## Independent remaining gates

- Real installed two-independent-GGUF-model approval/refusal/cancellation/revocation/replay and distinct Windows account/process isolation. The `cp12br7r1-windows-installed-acceptance.ps1` runner logs human observations and **never self-certifies** the native protocol.
- Owner physical Radeon AI Pro R9700 Vulkan/CPU inference, MMProj vision, model startup/restart, recovery, durable memory and complete tool use. Hosted Windows runners do not substitute for physical hardware.
- Production-ready resource provenance (including reviewed browser and codecs), multi-version NSIS upgrade/rollback and user data backup/migration.
- Historical UI failures: P03 workspace-routing and P4 scheduler-priority; preserve them as failures until independently repaired and retested.
- Do not reintroduce Basic/Advanced modes into Lu; that design belongs to NVP.

## R3 local source checks

Focused CP-10-R3, C6 native/installer, dual backend and physical-contract source checks: **18 passed, 0 failed**. These are local contract assertions, not installed Windows acceptance.

**Next:** Run the exact R3 source/native and actual hosted NSIS qualification once the preceding consolidated CI is terminal, repair measured failures as one batch, then execute physical Windows/two-model acceptance. Keep `main` and original R7 unchanged.
