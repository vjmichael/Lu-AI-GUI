# CP-07/08-UX-R1 — Lu Basic presentation cleanup

**Date:** 2026-11-20
**Branch:** `lu/cp07-08-ux-r1` (separate worktree based on CP-13-R1, itself based on published R7-R2)
**Scope:** Frontend copy, progressive disclosure and matching source contracts. No engine, model, tool, consent, transport, installer, workflow or CI changes.

## Purpose and design authority

Implements the still-open CP-13 V1-09 and UX-NOV16 plain-language requirement: ordinary Lu users should see tasks, models, history, permissions and useful outcomes rather than ANSF, GGUF, MMProj, source hashes, architecture group names, worker internals or performance counters. A person must still be able to inspect original evidence when choosing an Advanced or Technical view. A prettier label is **not** a substitute for a real capability or independent permission check.

## Changes, with exact scope

| Surface | Basic experience | Advanced/on-demand access retained |
|---|---|---|
| Chat model chooser and loading indicator | `LOCAL MODELS`; `Text model` or `Vision enabled`; clear selection-versus-loading warning; spinner reading `Getting your model ready…` | Existing native load/eject, model catalog, operating-system worker status, diagnostics sampling; no fabricated progress percentage |
| Settings | General, Models, Tools & permissions, Browser & research, Memory, Privacy & Observe; Advanced closed at entry | `Show advanced settings` reveals runtime, diagnostic views, model numeric tuning and full host-owned capability administration; toggling off while in an advanced-only panel returns to General |
| Model Library | `My models`, `Add model file…`, `Add vision support…`; friendly readiness; Downloads with readable states and explicit approval | `Technical details` opt-in shows original file hashes, revisions, exact transfer states and errors; native import, preflight, verified transfer, cancel and resume remain authoritative |
| History and revisions | `Saved history`, friendly message search, original message and correction history; exact hashes hidden behind `Technical proof` | Original immutable texts, hash checks, source IDs, corrections, contradictions, local recall and retention unchanged |
| Chat source evidence | `Source history (advanced)` is collapsed by default instead of a permanent ANSF/P0-TOOLS panel | The original native source identities, observed hashes, historic provenance and manual read/capture review are still available, not auto-granted |
| Privacy and collaborative diagnosis | Ordinary Privacy shows user consent and local checks; capture buttons explain exactly which screen image is selected | Collaborative diagnosis controls become available with Advanced settings; actual two-model reviews, cancellation, one-use invites and revocation use the same native calls |
| File proposals and activity | `Working with your file…`, clear interrupted-write warning, `Review permissions and continue` | Existing host-only file pickers, explicit confirmations, custody receipts and recovery remain untouched |

**Conservative boundaries:** GGUF/vision backing files remain displayed by exact filename; no inference or image perception is claimed simply by relabeling. Technical details may still appear in explicitly opened advanced forms and genuine file approval dialogs. No source code in `src-tauri/`, `lu-core/`, GitHub workflows, or backend was modified.

## Local qualification

- React desktop production build: **PASS** (`npm run build`).
- Independent source and safety/behavior contracts: **59/59 PASS** (`npm run check`). This suite was updated only where exact legacy UI wording changed; authority/consent requirements remain.
- New focused Basic/Advanced presentation contracts: **8/8 PASS**.
- Complete Node UI suite: **693 passed, 2 historical failures, 5 skipped (700 total)**. The two original, non-UX failures are `P03 routes from current user and already authorized read catalog only` and `P4 native scheduler maintains process custody and probes immediately before spawn`. A locally available scanner test binary from unchanged scanner sources resolved two missing-executable setup failures. No regression introduced by this change.
- No native Rust, local GGUF, installed graphical Windows, real screenshot walk-through, accessibility screen reader, or physical Radeon/R9700 testing was performed. These gates remain **open**, not passes.
- No new CI jobs; R7 Windows branch remains untouched. The current R7 exact-source hosted run is separate, with desktop source-check passed and a native protocol job blocked by a Windows CRLF-sensitive source-contract assertion.

## Remaining CP-07/08 work

1. **UX-R2 — Installed, accessible Basic/Advanced walkthrough:** screenshot/keyboard/screen-reader audit; verify no technical panels leak into ordinary Chat, History, Model Library, Settings, Tasks during model load, document work, cancellation and restart.
2. **UX-R3 — Review language consistency:** further user-facing terms in active file-operation and project workflows; preserve exact hashes and warnings inside explicit safety approval/recovery reviews.
3. **CP-10 / Windows:** run a fresh exact-source installed NSIS and physical Windows acceptance on a candidate containing this change, not the earlier R7 SHA.
4. **CP-13 parity:** keep V1-09 and UX-NOV16 **partially satisfied** until installed walkthrough and normal model/tool/multimodal outcomes are witnessed. Do not confuse source contract pass with product release acceptance.

## Reproduction

From the checked-out repo root:

```bash
cd lu-desktop
npm run build
npm run check
node --test tests/cp07-08-ux-r1-basic-copy.test.mjs tests/p0-physical-ux-contract.test.mjs tests/m4-belief-contract.test.mjs
node --test tests/*.test.mjs
```

The full suite needs the original bounded native scanner binary built from `tests/native-file-scan` in `tests/native-file-scan/target/debug/`; an absent executable is an unexecuted fixture, never a passing scanner test. The local qualification reused a previously built binary after checking that the original scanner source and Cargo.lock were identical. Offline dependencies were reused through an existing local Node cache; nothing was added to the persistent Library.
