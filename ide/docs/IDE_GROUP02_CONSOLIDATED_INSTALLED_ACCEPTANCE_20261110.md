# IDE Group 2 — Consolidated installed Windows/Linux acceptance

**Scope and owner:** Group 2 only, independent standalone Lu IDE. Stacked
source on Group 1 documentation head `1134531f0425b693a535ae1f46b53fd1d2692bc5`, with the Group 1 actual
native/NSIS witness pinned to `c827a94def0a936177e2c6aa16dc565739dfe61a`.
Do not call Group 1's pending installed Linux job a pass.

## Goal and exact group boundary

One authoritative, CI-only **unsigned** installed-app evidence group on
Windows 2022 (NSIS/WebView2) and Ubuntu 24.04 (Debian/GTK). It must:
- use one final source SHA and review exact npm/Cargo locks and independently
  bundled upstream notices;
- build actual NSIS and Debian packages, verify their SHA-256/notice inventory,
  install and launch each graphical native application;
- operate a genuine OS picker/CodeMirror editor, explicitly Save, restart
  into the real reopened project, reject an external-file collision while
  preserving the unsaved draft;
- verify actual native package uninstall removes installed program bytes,
  while preserving the exact installed app's `projects.json` and project
  `main.rs` without mutation, witnessed by separate read-only before/after
  digests and exact canonical catalog root/host ID;
- retain real Linux dpkg failed-upgrade/previous-version restoration tests,
  including durable user state; retain the default-disabled package/debugger
  security posture and prevent accidental execution on project selection;
- run source and hostile negative contract checks before expensive native
  build; no shell/IPC/JS substitution for genuine GUI evidence.

The shared `group02_user_data_preservation.py` witness requires a disposable
`RUNNER_TEMP` project, a single non-symlink native OS app-data catalog,
a well-formed version-one catalog and exact registered project root/ID. Only
then is a SHA receipt written under `RUNNER_TEMP`. The same data must
survive uninstall unchanged. This is **not** a full signed upgrade or a
physical-PC human-authorized protected worker test.

## Baseline evidence, not current Group 2 acceptance

- [Group 1 native Windows/Linux + protected Linux uv run 42766269202](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42766269202):
  **SUCCESS** at `c827a94...`.
- [Group 1 installed run 42766269459](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42766269459):
  Windows **SUCCESS** at `c827a94...`; Ubuntu still at Linux prerequisite
  stage when Group 2 was started. Do not promote an open job.
- Historical separate B2 installed Windows private draft recovery
  had passed at its own older SHA. It does not automatically prove the
  current installed Group 2 source, and does not authorize protected tools.

## Required report

For each OS record source SHA, exact job/run link, actual package SHA, native
install/launch/UI outcome, explicit Save, restart, file conflict, old-version
rollback where supported, real uninstall, unchanged catalog/source receipts
and any failed step with its original logs. Anything pending or skipped
stays OPEN. A successful code check alone is never an installed pass.

## Exclusions / stop rule

Group 3 security approval, Group 4 protected Run/Debug, Group 5 LSP/IDE
integration and Group 6 final branch convergence are separate work.
`ADR_0003_LU_CORE_DLC_VERSION_BOUNDARIES_20261110.md` keeps all live
Core-DLC integration deferred until after Lu Core v1.0. No signing/legal
release approval, physical approval, PR mass merge or Core source changes
are authorized by Group 2.

## First genuine Group 2 installed run — failed, exact SHA `accdd179e2f0c6ba363b270e7a0794fbf0e974e3`

[Consolidated run 42769749355](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42769749355)
built native NSIS/Debian and passed source/lock/notice checks. Ubuntu native
dpkg injected post-install failure and previous-package restoration passed.

- Linux installed GUI job `129674510885`: GTK's native folder selection
  registered a DIFFERENT directory (`unexpected_project_persisted`), so
  correct exact-root and WebView acceptance failed. In particular,
  `P04_GTK_FIXTURE_DIRECTORY_CANDIDATES=0` accompanied a pointer/typeahead
  fallback, and that fallback was mistakenly treated as a selection
  candidate. This is a genuine native test-driver behavior defect, not
  evidence that any unrelated directory is an acceptable project.
  Corrected driver refuses to infer selection from typeahead and tries
  a direct native GTK location Open path first. Strict exact-root
  registered catalog and real WebView evidence remain mandatory.
- Windows installed GUI job `129674510618`: native shell, actual
  project picker, explicit Save, restart/reopen, protected debugger
  default-denial and external-source conflict all passed. The newly
  added read-only Group 2 receipt failed
  `IDE_G02_USER_DATA_DENIED:exact_registered_project_not_proven`
  at the pre-uninstall comparison. Rust's canonical Windows path may
  carry an extended-length prefix; string spelling is not OS directory
  identity. The repair compares the two existing directories by actual
  filesystem identity (`os.path.samefile`), retaining bounded catalog,
  non-symlink, exact display name and native host-ID checks.
