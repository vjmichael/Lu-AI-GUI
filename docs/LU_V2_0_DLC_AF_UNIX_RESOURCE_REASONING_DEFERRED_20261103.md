# Lu v2.0 parking lot — common AF_UNIX DLC platform, Cognitum, and NVP

**Decision date:** 2026-11-03 (America/Phoenix). **Status:** ROADMAP / DOCUMENTATION ONLY. **Do not implement these features during the current Lu v1.0 tools effort.** This document supersedes any prior plan to use Windows named pipes or to treat read-only model status as a complete DLC API.

## Non-negotiable platform decisions

1. **Both native Windows and native Linux use AF_UNIX.** Exactly one Lu-owned typed Protobuf service semantics and compatible AF_UNIX carrier for out-of-process first-party apps. Do **not** add a Windows named-pipe adapter, pipe fallback, TCP service, Docker requirement, MCP dependency, or shared internal llama.cpp HTTP credential. If native Windows AF_UNIX access-control, lifetime or peer proof is insufficient for a secure shipping implementation, record that as a blocker; do not silently switch transports.
2. **Transport is not authorization.** OS-specific AF_UNIX peer identity, install provenance, account/profile, workspace/project scope, consent, capability discovery, lease expiry/revocation and per-effect grants remain distinct. Use real Windows AF_UNIX evidence (WinSock capability, socket path ACL / file replacement and peer authentication), and real Linux peer-credential and runtime-dir tests. Linux pass is not Windows acceptance.
3. **One logical first-party host API** with host-native in-process interface for vetted Lu-hosted modules and AF_UNIX for separate applications. Cognitum is an optional *Core Extension / provider*; NVP Film and future IDE/Research/Writing are *Workspace consumers* (and may also provide approved tools). No DLC bypasses Lu's model ownership, capability router, task authority, ANSF scoping or resource guardian.
4. **v1.0 focus remains general purpose.** Build a sound installed desktop with inference, memory, native tools, task authorization, durable journal, cancellation and safe worker management. The DLC interface remains research/design until v2.0. Neither `0.1.0-alpha.2` nor existing AF_UNIX prototypes are proof of shipped DLC support.

## NVP: cooperative model unload/reload and machine-wide RAM/VRAM management

NVP is a valuable experiment because an independent film-rendering app competes with Lu's loaded GGUF for the *same consumer PC* RAM, memory commit and GPU VRAM. Read-only status is not sufficient. NVP should be able to request:

- Current measured model/session inventory, admission estimates and pressure indicators (never invent numbers or expose engine secrets).
- A **cooperative Lu-owned model unload / eviction / profile reduction** when it needs resources for approved Blender/FFmpeg/scene work, and a later **Lu-owned model reload / resume request** when resources are available.
- Hold/release *its own* inference lease, structured streaming + cancellation, and ask for a queued resource transition with an explicit acknowledge/deny/defer outcome and durable completion.
- Resource arbitration across active Lu conversations, other workspaces, model loading transient peaks, NVP render reservations and external processes. Prevent PC freezes, large unintended allocations and VRAM overcommit. If a model is in use by another approved session, Lu may refuse/defer; NVP may not force-kill/unload it, take its API key or claim resource release from a request alone.
- Recovery after restart where previous requested unload/reload or resource transitions are reconciled against actual observed engine/process state. No blind replay or claim of freed memory until measured.

This is a future **bidirectional resource-service contract**, not permission for NVP to control Lu internals. NVP retains ownership of its Digital Film World, Digital Take Packages, Blender workers and rendered/film artifacts.

## Cognitum: optional verified reasoning/logic provider

Cognitum is the second useful integration experiment: it offers *mathematical, causal, symbolic, optimization, logical verification, structured evidence and other non-LLM computation* that can complement transformer behavior. Lu should be able to **use** Cognitum results within its own host-selected reasoning / tool-routing / task orchestration process when the user enables and authorizes that CE.

