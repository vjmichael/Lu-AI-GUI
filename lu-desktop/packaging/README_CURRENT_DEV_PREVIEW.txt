LU CURRENT DEVELOPMENT PREVIEW — NOVEMBER 2026 SOURCE-LOCKED NSIS
=================================================================

This is a NEW exact-current-commit desktop build, not the older alpha
installer or AGENT-01G one-operation demo. It contains the current Lu Core
Phase 1/2/3 implementation and TWO source-pinned llama.cpp servers:
CPU and Vulkan.

PRODUCT IDENTITY
  Name: Lu Current Development Preview
  Version: 0.2.0-rc.1 (preview version label, NOT a release acceptance)
  App identifier: org.lu.current.devpreview
  Default install: %LOCALAPPDATA%\Programs\LuCurrentPreview

What it does NOT include
  - No GGUF weights, private documents or Qwen model bundled.
  - Bundled inference engines include BOTH genuine source-built CPU and
    Vulkan llama-server executables from pinned llama.cpp commit
    e0f142d5ddc1461ce721644d92af33db23545105, supporting Qwen3.8.
  - Both engine digests and exact byte lengths are verified at Lu startup;
    model prompts cannot change executable paths.
  - On Windows, Vulkan is the default backend. In Lu Settings > Inference
    backends, select CPU or Vulkan; RESTART Lu to apply the new setting.
    Switching a setting does not grant a tool permission or execute a model.
  - No proof of AMD Radeon AI PRO R9700 Vulkan acceleration.
  - No third-party cloud inference or automatic tool approval.
  - Unsigned test preview, not production, not a finalized v1.0.

YOUR QWEN3.8-27B HANDOFF
  1. Obtain a legitimate Qwen3.8-27B GGUF (Q4_K_M is recommended for initial
     32GB GPU experiments), e.g. axteam/Qwen3.8-27B-GGUF or ggml-org.
  2. If vision is needed, use the model provider's MATCHING MMProj GGUF.
     The axteam text GGUF lacks a vision projector.
  3. Use scripts/run-qwen38-27b-local.ps1 to test a real model response
     using the bundled Vulkan llama-server. You may compile another pinned
     Vulkan engine with -BuildVulkan for a SEPARATE diagnostic. This does
     not bypass Lu's sealed per-backend engine identities.
  4. Install the preview in its isolated directory and load/import a
     local GGUF through Lu. Select Vulkan in Settings and restart before
     first loading Qwen3.8; use CPU if the Vulkan driver is unavailable.
     Begin with text-only and a disposable read-only Git repository.
     Do not mistake a successful CPU fallback for proven GPU offload.
  5. In Lu, test goal/clarification, draft/review-only plan, separately
     reviewed step, source oracle, repeated multi-step child-plan approval,
     replan without grants, memory source/contradiction and restart recovery.
     Record PASS/FAIL for each. No automatic retries of uncertain effects.

SECURITY / DATA
  Uses a distinct install directory and product identity. Uninstall only
  removes preview program binaries; it does not delete Lu ANSF chat
  SQLite, model GGUF files or personal user project data. Verify this.
  The build provenance and installer SHA-256 files are included for
  independent checking. SmartScreen may warn for an unsigned build.
  Do not globally disable Windows security; review the repository SHA,
  verify published checksum and test only if you trust the source.

CONFIRMED vs NOT TESTED
  Native GitHub Windows build/install checks and actual compile results
  must be read at the exact commit. An installer created without a
  successful native run is not a downloadable, proven installer. Physical
  AMD Vulkan/MMProj/27B and user approval remain the owner's test scope.

GitHub repository: https://github.com/vjmichael/Lu-AI-GUI