- No installed Group 2 release/approval acceptance can be claimed from
  this red run. One consolidated current-head native installed group
  is required after the repairs; do not dispatch micro-CI.

## GTK native selection repair — 2026-11-10, current Group 2 work

The last true installed Ubuntu native test at SHA `61944a409e86`,
[run 42771116544](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42771116544),
reported no selected GTK directory and no enabled affirmative chooser action.
The actual source was inspecting for the target folder row BEFORE navigating
the native chooser to that folder's parent, so it could not find the real row
in the default chooser location. That ordering defect is now corrected.
GTK's real Ctrl+L navigation to the exact parent happens first. An expanded,
bounded (28 levels / 12,000 elements) native AT-SPI subtree scan now checks
exact accessible name OR exact accessible text under a visible genuine GTK
chooser; it can select a nested native row via actual AT-SPI Selection
or verified row keyboard focus. No fuzzy name match, guessed file path,
Tauri IPC, browser JavaScript, raw path in logs or synthetic registration
is permitted. If the GTK provider still hides the row, a bounded OS-native
keyboard candidate is allowed but registration of a wrong folder or a stuck
chooser remains a hard FAIL. Added pure negative tests for exact-name role
matching and the required navigate-before-scan order.

This is a **repair hypothesis** until new exact-SHA installed Linux GUI
acceptance passes. Windows remains part of one consolidated OS gate at
the final source SHA. Prior Windows result from `61944a409e86...` passed
exact native NSIS install/edit/restart/conflict/uninstall and data retention;
it cannot automatically certify the new SHA.

## Installed GTK failure analysis — new evidence 2026-11-10

[Group 2 run 42773916622](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42773916622)
built and installed actual Debian bytes and passed its real failed-upgrade
recovery gate. The native GTK picker remained open. It reported 20 visible
native rows yet ZERO exact target names/text immediately after the attempted
Ctrl+L parent navigation. Later typeahead created 2 exact accessible TEXT
matches, but without native row selection. This proves the previous matching
rule risked confusing the GTK location/search entry with a selectable
directory, even though independent installed registration correctly denied it.

The native GTK3 file chooser supports a documented per-user startup-mode
preference to open at the current working directory. On disposable CI only,
the real GTK/X11 installed editor now starts in an isolated parent directory
containing only the actual test project child. Its DBus session calls
`gsettings set org.gtk.Settings.FileChooser startup-mode cwd` and verifies
it. The installed IDE process launches with this parent as its CWD;
no project path is sent into Lu IDE, the OS dialog is still required,
and no worker authorization is created. The exact row identification policy
now requires GTK file-list row/cell identity or a label nested inside it,
never bare location-entry/search text. A verified closed chooser is also
required before sending any additional native keyboard input.
This is test-environment determinism and strictly stronger native input
identity, not a bypass of actual GUI folder selection. No success is claimed
until one **final exact-head** Windows/Linux consolidated installed group
passes with both project catalog and real WebView root confirmation,
editing/restart/conflict/uninstall and user-data preservation.

## Native WebView project activation after GTK selection — 2026-11-10 UTC

[Run 42775168983](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42775168983)
was the first genuine Ubuntu Debian-installed group in which
`P04_NATIVE_GTK_REGISTRATION_RESULT=registered_exact_root_and_webview`
and `P04_NATIVE_GTK_HOST_PICKER_RESULT=registered` PASSED. The host
`register_from_native_picker` atomically persists and activates the record
(`active: true`), but the GTK installed editor test next failed looking for
the plain-text React paragraph `Open · editor available`, whose AT-SPI
accessible name is not guaranteed. This was a **test/UI accessibility contract
gap**, not proven evidence of inactive host registration. The same run's
Windows installed acceptance succeeded.

Repair: the actual React project-card **Edit** and **Reopen** buttons expose
explicit, human-readable project-scoped `aria-label` names. The installed
Linux AT-SPI test observes the exact registered project card **AND** a visible,
enabled, active-only native `Edit project <display name>` button before it
claims WebView registration. It activates that unique control through real
AT-SPI action or OS keyboard focus. On restart it first uses an exact, visible
`Reopen project <display name>` button where available. The stale paragraph
string no longer acts as a gate. Diagnostic counters (exact project label,
visible Edit/Reopen buttons, enabled states) report categories only, never raw
file paths, accessible text or source data. GTK chooser and host catalog
root-equality, native Save, restart, conflict and uninstall/data preservation
checks remain mandatory. No direct Tauri IPC, injected DOM automation,
synthetic registration or protected execution is introduced.

This is an unaccepted implementation until the next exact-head NSIS/Debian
installed Windows and Linux group runs to completion.

