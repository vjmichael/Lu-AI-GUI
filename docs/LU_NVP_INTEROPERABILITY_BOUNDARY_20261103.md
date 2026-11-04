# Lu ↔ NVP — independent source-contract verification

**2026-11-03.** This is a development-only CI branch based on pinned Lu-AI-GUI main source ac015280e4594b7cb5fb730a240b8d2a8a819975. The accepted installed Lu 0.1.0-alpha.2 binary, its Windows acceptance and its source revision are NOT modified or re-certified by this branch.

## Intended use

NVP needs a real local LLM inference engine for filmmaker workflows. Lu Core is an appropriate prospective provider because the functional alpha already proved GGUF/MMProj inference on the owner's Windows system and the independent Rust Core has typed model sessions, streamed events, cancellation, and supervised engine lifecycle. The external integration boundary, however, does **not** exist yet in the alpha.

- The independent desktop's lu_status, lu_load_model, lu_unload_model and lu_generate are internal Tauri commands. Not a published external socket service.
- llama.cpp is a private Lu child on an ephemeral 127.0.0.1 port with a Lu-issued API key; another app must not discover or borrow this credential.
- The common AF_UNIX carrier resides only on isolated jan_alt branch lu/ipc-common-af-unix-20261031 and carries native ToolCall/ToolReply. Native Windows/Linux socket evidence exists but does not confer inference permissions, cross-application host identity or installed alpha integration.
- An external NVP provider must be Lu-owned, explicit opt-in, compatible with saved conversation/creator approval and maintain separate model custody. NVP must not unload Lu-owned models or declare NVP ownership merely because it observed a model.

## Implementation

This branch adds tools/lu_nvp_source_seal.py and .github/workflows/lu-nvp-independent-inference-contract.yml only, plus a narrowly repaired stale Rust test initializer (now explicitly supplies an empty history array). It does not edit runtime code or enable any listener. The workflow verifies exact canonical Git blob IDs for the Lu inference proto, desktop host, persistent engine and real-model smoke example. It then runs Lu Core's actual Rust inference lifecycle, Protobuf wire and host resource-authority tests on Linux and Windows. This is genuine source-level execution, but NOT a real GGUF or deployed desktop test.

## Live acceptance still required

The existing Lu Core example src-tauri/lu-core/examples/persistent_llama_smoke.rs provides a real pinned-engine/GGUF two-generation/same-PID smoke on an authorized host. On Windows the current persistent backend requests 99 GPU layers, so no automatic large-model acceptance should run until NVP/Lu resource admission, host RAM/commit and VRAM budget checks are in force. Prefer a small test model and verified OS process-tree cleanup. User authorization must be host-held; the AF_UNIX path/name is not authority.

A production Lu-to-NVP integration will require a supported Lu external-host protocol (or Film DLC running inside the Lu host) with versioned authenticated requests, session custody, token streaming, cancellation, generation and resource measurements, and owner-verified completion. Only afterward should NVP enable control, as distinct from passive status observation.
