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