## Exact Linux native editor Save/restart success and conflict alert gap (2026-11-10)

[Consolidated run 42835712577](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42835712577),
source `d4eb5fa8154352ac9c2ec8b5728ad548613794eb`,
demonstrated these **genuine installed Ubuntu 24.04** results:

- `P04_NATIVE_GTK_REGISTRATION_RESULT=registered_exact_root_and_webview`
- `IDE_G02_NATIVE_ACTIVE_EDITOR_BUTTON=verified_exact_project`
- `WO8_LINUX_NATIVE_EDITOR_REVIEW_AND_EXPLICIT_SAVE=PASS`
- `WO8_LINUX_PROJECT_REOPEN=exact_native_atspi_action`
- `WO8_LINUX_NATIVE_RESTART_REOPEN=PASS`

It then failed when the real external source changed and the fixture waited for
a free-form error-message substring `File changed on disk` to be exposed
as an AT-SPI object. This showed a second **accessibility labeling gap**:
React's visible `role="alert"` did not have an explicitly named alert,
and the dirty source tab exposed only a visual dot / nonsemantic text.

The corrected React UI now assigns a fixed accessibility label to the
actual conflict alert ONLY when the native Save returns `FILE_CHANGED_ON_DISK`,
with the full error preserved via `aria-describedby`. The existing
editor tab's actual `tab.dirty` state also produces a named
`Unsaved source tab <relative path>` accessible control. The Linux
installed AT-SPI witness insists on both named, showing native elements
for the exact fixture; the disk SHA must still equal the external writer's
pre-save SHA, and outside edits must remain unmodified. A generic footer
or banner text, failed save call, synthetic DOM value or injected IPC is
never enough. On failure only fixed category-count diagnostics are emitted.
No save semantics or filesystem authorization were changed.

The new source must pass one final exact-SHA Windows NSIS/Linux Debian
installed group, including error-state witness, project save/restart,
real package uninstall and source/catalog preservation before Group 2 closes.

## Real Linux conflict alert reached; nested source-tab accessibility — 2026-11-10 UTC

[Consolidated installed run 42837344891](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42837344891),
source `6200387e765a9ce5a21baa4365b4a759c0a938f7`, confirmed real
native GTK project registration, active Edit button, explicit Save and
reopened project after restart. It also confirmed
`IDE_G02_LINUX_NATIVE_CONFLICT_INVENTORY=alert:visible;dirty_tab:missing`.
The new *actual React conflict error role=alert* is present. However the
global installed AT-SPI desktop traversal is bounded to 13 levels and does
not necessarily reach CodeMirror's deeply nested source tab (which has the
explicit unsaved-label derived from `tab.dirty`). That absence is not
proof the native disk guard failed; equally it cannot be promoted as an
accepted preserved draft.

Repair: the conflict witness still requires a distinct installed native
named alert and unchanged external file SHA, but now scans only the real
`Source explorer and editor` accessible workspace's descendants using a
separate, bounded traversal (28 levels, 8,000 elements). It demands the exact
`Unsaved source tab main.rs` named tab *under that workspace*. A generic
footer, arbitrary other tab, external file overwrite, or injected tool call
still fails. No Rust host, source Save, worker permission or Lu Core code
was changed. One exact-head Windows/Linux installed acceptance pair must
prove the repair before Group 2 can close.

## Real Linux packaging interrupted by missing upstream notice — 2026-11-10

[Run 42838670900](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42838670900),
source `9caea6b6ad04d2f25abc9405de5105f5ce34cfa4`: Ubuntu installation prerequisites and all 276 frontend
tests passed, but the package stage stopped **before any GTK installed
acceptance** with `DENIED: upstream_notice_fetch_unavailable` from the
existing exact-origin verification. This is an external license-source fetch
failure, not an IDE compiler or GTK regression. Windows installed acceptance
passed at this same SHA.

The provenance helper now retries only bounded temporary transport errors
(three attempts, 1/2 second pause) for each of its six *unchanged pinned
upstream URLs*. A permanent HTTP 404, unexpected redirect, excessive bytes,
remote/local notice mismatch, absent receipt, or failed origin verification
remains a hard FAIL. The original exact source SHA and notice content
comparison are unchanged. Added offline positive/hostile tests for one
transient recovery, permanent HTTP rejection, unexpected redirect, and
permanent network refusal. Both Windows/Linux gate jobs test those policies
before expensive package generation.

The canonical Group 2 final acceptance remains OPEN until a single
exact-SHA installed Windows NSIS plus Linux Debian run completes, including
real GTK editor conflict/draft, restart and uninstall with unchanged
source/catalog evidence. No legal release approval is inferred.

## Linux preserved draft native status after conflict (2026-11-10)

