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

## Debian — real installed Linux package and complete scripted human matrix

The Linux job builds the exact source-head native Tauri ELF, wraps **that real
binary** in an actual `.deb`, installs with `dpkg -i`, checks the installed
binary digest, seeds the original two-project ANSF profile under a disposable
`XDG_DATA_HOME`, and uses a genuine X11/DBus/GTK/WebKit environment.

The first AT-SPI smoke verifies the installed native task-list window. The
subsequent **strict OS-interaction matrix**, implemented in
`scripts/plan07d-debian-installed-human.py`, requires:

1. Accessible installed process/window identity, original task controls and a
   keyboard-editable independent review note, verified through live AT-SPI.
2. Opening a *real* native GTK/portal source-folder picker and selecting the
   same original project (never passing a renderer-controlled source path).
3. Actual native dialog **Refuse evidence review** and independent read-only
   original ANSF `prepared -> refused` proof.
4. Actual native dialog **Approve evidence review only** followed by an actual
   OS folder-picker cancellation and `prepared -> cancelled` proof.
5. Actual native evidence-review approval, followed by the current original
   project picker and fresh independent source scan, producing exactly
   `approved_for_review`. This is **not requirement or task acceptance**.
6. Visible switch between two distinct original projects, normal installed
   application close and restart, and original SHA continuity.
7. Kill the actual installed process with an unresolved prepared native review,
   then reopen the real app, load the review history and verify exactly one
   `interrupted` terminal record without a phantom human approval.
8. Run actual `dpkg -r`, then compare original source, preserved user data,
   task/plan and decision histories byte-for-byte to their **pre-removal**
   independent receipts.

The script refuses to claim success if any OS window, accessibility control,
source picker, revision, or original receipt cannot be observed. Scripted
hosted UIA/AT-SPI tests are still not physical-owner acceptance. On Linux,
the package uses explicitly inert *compile-time* Windows resource placeholders:
they are **not** a genuine loaded model.

**Completion state:** the matrix is implemented and wired into one grouped
acceptance workflow, but no passing native OS execution receipts have yet
been retrieved. Do not call the Debian human matrix passed based on source.

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
3. Obtain and inspect **Linux Debian genuine native folder selection, refusal,
   approval, cancellation, project switching, graceful and interrupted restart**
   receipts. These interactions are implemented in the new scripted matrix,
   not yet demonstrated by native OS logs.
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

**Status:** PLAN-07D Windows and Debian installed interaction matrices staged. The gate remains
**OPEN** until original current-head operating-system receipts pass.


## Evidence investigation and CI queue reconciliation

The first exact-source PLAN-07D grouped run was GitHub Actions #43063700540,
created at 2026-11-11 16:59Z at `d913acbd`. At inspection, both OS jobs
remained **queued** without compiler, UIA, or AT-SPI steps and without any
uploaded evidence artifacts. This is a scheduler/queue state, *not* a test
failure or a PASS. Numerous preceding source-only PR synchronizations ended
`cancelled`, providing no installation proof. The new full-Debian matrix is
added to the same grouped gate and must be read at the **next exact head**.
Do not trigger per-file CI or falsely attribute stale receipts to a newer SHA.

### Genuine GGUF reconciliation

The existing `genuine-plan07b-gguf` job in REPO-01 calls
`scripts/plan07b-real-gguf-acceptance.sh` to run a pinned Qwen3-0.6B GGUF
against a genuine native ANSF fixture, original reply, cancellation, citations
and read-only independent no-auto-acceptance oracle. That is a separate host
model test, **not** the installed Lu normal FIFO path and **not** the owner's
Qwen3.8-27B/Radeon test. No current-head genuine model PASS receipt is
available in the inspected PR-branch runs; the earlier exact SHA marker alone
is not test evidence. The single consolidated PLAN-07D workflow may include
its own genuine host model job; it must preserve this distinction.

### Owner physical Windows boundary

Hosted Windows CI cannot operate the creator's physical Radeon AI Pro R9700.
Do not touch, reset or migrate their existing ANSF profile for fixture tests.
Physical acceptance requires an independently hash-identified installer,
operator-owned test project, real native app interactions with screen-reader
and keyboard, manual approval/refusal/cancellation/picker, two-project switch,
actual restart, and a query-only preservation comparison against a backed-up
original ANSF snapshot. Actual loaded local Qwen3.8-27B/MMProj model and
cancellation receipts must be explicitly observed; hosted CPU Qwen3-0.6B
cannot satisfy that gate. Until owner-operated receipts exist, keep physical
acceptance OPEN regardless of hosted status.
