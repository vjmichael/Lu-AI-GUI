# Lu PLAN-07D — installed NSIS and Debian application acceptance

**Scope:** Lu Core, stacked on PLAN-07C draft PR #219, not Lu IDE. This is an
independent installed-application gate, not a new authority or automatic
completion path. Do not merge, tag, promote an installer, or start PLAN-08.

## Acceptance boundary (2026-11-11)

Source head before this gate: `2d46a9ad4c72b2976d90f9b785a8720747ecd7c2`.
07C's native prepared/terminal review journal has been implemented in source,
but actual Windows/Linux compiler, genuine Qwen and installed operator
results are independent. No acceptance is implied by source, queued CI,
a check mark in a task list, or a process merely launching.

One opt-in pair of grouped hosted jobs in
`.github/workflows/lu-plan07d-installed.yml` requires a **new one-time
PR #219 body transition** of `LU_PLAN07D_INSTALLED_SHA=<exact 40-digit head>`.
No per-edit heavyweight CI fanout. This does not touch the separate PLAN-05
or PLAN-06 branches, and it does not promote a release.

## Windows — real NSIS application

The exact-head job builds the genuine source-pinned CPU llama.cpp NSIS
installer, verifies the installer, packaged engine and installed native
executable hashes, and installs into an isolated temporary directory. It
creates two source-backed projects and original ANSF SQLite register history
using the existing **explicitly synthetic original model fixture**, which
must never be labeled as real GGUF output.

`scripts/plan07d-windows-installed-acceptance.ps1` requires all of the
following, using **actual OS windows and UI Automation** rather than a
headless renderer stand-in:

- Installed process identity and independent Project Tasks HWND; accessible
  names for evidence refresh, human review journal, task selection and note.
- Operating-system folder selection for the selected original source and a
  host-backed source preview.
- Native OS **Refuse evidence review** with a new durable
  `prepared -> refused` review; no execution or requirement completion.
- Native OS approval followed by cancelled folder selection with durable
  `prepared -> cancelled`; no phantom approval.
- Native OS **Approve evidence review only**, followed by the original native
  project picker and repeated source validation, yielding only
  `approved_for_review`; it does not append task completion.
- Visible switch between two distinct source projects, graceful application
  close, restart and readback of all unchanged original decision SHAs.
- Forced process interruption *during the genuine prepared OS confirmation*,
  followed by restart and a new `interrupted` terminal record, never a
  replayed approval.
- Actual NSIS uninstall, absence of installed files, and unchanged original
  proposal, register, original user marker, board and human decision receipts.

A missing UIA control, OS dialog, source picker, source receipt, or restart
proof is a test failure. Hosted CI clicks do not satisfy physical-owner
human evaluation or the Radeon AI Pro R9700/Qwen3.8-27B test.

## Debian — real installed Linux package, bounded initial smoke

The Linux job builds the source-head native Tauri ELF, wraps **that real
binary** in an actual `.deb`, installs with `dpkg -i`, checks the installed
binary digest, seeds the same original two ANSF scopes under a disposable
`XDG_DATA_HOME`, and uses a real Xvfb/DBus GTK/WebKit desktop with AT-SPI
to observe and invoke the installed **Project tasks** action. It closes and
restarts the installed executable, then removes the package with
`dpkg -r` and independently compares the original ANSF record.

This is **hosted Debian UI smoke, not completed PLAN-07D Linux acceptance**:
genuine source picker, approval/refusal/cancellation, project switching and
interrupted-review restart must still be tested interactively on installed
Debian. Linux compile-time Windows resource stubs are explicitly inert and
must never be represented as a packaged or loaded genuine model. Neither
Debian smoke nor NSIS host fixture is genuine GGUF inference.

## Independent original ANSF oracle

`scripts/plan07d-readonly-ansf-witness.py` invokes the existing independent
PLAN-06 original-source witness, then reopens the **same original
`ansf/library.db`** read-only and checks the PLAN-07C event table:
original serialized byte hash, one prepared plus at most one terminal event,
predecessor SHA, immutable task/source/model proof fields, exact operator note,
two original projects, both `noCompletionAuthority` and
`noExecutionAuthority`, and no unrelated ANSF data loss. The oracle can
require phases but cannot certify UI clicks; only separate OS automation can.
No table is modified by the oracle.

## Explicit open gates — no release acceptance yet

1. Inspect actual results of the exact-head grouped Windows/Linux native
   PLAN-07A/B/C matrix; repair any demonstrated failures.
2. Inspect actual PLAN-07D hosted NSIS and Debian job logs, original UIA/AT-SPI
   trees and read-only SQLite receipts; fix all demonstrated failures. Do not
   count skipped, queued, cancelled or purely source checks as success.
3. Complete **Linux Debian native folder selection, refusal, approval,
   cancellation, project switching and interruption/restart**. The included
   Debian smoke is not this full matrix.
4. Run the genuine loaded GGUF PLAN-07B host/installed cancellation and
   evidence-citation tests. No original Qwen generation in the disposable
   task fixtures is claimed.
5. Owner-operated **physical Windows** installed accessibility,
   approval/refusal, source picker, project switching, restart, ANSF
   preservation and genuine Qwen3.8-27B/Radeon acceptance. Do not alter the
   owner's original user profile to test.
6. Preserve independent PLAN-05 and PLAN-06 installed gates. No automatic
   model acceptance, task completion, native worker authority or merge to
   `main`.

**Status:** PLAN-07D acceptance infrastructure staged. The gate remains
**OPEN** until original current-head operating-system receipts pass.
