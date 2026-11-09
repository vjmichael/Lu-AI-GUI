# Lu AGENT-01L — physical Windows installed-app acceptance (OPEN)

**Purpose:** Independent user-run acceptance of the exact unsigned Windows NSIS build, after the GitHub native Windows Job Object gate passes. This is not an installation receipt and does not instruct Lu to replay permissions.

## Build identity

The source branch is `lu/agent01l-linux-cgroup-v2-20261108`, stacked on native OS custody PR #173 and AGENT-01K. The current NSIS packaging job is `Lu AGENT-01G Current-Source Physical Windows NSIS CPU Test` in `.github/workflows/lu-agent01g-physical-windows-nsis.yml`. Its **first prerequisite** is exact SHA/source-tree verification, followed by pinned NSIS 3.11, genuine CPU llama.cpp compilation and actual Tauri Lu Windows binary. It then runs a **Windows hosted runner** install/uninstall cycle, uploads a SHA-256 checksum and the unsigned installer.

A queued source job is not a compiled installer. A hosted runner is not the physical PC. **Do not use a guessed `sandbox:` link or claim downloaded bytes until a real artifact is uploaded.**

## Physical machine acceptance record

| Gate | Required evidence | Status |
| --- | --- | --- |
| Correct installer | Actual GitHub Actions artifact, exact commit SHA, installer SHA-256 equals `installer-checksums.json`; source provenance matches | OPEN |
| NSIS lifecycle | Install and launch native Lu x64; clean normal close, uninstall and reinstall with user consent | OPEN |
| Installed CPU inference | Real GGUF model selected in Lu, normal text response from running llama.cpp (not placeholder) | OPEN |
| Multimodal model | If tested, actual GGUF+MMProj image Q&A; never conflate CPU-only build with an AMD Vulkan build | OPEN |
| Windows GPU | Separately built Vulkan backend and actual AMD offload measurement; CPU NSIS is **not** this test | OPEN |
| Exact native Git | Model chooses a reviewed Git operation; independent native Git HEAD/status oracle confirms the source facts | OPEN |
| Fresh two-step authority | Original approved step uses one fresh saved human execution turn. Later step requires a new child plan, second plan review, a different fresh saved human turn, and distinct tool permission | OPEN |
| OS custody | Actual Job Object kernel `ActiveProcesses==0` witness recorded for exact reviewed completed call, reopened in original turn/task journal; no unverified descendant remains | OPEN |
| Restart / no replay | Close/restart Lu, see original chat/ANSF history and independent receipts, and prove that old owners/grants cannot be reactivated. A new step must request new consent | OPEN |
| Failure/recovery | Cancel worker and crash/restart: Lu shows interrupted/uncertain operations, never invents a successful result or silently repeats an OS effect | OPEN |

## Human-safe sequence

1. Use **only** the genuine Windows NSIS artifact generated from the exact reviewed SHA. Inspect its GitHub provenance and SHA-256 before running the unsigned test-only installer. Do not bypass Windows security warnings for a build whose origin cannot be verified.
2. On a disposable test installation or a backup-safe physical PC, install Lu through the NSIS UI, open it normally, import a local Qwen GGUF (and MMProj only if that model requires it), and perform a simple text prompt. The pinned CPU NSIS branch should not be described as a Vulkan/ROCm benchmark.
3. In an existing local Git repository, approve one explicit reviewed operation; compare Lu's returned Git HEAD with a separate native Git process on the same unchanged repository.
4. Prepare the next step using Lu's native approval UI. Verify that this creates a **new** child plan requiring its own review and consent, not an automatic tool action. Complete a second separately authorized Git check and independently verify it.
5. Restart Lu. Inspect chat history, approval links, TaskStore/EffectJournal evidence, the independent outcome oracle and the original Windows Job Object witness. Attempting to reuse either historical grant or scheduler owner must be denied.
6. Record installed Lu version, Windows build, GPU model, model filename and SHA-256, installer SHA-256, exact repository commit tested, passed/failed gates, and original native logs. Do not upload personal secrets or unrelated private conversation data.

## What CI can and cannot certify

- Windows 2025 GitHub runner: compile and execute Win32 Job Object/native Git/SQLite tests, build a complete CPU NSIS package, and run an automated disposable VM install/uninstall lifecycle.
- **Physical Windows PC:** actual desktop UX, local Qwen model, GPU-Vulkan hardware offload, actual UI approvals, host restart, OS descendant recovery and real user install/uninstall.

**Status:** No NSIS artifact has been confirmed on the original and stacked runs at last inspection. Native Windows Job tests and actual model/physical PC acceptance are pending, not failed or passed. When the GitHub artifact exists, provide the exact verified artifact link; never invent an `.exe` or call the source artifact a build.