[Exact-head grouped run 42840704155](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42840704155)
at `2d7efd74808caf94ff4b5b093427d958a43e36b4` confirmed Windows
NSIS **SUCCESS** including real project Save/restart/conflict, native
uninstall and unchanged catalog+source hashes. Linux native Debian and
upstream license verification passed, as did real GTK exact project
registration, active Edit, explicit Save, restart and Reopen. The Linux
conflict check failed with `alert:visible;dirty_tab:missing` even after
a bounded 28-level AT-SPI source-workspace subtree traversal. It reached
the original conflict check, but the named WebKitGTK tab role is not
reliably exported, so the error cannot be promoted without a separate
real *dirty editor state* witness.

The React source preview's already-visible character-count / Saved vs
Unsaved status now explicitly exposes `role=status` with an accessible
name derived **only from `activeTab.dirty` and `activeTab.relativePath`**.
This is the actual currently visible editor toolbar, not a newly fabricated
or generic footer flag. The installed native Linux test still demands the
named conflict `role=alert` and the original external file's exact SHA
unchanged after rejected Save, and now accepts either the exact native
dirty source tab OR this exact named active-editor status as the
independently accessible dirty-state witness. It rejects arbitrary
unrelated status, successful file overwrite, wrong source name or
inaccessible editor. Source Save behavior and native security/policy
remain unchanged.

This source remains unaccepted until the next exact-head consolidated
Windows/Linux installed run reports all required checks including
the real Debian uninstall and catalog+source retention.

## Conflict and unsaved draft: definitive native Save-state witness (2026-11-10)

[Exact-head installed run 42843681742](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42843681742),
source `5dd6032abf729e8fa71fab4a50dc8233b5fe80d5`, again proved real GTK registered root, exact active
project Edit, explicit source Save, native restart/reopen, and a visible
`File changed on disk. Unsaved edits preserved` alert. WebKitGTK still
does not expose the dirty-tab label or the new active source status by
the tested AT-SPI traversals (`alert:visible;dirty_tab:missing`).
This is an **accessibility discovery limitation**, not demonstrated loss
of the unsaved draft. The disk hash and external writer sentinel remain
mandatory.

The actual source editor's primary `Save · Ctrl+S` button is
`disabled={!activeTab.dirty || explorerPending || !!savingKey}`.
A real native click on this button caused the exact conflict alert.
Therefore, checking the **same unique, visible, enabled native Save
button again AFTER the rejected operation** independently proves the
actual current editor remains dirty and can still attempt explicit
Save, without trusting generic footer text or missing tab names.
The installed acceptance now requires the actual alert, external
disk-content SHA unchanged, and either the exact dirty tab, exact
dirty active source status, or exactly one enabled real primary Save
button. Source contract tests bind the last case to React's
`activeTab.dirty` gate; any disabled/missing/ambiguous Save button,
wrong project, absent alert or overwritten disk still FAILS.
No host Save logic or filesystem authorization changed. The repair
remains unaccepted until another exact-head installed Windows/Linux
consolidated group passes including package uninstall and retention.

## Group 2 native installed Jobs empty-journal accessibility — 2026-11-10

[Run 42846150719](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42846150719),
exact source `f6eb02d8a12cea8e0762ed592526d47a57001b01`: Windows NSIS installed UI **PASS**.
Ubuntu Debian installed GTK exact project registration, active Edit,
explicit Save, restart/reopen, external file-hash rejection and genuinely
unsaved draft **PASS**. The follow-on installed read-only Jobs test found the
actual headings `DEV-03.4`, `DEV-03.3B` and `DEV-03.5`, but failed to
discover the plain paragraph `No matching protected job journal entries observed`.
This is an installed accessibility-surface gap, not evidence of a worker
executing, the source changing, or a broken native journal. The job never
reached the post-uninstall project/catalog retention gate, so Linux Group 2
is **NOT ACCEPTED**.

Targeted repair: React keeps the same real `projectJobLifecycle(...).jobs.length===0`
condition and visible text, but makes that paragraph a distinctly named
`role=status` accessibility node. The real Debian-installed AT-SPI test
first finds the existing DEV-03.5 heading, then its named parent region,
and scans only that native WebView region (max 3,200 nodes / 24 levels).
It requires exactly one visible named empty-journal status and no
project-job list; on failure it prints fixed-category counts only.
It **still checks** real native Discover test files, default-disabled
protected Run/Test, unchanged source digest and real restart. No IPC,
mock Jobs state, direct catalog editing, executable authorization or
Lu Core integration is introduced. Negative/source contract tests run
before expensive Linux packaging.

**Closure policy:** one final exact-head Windows NSIS and Linux Debian
installed acceptance pair. Linux must also reach real dpkg uninstall,
absence of the installed program and byte-identical original source and
native project catalog. Do not promote PR #203 or Group 2 without both
completed successful OS jobs.
