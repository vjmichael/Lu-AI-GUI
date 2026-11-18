# Lu Current Preview: CPU plus Vulkan llama.cpp (November 2026)

Scope: exact-head development preview, not historical alpha, unsigned, not v1.0.

## Implemented packaging
- Build TWO native Windows x64 llama.cpp servers from the same pinned commit e0f142d5ddc1461ce721644d92af33db23545105.
- CPU engine: llama-server.exe; Vulkan engine: llama-server-vulkan.exe. Vulkan compiler requires pinned LunarG SDK 1.4.350 and glslc.
- Package two independent SHA256 identities, sizes and package manifest into the Lu desktop executable and NSIS. Verify both at host startup and in installed Windows lifecycle.
- Lu Settings presents CPU and Vulkan choices. A native user confirmation persists the selected backend for the NEXT Lu restart. The model cannot select a path or grant a tool.
- Default backend for the new Windows preview is Vulkan; CPU remains independently launchable if GPU drivers/offload are unavailable.
- No weights, GGUF, MMProj, personal data, profile history or cloud keys in the installer.

## Manifest source defect and fail-closed repair
Earlier Windows #42758146285 compiled the CPU server and Lu desktop release but the PE RT_MANIFEST strict XML parser failed. Source XML itself is valid. Strict verifier now handles only unambiguous Windows UTF8 / UTF16LE encodings and at most one resource NUL terminator. It retains exact PE x64 Windows GUI checks, manifest resource ID 1, Common Controls v6 assemblyIdentity, TaskDialogIndirect import, and fails closed on malformed/multiple XML. The bounded original resource prefix/suffix is shown on failure; no manifest bypass.

## Independent evidence
One SHA-bound Win/Linux grouped build compiles Lu, both engines, and produces genuine current NSIS install/uninstall evidence. Real physical Radeon AI PRO R9700 Vulkan offload, Qwen3.8-27B loading, MMProj and native approval/refusal remain separately OPEN until run on owner's PC. Historical tiny 0.6B benchmark remains deferred.