- Cognitum remains separately developed/installed and **must not be silently bundled into Lu Core**, patch model weights, replace Lu's inference loop, impersonate a model, or mint Lu tool authority.
- Reviewed Cognitum capabilities and schema/effect declarations appear in Lu's host-owned capability inventory. Lu chooses eligible candidates from the actual task, validated metadata and grants; Cognitum returns typed results, uncertainty, proof witnesses, provenance and limitations. Lu checks the evidence before using it to make further decisions.
- Genuine effect-free calculation (e.g. rational arithmetic) should be declared as pure computation rather than fictitious filesystem-read authority. ANSF/memory, project data, GPU reservations and external effects remain separately permitted.
- Explore a reusable Lu-internal **reasoning services / capability router** that can invoke verified subroutines from Cognitum; evaluate concrete task success, latency, confidence and error categories with/without Cognitum. Do not turn the CE into a mandatory architecture dependency.

Cognitum evidence / prior design: [Cognitum PR #61](https://github.com/vjmichael/cognitum/pull/61), [Lu issue #8](https://github.com/vjmichael/Lu-AI-GUI/issues/8), and Cognitum `docs/lu-dlc-ce-addenda.md`.

## Existing research versus shipping claims

| Research/source evidence | What it demonstrates | What it does not prove |
| --- | --- | --- |
| `jan_alt` AF_UNIX tool transport; NVP B5 Windows/Linux socket fixture | Portability feasibility of an AF_UNIX carrier and narrowly scoped socket protocol | Complete Lu Core inference service, OS-scoped production peer grants, model unload/reload or installed-alpha integration |
| [Lu PR #11](https://github.com/vjmichael/Lu-AI-GUI/pull/11) | Draft typed first-party bidirectional service RFC | Canonical shipped runtime API |
| [Lu PR #10](https://github.com/vjmichael/Lu-AI-GUI/pull/10) | Source-level DLC request validation/capability fencing | Live external service, user approval or multi-workload resource arbitration |
| [NVP PR #35](https://github.com/vjmichael/nvp_offline_film/pull/35) | Film service needs and independent provider requirements | Live GGUF provider or workload-safe co-admission |
| [Cognitum PR #61](https://github.com/vjmichael/cognitum/pull/61) | Provider-side typed CE/evidence contract | Embedded Cognitum runtime or host-controlled reasoning service |

## Deferred v2.0 acceptance themes

- Versioned AF_UNIX contract, strict framing and explicit schema evolution. Independent Windows 10/11 and Linux tests of secure peer admission, reconnect, filesystem/socket replacement, ACL and lifetime conditions.
- One host-owned capability router/inference broker for IDE, NVP, Cognitum, Research, Writing; per-call user/creator/project authorization.
- Real GGUF/MMProj model session sharing, safe engine unload/reload negotiation with measured RAM/VRAM/commit budgets and GPU-guardian stress tests.
- End-to-end verified Cognitum evidence exercises; real film workload concurrency; worker crash/cancel/restart, durable receipts and no resource hallucination.
- Complete installation/upgrade/disable/uninstall lifecycle, native UI support, rollback and negative security tests.

## Immediate v1.0 redirection

**Freeze deeper WRITE-01E filesystem race/power-failure research at the existing verified development checkpoint** (branch `lu/write-01e-approved-edit-gate-20261103`, commit `2a9a1355`; CI was green for that limited source scope). Preserve its remaining acceptance caveats rather than declaring it production-complete.

Resume broad native tool coverage via existing `NativeToolRegistry`, `ModelToolCatalog`, typed Protobuf codecs, `ConversationAuthority` and user-bound policy. First harden Lu\'s **internal JSON model/tool boundary**, then prioritize deterministic, bounded, **read-only first** capabilities such as Git inspection, archive listing and further approved developer utilities. The JSON validator is not a user-invoked tool or file inspector. Avoid duplicated tool registries, automatic shell execution, unsafe mutation, and tool authority inferred from model text.

**Tracking:** [Lu DLC-API #8](https://github.com/vjmichael/Lu-AI-GUI/issues/8) and [DLC-IPC #9](https://github.com/vjmichael/Lu-AI-GUI/issues/9) carry the deferred architecture; current v1.0 implementation branch is separate from both experimental DLC branches.
