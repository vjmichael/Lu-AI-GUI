# E1-P09 — Reviewed application evidence in native conversation activity

## User-visible result

The existing **Tool activity** pane now displays reviewed Git/FFprobe
turns alongside Reader, Workspace and Git model/tool turn audit records.
Each reviewed application entry is tied to the authenticated original
human conversation and shows host-recorded model responses, tool
feedback, rejections and repairs **separately from** a read-only summary
of its actual per-turn native effect journal.

The journal summary exposes only bounded counts:
APPLIED, APPLIED with retained receipt, UNCERTAIN, FAILED, EXECUTING
and PREPARED. If the effect journal does not exist, Lu explicitly says
so; it **never fabricates a successful OS effect or makes a new SQLite
database** merely to display evidence.

Model-answer claims cannot upgrade an uncertain effect to success:

- `EFFECT_UNCERTAIN_OR_IN_FLIGHT` overrides a model FinalAnswer when
  any worker journal is PREPARED, EXECUTING or UNCERTAIN.
- `EFFECT_FAILED` and `APPLIED_RECEIPT_MISSING` distinguish
  non-successful effects and missing stored receipts.
- `ANSWER_RECORDED_WITH_APPLIED_JOURNAL` means precisely that:
  a model answer was recorded and some effects were APPLIED with
  retained journal receipts. It does **not** prove an external worker
  remains terminated after restart or certify an arbitrary model answer.
- `MODEL_ANSWER_ONLY` means a model reply is present but no matching
  APPLIED native effect receipt is available.

The read-only history command authenticates the selected ANSF chat
using a one-entry, indexed window; it does not materialize the
potentially huge full conversation just to show a tool activity pane.

## Code

- `lu-desktop/src-tauri/src/reviewed_app_audit.rs` opens the
  host-derived, exact `lu-reviewed-effects-g-<32 lowercase hex>.sqlite`
  file in SQLite READ_ONLY mode. Host turn IDs are validated; symbolic
  links/Windows reparse files and linked WAL/SHM sidecars are refused.
  The database file and row counts are bounded. No raw receipt,
  fingerprint, call ID, subprocess output, file path, model prompt, or
  executable is returned to the WebView.
- The existing `lu_tool_turn_history` command admits entries from
  `lu-reviewed-app-model-audit.sqlite` only when the turn is present
  as a **host-authenticated original human turn** in the selected chat.
  It joins the *two separate evidence sources* as observations;
  neither grants authority, resurrects cancelled tasks, or retries a
  tool effect.
- The existing React Tool activity panel displays the counts and
  explicit evidence caveat; no new app installation, Docker or MCP
  requirement is introduced.
- E1-P08 chat recovery and E1-P08C cgroup-v2 custody are distinct
  workstreams and remain separately gated.

## Required native acceptance

`.github/workflows/lu-e1-p09-reviewed-audit.yml` must run:

1. The **actual production Rust module**, imported by the portable
   native recovery harness, on both Ubuntu 24.04 and Windows 2025.
   Tests construct real EffectJournal SQLite transitions (APPLIED with
   receipt, UNCERTAIN, PREPARED), check no fake missing journal,
   reject invalid host IDs and linked files, and confirm that no
   raw worker receipt/details cross the serialized UI boundary.
2. Model-answer classification regressions: an uncertain, failed or
   missing-receipt native effect may not be called successful even when
   the model recorded a final answer.
3. Actual Node frontend/backend contract tests, including the native
   picker/permission boundary from E1-P07 and this new audit summary.
4. A genuine Windows Tauri cargo check/test and built React UI, with
   strictly labeled nonfunctional CI-only inference placeholders.
5. Exact-head SHA checkout, frozen source verification and evidence
   artifacts. Tests must be completed and inspected at the **final
   candidate commit**, not an unrelated parent or synthetic merge.

No native Windows/Linux or installed-app acceptance is claimed from
source changes alone. This is an inspection capability, not proof of
external process cleanup or authorization to execute new tools.

## Later tasks

A subsequent phase can add independently attributable semantic
outcomes and receipts that a human can click from each original
assistant message, but it must not derive application truth from model
text, or turn read-only activity inspection into a new user grant.
