# AGENT-01G — Exact-current-source physical Windows NSIS test build

**Status:** Test-only **builder and workflow source committed/pending native compilation**. Not a downloadable installer until GitHub's Windows build, native NSIS cycle, and artifact upload actually complete. Not alpha, production, GPU/Vulkan acceptance, a code-signed release, or a replacement for your regular Lu installation.

**Owner direction:** Use a new current-source Windows installer to exercise acceptance gates that hosted Rust tests cannot clear: native Windows dialogs, real GGUF selection, real worker/tool choice, human authorization, independent evidence shown in conversation, interruption and restart.

## What is being built

- **Source:** the exact immutable GH pull-request head of `vjmichael/Lu-AI-GUI` containing AGENT-01F/01G and the present React/Tauri conversation/plan interfaces. The frozen 739+ source blob manifest is checked before any build.
- **Engine:** a **real source-compiled Windows x64 llama.cpp CPU** `llama-server.exe` from upstream commit `7fe450e19305b828c199d602c23a8337aaa1f03b`, compiled with MSVC static C runtime and Vulkan intentionally off. Its real bytes and SHA-256 are captured after the build, bound to the exact desktop executable at Rust compilation by feature `physical_win_cpu_eval`, and checked again at startup. No arbitrary model-provided engine binary can be substituted.
- **Default production engine unchanged:** normal builds retain the existing pinned Vulkan SHA and four-file resource manifest. The CPU test feature is explicitly opt-in, with a unique app identity `org.lu.agent01g.devtest`, title **Lu AGENT-01G CPU TEST (NOT ALPHA)** and an isolated NSIS installation directory `%LOCALAPPDATA%\Programs\LuAGENT01GPhysicalTest`.
- **Files:** current Rust desktop EXE with actual React frontend, source-built `llama-server.exe`, engine manifest, build provenance JSON, a human physical acceptance README and a real installer/uninstaller. **No GGUF is bundled**; operator imports an existing small local model.
- **NSIS:** separate `NSIS_AGENT01G_PHYSICAL_WINDOWS_CPU_TEST.nsi`, self-contained unsigned EXE, user-level installation, Start Menu entry and isolated test-only uninstall. Uninstall leaves all user profiles, chats and model files in AppData.
- **Native build:** actual `cargo rustc --release --locked` with Win64 Common Controls v6 embedded RT_MANIFEST verification (to avoid R13/R22 TaskDialogIndirect startup breakage), and current frontend checks.
- **Native packaging gate:** on a disposable GH Windows agent, run the existing genuine `scripts/m3-windows-nsis-cycle.ps1`: verify installer SHA, silent installation, installed desktop SHA, installed engine SHA, actual installed uninstaller and removal. That result is separately labeled as an **installation lifecycle check, not loaded GGUF or dialog testing**.

## Exact files

| File | Role |
|---|---|
| `.github/workflows/lu-agent01g-physical-windows-nsis.yml` | Exact-head Windows build and upload only after native NSIS is produced and its installed lifecycle passes |
| `scripts/build-agent01g-physical-windows-nsis.ps1` | Native MSVC source engine build, pinned hash, frontend, Win64 manifest, real Rust binary and NSIS package |
| `lu-desktop/packaging/NSIS_AGENT01G_PHYSICAL_WINDOWS_CPU_TEST.nsi` | Isolated unsigned installer |
| `lu-desktop/packaging/README_AGENT01G_PHYSICAL_WINDOWS_CPU_TEST.txt` | Physical testing guide shipped with installer |
| `lu-desktop/tests/agent01g-physical-installer-contract.test.mjs` | Deterministic source-only safety contracts; cannot replace native build |
| `docs/acceptance/LU_AGENT01G_PHYSICAL_WINDOWS_CPU_TEST_20261108.json.template` | User physical test receipt; blank statuses default to NOT_TESTED |

## Download after real compilation

The Windows GitHub Actions job **must actually complete** with a green result and a genuine artifact named `Lu-AGENT01G-Physical-Windows-CPU-NSIS-<exact commit SHA>`. The downloaded artifact ZIP then contains one genuine `Lu-AGENT01G-Physical-Windows-CPU-Test-<SHA12>.exe` alongside `installer-checksums.json`, `BUILD_PROVENANCE.json` and the native install-cycle receipt. Never invent a download link before that artifact exists.

**Do not manually rename or repackage the older R22/R25 installers**. Existing normal `Lu Desktop CI` only stages `NOT_AN_ENGINE_CI_PLACEHOLDER` resources to type-check: its output cannot run a model and must not be distributed as acceptance proof.

## What the user can actually help close

1. **WIN-NSIS / M3:** physical clean install, launch, exit, uninstall, no regression to main Lu profile. Hosted native install-cycle artifact is independent supporting evidence.
2. **WIN-GGUF:** real local small GGUF load, response, unload. CPU branch tests general functionality only; not Vulkan GPU memory offload.
3. **AGENT-01F/01G UI:** original human goal, real model structured plan, approval-only confirmation, distinct native picker and fresh step confirmation (including native cancel -> no authority).
4. **AGENT-01G execution/evidence:** with a disposable Git repo, one reviewed Git HEAD operation must originate from the actual loaded model; source correctness independently verified and recorded separately from model prose.
5. **AGENT-01G recovery:** restart and interrupted-step history, no old permission reuse or silent uncertain replay. Screenshots and logs are **observations**, not automatic source correctness acceptance.

**Still open:** Windows Job Object full descendant custody, Linux cgroup-v2, provider semantic reliability, whole-app multi-step continuation, AMD Vulkan offload, legally cleared redistribution, signing and physical release acceptance. Do not mark them closed on the strength of this installer alone.

**Operator privacy:** record sanitized evidence and compact FAIL/PASS statuses. Avoid publishing raw GGUF paths, secrets, personal memory, repository names, tokens or screenshots to the public repo.

## Failure policy

If current Rust code, pinned-engine build, Win64 Common Controls v6, exact binary hashes, NSIS compilation or real disposable install/uninstall fails, the GitHub workflow must **fail without uploading a deceptive installer**. Inspect actual Windows logs and repair the exact failure before trying a later new source head. Do not use a source-only contract pass or an older Lu executable as a substitute.

**Companion:** [AGENT-01G engineering ledger](LU_AGENT01G_REVIEWED_STEP_EXECUTION_20261108.md), [K/I/A/F execution ledger](LU_KIAF_ACTIVE_EXECUTION_LEDGER_20261108.md).
